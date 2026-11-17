use std::collections::{HashMap, HashSet};
use std::time::Duration;

use entity::public_progressive_action_state::{Column, Entity, Model};
use game_module::module_bindings::PublicProgressiveActionState;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, sea_query};

use crate::AppState;
use crate::websocket::{
    SpacetimeUpdateMessages, WorkerReceiveMetrics, batched_worker::BatchTimer,
    worker_channel::UnboundedReceiver,
};

#[derive(Default)]
struct Pending {
    upserts: HashMap<i64, Model>,
    deletes: HashSet<i64>,
}

impl Pending {
    fn upsert(&mut self, model: Model) {
        self.deletes.remove(&model.entity_id);
        self.upserts.insert(model.entity_id, model);
    }

    fn delete(&mut self, id: i64) {
        self.upserts.remove(&id);
        self.deletes.insert(id);
    }

    async fn flush(&mut self, state: &AppState, conflict: &sea_query::OnConflict) -> bool {
        let mut success = true;
        let rows: Vec<_> = self
            .upserts
            .drain()
            .map(|(_, model)| model.into_active_model())
            .collect();
        if !rows.is_empty()
            && let Err(error) = Entity::insert_many(rows)
                .on_conflict(conflict.clone())
                .exec(&state.conn)
                .await
        {
            tracing::error!(%error, "Could not insert public progressive actions");
            success = false;
        }
        let ids: Vec<_> = self.deletes.drain().collect();
        for chunk in ids.chunks(1000) {
            if let Err(error) = Entity::delete_many()
                .filter(Column::EntityId.is_in(chunk.iter().copied()))
                .exec(&state.conn)
                .await
            {
                tracing::error!(%error, "Could not delete public progressive actions");
                success = false;
            }
        }
        success
    }

    async fn snapshot(
        &mut self,
        state: &AppState,
        conflict: &sea_query::OnConflict,
        data: Vec<PublicProgressiveActionState>,
        region: entity::shared::Region,
        batch_size: usize,
    ) {
        if !self.flush(state, conflict).await {
            tracing::error!(
                region,
                "Could not flush public progressive actions before snapshot"
            );
            return;
        }
        if let Err(error) = crate::websocket::initial_sync::reconcile::<Entity, _, _, _, _, _>(
            &state.conn,
            Column::Region,
            region,
            conflict,
            batch_size,
            data.into_iter()
                .map(|value| Model::from_wire(value, region)),
            |model| model.entity_id,
            |ids| sea_orm::Condition::all().add(Column::EntityId.is_in(ids.iter().copied())),
            |_, _| {},
        )
        .await
        {
            tracing::error!(%error, region, "Could not reconcile public progressive actions");
        }
    }
}

pub(crate) fn start_worker(
    state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PublicProgressiveActionState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    state.shutdown.clone().spawn_worker(async move {
        let metrics = WorkerReceiveMetrics::new("public_progressive_action_state");
        let conflict = sea_query::OnConflict::column(Column::EntityId)
            .update_columns([Column::BuildingEntityId, Column::OwnerEntityId, Column::Region]).to_owned();
        let mut pending = Pending::default();
        let mut changed_buildings = HashSet::new();
        let mut changed_players = HashSet::new();
        loop {
            let mut timer = BatchTimer::new(time_limit);
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break; };
                        timer.start();
                        metrics.record(1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => { pending.snapshot(&state, &conflict, data, database_name, batch_size).await; changed_buildings.clear(); changed_players.clear(); crate::crafting::notifications::all(&state); },
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => { changed_buildings.insert(new.building_entity_id as i64); changed_players.insert(new.owner_entity_id as i64); pending.upsert(Model::from_wire(new, database_name)); },
                            SpacetimeUpdateMessages::Update { old, new, database_name, .. } => { changed_buildings.insert(old.building_entity_id as i64); changed_buildings.insert(new.building_entity_id as i64); changed_players.insert(old.owner_entity_id as i64); changed_players.insert(new.owner_entity_id as i64); pending.upsert(Model::from_wire(new, database_name)); },
                            SpacetimeUpdateMessages::Remove { delete, .. } => { changed_buildings.insert(delete.building_entity_id as i64); changed_players.insert(delete.owner_entity_id as i64); pending.delete(delete.entity_id as i64); },
                        }
                        if pending.upserts.len() >= batch_size || pending.deletes.len() >= batch_size { break; }
                    }
                    _ = timer.elapsed() => break,
                    else => break,
                }
            }
            if pending.flush(&state, &conflict).await { crate::crafting::notifications::players(&state, &mut changed_players); crate::crafting::notifications::buildings(&state, &mut changed_buildings).await; } else { changed_buildings.clear(); changed_players.clear(); }
            if rx.is_closed() && rx.is_empty() { break; }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, DbErr, MockDatabase};

    fn state(db: sea_orm::DatabaseConnection) -> AppState {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        AppState::new(db, tx, Default::default())
    }

    fn conflict() -> sea_query::OnConflict {
        sea_query::OnConflict::column(Column::EntityId)
            .update_columns([
                Column::BuildingEntityId,
                Column::OwnerEntityId,
                Column::Region,
            ])
            .to_owned()
    }

    #[test]
    fn coalesces_updates_and_deletes() {
        let mut pending = Pending::default();
        let wire = PublicProgressiveActionState {
            entity_id: 1,
            building_entity_id: 2,
            owner_entity_id: 3,
        };
        pending.upsert(Model::from_wire(wire.clone(), 4));
        pending.delete(1);
        assert!(pending.upserts.is_empty());
        pending.upsert(Model::from_wire(wire, 5));
        assert!(pending.deletes.is_empty());
        assert_eq!(pending.upserts[&1].region, 5);
    }

    #[tokio::test]
    async fn snapshot_flushes_live_updates_before_reconciliation() {
        let wire = PublicProgressiveActionState {
            entity_id: 1,
            building_entity_id: 2,
            owner_entity_id: 3,
        };
        let model = Model::from_wire(wire, 4);
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![model.clone()], vec![]])
            .into_connection();
        let mut pending = Pending::default();
        pending.upsert(model);
        pending
            .snapshot(&state(db.clone()), &conflict(), vec![], 4, 100)
            .await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("SELECT"));
    }

    #[tokio::test]
    async fn failed_flush_aborts_snapshot() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("failure".into())])
            .into_connection();
        let mut pending = Pending::default();
        pending.upsert(Model::from_wire(
            PublicProgressiveActionState {
                entity_id: 1,
                building_entity_id: 2,
                owner_entity_id: 3,
            },
            4,
        ));
        pending
            .snapshot(&state(db.clone()), &conflict(), vec![], 4, 100)
            .await;
        assert_eq!(db.into_transaction_log().len(), 1);
    }
}
