//! Persist transient craft events with the building/claim context available at receipt.
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use entity::craft_activity;
use entity::shared::Region;
use game_module::module_bindings::{
    BuildingState, BuildingStateTableAccess, CraftCompletedEvent, CraftCompletedEventTableAccess,
    CraftEvent, CraftEventTableAccess, DbConnection, ProgressiveActionState,
    ProgressiveActionStateTableAccess, Reducer,
};
use sea_orm::{EntityTrait, Set};
use spacetimedb_sdk::{Event, EventTable, Table, TableWithPrimaryKey};

use crate::AppState;
use crate::websocket::worker_channel::{self, UnboundedSender};

#[derive(Clone, Copy)]
struct Project {
    building: u64,
    recipe: i32,
    count: i32,
}

#[derive(Default)]
struct Context {
    projects: DashMap<u64, Project>,
    buildings: DashMap<u64, u64>,
}

impl Context {
    fn project(&self, value: &ProgressiveActionState) {
        self.projects.insert(
            value.entity_id,
            Project {
                building: value.building_entity_id,
                recipe: value.recipe_id,
                count: value.craft_count,
            },
        );
    }

    fn building(&self, value: &BuildingState) {
        self.buildings
            .insert(value.entity_id, value.claim_entity_id);
    }

    fn claim(&self, building: u64) -> Option<i64> {
        self.buildings
            .get(&building)
            .and_then(|claim| (*claim != 0).then_some(*claim as i64))
    }
}

fn timestamp(source: &Event<Reducer>) -> Option<DateTime<Utc>> {
    match source {
        Event::Reducer(reducer) => {
            DateTime::from_timestamp_micros(reducer.timestamp.to_micros_since_unix_epoch())
                .or_else(|| Some(Utc::now()))
        }
        Event::Transaction => Some(Utc::now()),
        _ => None,
    }
}

fn hit(
    value: &CraftEvent,
    region: Region,
    time: DateTime<Utc>,
    context: &Context,
) -> craft_activity::ActiveModel {
    let project = context.projects.get(&value.target_entity_id).map(|p| *p);
    craft_activity::ActiveModel {
        region: Set(region),
        timestamp: Set(time.into()),
        event_type: Set(0),
        actor_entity_id: Set(value.actor_entity_id as i64),
        progressive_action_entity_id: Set(value.target_entity_id as i64),
        building_entity_id: Set(project.map(|p| p.building as i64)),
        claim_entity_id: Set(project.and_then(|p| context.claim(p.building))),
        recipe_id: Set(project.map(|p| p.recipe)),
        craft_count: Set(project.map(|p| p.count)),
        progress: Set(Some(value.progress)),
        is_crit: Set(Some(value.is_crit)),
        ..Default::default()
    }
}

fn completed(
    value: &CraftCompletedEvent,
    region: Region,
    time: DateTime<Utc>,
    context: &Context,
) -> craft_activity::ActiveModel {
    craft_activity::ActiveModel {
        region: Set(region),
        timestamp: Set(time.into()),
        event_type: Set(1),
        actor_entity_id: Set(value.actor_entity_id as i64),
        progressive_action_entity_id: Set(value.progressive_action_entity_id as i64),
        building_entity_id: Set(Some(value.building_entity_id as i64)),
        claim_entity_id: Set(context.claim(value.building_entity_id)),
        recipe_id: Set(Some(value.recipe_id)),
        craft_count: Set(Some(value.craft_count)),
        progress: Set(None),
        is_crit: Set(None),
        ..Default::default()
    }
}

pub(crate) fn register_listeners(
    ctx: &DbConnection,
    region: Region,
    tx: UnboundedSender<craft_activity::ActiveModel>,
) {
    // Separate per-connection maps avoid cross-region collisions and are rebuilt on reconnect.
    let context = Arc::new(Context::default());
    let cache = context.clone();
    ctx.db.on_initial_owned::<ProgressiveActionState>(
        "progressive_action_state",
        move |_, rows| {
            cache.projects.clear();
            for row in &rows {
                cache.project(row);
            }
        },
    );
    let cache = context.clone();
    ctx.db
        .progressive_action_state()
        .on_insert(move |_, row| cache.project(row));
    let cache = context.clone();
    ctx.db
        .progressive_action_state()
        .on_update(move |_, _, row| cache.project(row));
    let cache = context.clone();
    ctx.db.progressive_action_state().on_delete(move |_, row| {
        cache.projects.remove(&row.entity_id);
    });

    let cache = context.clone();
    ctx.db
        .on_initial_owned::<BuildingState>("building_state", move |_, rows| {
            cache.buildings.clear();
            for row in &rows {
                cache.building(row);
            }
        });
    let cache = context.clone();
    ctx.db
        .building_state()
        .on_insert(move |_, row| cache.building(row));
    let cache = context.clone();
    ctx.db
        .building_state()
        .on_update(move |_, _, row| cache.building(row));
    let cache = context.clone();
    ctx.db.building_state().on_delete(move |_, row| {
        cache.buildings.remove(&row.entity_id);
    });

    let cache = context.clone();
    let hit_tx = tx.clone();
    EventTable::on_insert(&ctx.db.craft_event(), move |event, value| {
        if let Some(time) = timestamp(&event.event)
            && hit_tx.send(hit(value, region, time, &cache)).is_err()
        {
            tracing::warn!(region, "Could not queue craft hit");
        }
    });
    EventTable::on_insert(&ctx.db.craft_completed_event(), move |event, value| {
        if let Some(time) = timestamp(&event.event)
            && tx.send(completed(value, region, time, &context)).is_err()
        {
            tracing::warn!(region, "Could not queue craft completion");
        }
    });
}

pub(crate) fn start_worker(state: AppState, flush_delay: Duration) -> UnboundedSender<craft_activity::ActiveModel> {
    let (tx, mut rx) =
        worker_channel::shutdown_channel::<craft_activity::ActiveModel>(&state.shutdown);
    state.shutdown.clone().spawn_worker(async move {
        let mut batch = Vec::with_capacity(500);
        while crate::websocket::batched_worker::collect_db_batch(&mut rx, &mut batch, 500, flush_delay).await != 0 {
            metrics::histogram!("worker_db_flush_rows", "worker" => "craft_activity").record(batch.len() as f64);
            for attempt in 1..=3 {
                match tokio::time::timeout(Duration::from_secs(30), craft_activity::Entity::insert_many(batch.clone()).exec(&state.conn)).await {
                    Ok(Ok(_)) => break,
                    Ok(Err(error)) => tracing::error!(%error, attempt, "Could not persist craft activity"),
                    Err(_) => tracing::error!(attempt, "Craft activity persistence timed out"),
                }
                if attempt == 3 {
                    metrics::counter!("craft_activity_dropped_total", "reason" => "persistence_failed").increment(batch.len() as u64);
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            batch.clear();
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

    fn context() -> Context {
        let context = Context::default();
        context.projects.insert(
            1,
            Project {
                building: 2,
                recipe: 3,
                count: 4,
            },
        );
        context.buildings.insert(2, 5);
        context
    }

    #[test]
    fn hits_retain_context_and_raw_data_when_context_missing() {
        let value = CraftEvent {
            actor_entity_id: 6,
            target_entity_id: 1,
            progress: 7,
            is_crit: true,
        };
        let context = context();
        let row = hit(&value, 8, Utc::now(), &context);
        assert_eq!(row.building_entity_id, Set(Some(2)));
        assert_eq!(row.claim_entity_id, Set(Some(5)));
        assert_eq!(row.recipe_id, Set(Some(3)));
        assert_eq!(row.actor_entity_id, Set(6));
        assert_eq!(row.progress, Set(Some(7)));
        context.projects.clear();
        let row = hit(&value, 8, Utc::now(), &context);
        assert_eq!(row.building_entity_id, Set(None));
        assert_eq!(row.progressive_action_entity_id, Set(1));
        assert_eq!(row.is_crit, Set(Some(true)));
    }

    #[test]
    fn completion_uses_event_fields_and_claim_context() {
        let value = CraftCompletedEvent {
            actor_entity_id: 6,
            progressive_action_entity_id: 1,
            building_entity_id: 2,
            recipe_id: 3,
            craft_count: 4,
        };
        let context = context();
        context.projects.clear();
        let row = completed(&value, 8, Utc::now(), &context);
        assert_eq!(row.building_entity_id, Set(Some(2)));
        assert_eq!(row.claim_entity_id, Set(Some(5)));
        assert_eq!(row.recipe_id, Set(Some(3)));
        assert_eq!(row.craft_count, Set(Some(4)));
        assert_eq!(row.progress, Set(None));
    }

    #[tokio::test]
    async fn worker_drains_events_on_shutdown() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![craft_activity::Model {
                id: 1,
                region: 8,
                timestamp: Utc::now().into(),
                event_type: 0,
                actor_entity_id: 6,
                progressive_action_entity_id: 1,
                building_entity_id: Some(2),
                claim_entity_id: Some(5),
                recipe_id: Some(3),
                craft_count: Some(4),
                progress: Some(7),
                is_crit: Some(true),
            }]])
            .into_connection();
        let (out, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        let tx = start_worker(state.clone(), Duration::from_secs(60));
        tx.send(hit(
            &CraftEvent {
                actor_entity_id: 6,
                target_entity_id: 1,
                progress: 7,
                is_crit: true,
            },
            8,
            Utc::now(),
            &context(),
        ))
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.shutdown.drain())
            .await
            .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
    }
}
