use crate::AppState;
use crate::websocket::snapshot_write::{self, Writer};
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::ProgressiveActionState;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, sea_query};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

fn progress_only(old: &ProgressiveActionState, new: &ProgressiveActionState) -> bool {
    old.entity_id == new.entity_id && old.building_entity_id == new.building_entity_id
        && old.owner_entity_id == new.owner_entity_id && old.recipe_id == new.recipe_id
        && old.craft_count == new.craft_count && old.function_type == new.function_type
}

pub(crate) fn start_worker_progressive_action_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ProgressiveActionState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("progressive_action_state");
        let on_conflict =
            sea_query::OnConflict::column(::entity::progressive_action_state::Column::EntityId)
                .update_columns([
                    ::entity::progressive_action_state::Column::BuildingEntityId,
                    ::entity::progressive_action_state::Column::FunctionType,
                    ::entity::progressive_action_state::Column::Progress,
                    ::entity::progressive_action_state::Column::RecipeId,
                    ::entity::progressive_action_state::Column::CraftCount,
                    ::entity::progressive_action_state::Column::LastCritOutcome,
                    ::entity::progressive_action_state::Column::OwnerEntityId,
                    ::entity::progressive_action_state::Column::LockExpiration,
                    ::entity::progressive_action_state::Column::Preparation,
                    ::entity::progressive_action_state::Column::Region,
                ])
                .to_owned();

        let mut pending = PendingActions::default();
        let mut changed_buildings = HashSet::new();
        let mut changed_players = HashSet::new();
        let mut progress_updates = HashMap::new();
        loop {
            let mut timer = BatchTimer::new(time_limit);

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break; };
                        timer.start();
                        receive_metrics.record(1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                pending.reconcile_snapshot(&global_app_state, &on_conflict, data, database_name, batch_size).await;
                                changed_buildings.clear();
                                changed_players.clear();
                                progress_updates.clear();
                                crate::crafting::notifications::all(&global_app_state);
                            }
                            SpacetimeUpdateMessages::Update { old, new, database_name, .. } => {
                                if progress_only(&old, &new) {
                                    progress_updates.insert(new.entity_id as i64, new.clone());
                                } else {
                                    changed_buildings.insert(old.building_entity_id as i64);
                                    changed_buildings.insert(new.building_entity_id as i64);
                                    changed_players.insert(old.owner_entity_id as i64);
                                    changed_players.insert(new.owner_entity_id as i64);
                                    progress_updates.remove(&(new.entity_id as i64));
                                }
                                let model = ::entity::progressive_action_state::ModelBuilder::new(new).with_region(database_name).build();
                                pending.upsert(model);
                                if pending.upserts.len() >= batch_size { break; }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                changed_buildings.insert(new.building_entity_id as i64);
                                changed_players.insert(new.owner_entity_id as i64);
                                let model: ::entity::progressive_action_state::Model = ::entity::progressive_action_state::ModelBuilder::new(new).with_region(database_name).build();
                                pending.upsert(model);
                                if pending.upserts.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                changed_buildings.insert(delete.building_entity_id as i64);
                                changed_players.insert(delete.owner_entity_id as i64);
                                progress_updates.remove(&(delete.entity_id as i64));
                                pending.delete(delete.entity_id as i64);
                                if pending.deletes.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if pending.flush(&global_app_state, &on_conflict).await {
                crate::crafting::notifications::progress(&global_app_state, &mut progress_updates).await;
                crate::crafting::notifications::players(&global_app_state, &mut changed_players);
                crate::crafting::notifications::buildings(&global_app_state, &mut changed_buildings).await;
            } else { changed_buildings.clear(); changed_players.clear(); progress_updates.clear(); }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if pending.upserts.is_empty() && pending.deletes.is_empty() && rx.is_closed() && rx.is_empty()
            {
                break;
            }
        }
    });
}

#[derive(Default)]
struct PendingActions {
    upserts: HashMap<i64, ::entity::progressive_action_state::Model>,
    deletes: HashSet<i64>,
    rows: Vec<::entity::progressive_action_state::ActiveModel>,
    delete_ids: Vec<i64>,
}

impl PendingActions {
    fn upsert(&mut self, model: ::entity::progressive_action_state::Model) {
        self.deletes.remove(&model.entity_id);
        self.upserts.insert(model.entity_id, model);
    }

    fn delete(&mut self, id: i64) {
        self.upserts.remove(&id);
        self.deletes.insert(id);
    }

    async fn flush(&mut self, state: &AppState, conflict: &sea_query::OnConflict) -> bool {
        let mut success = true;
        self.rows.extend(
            self.upserts
                .drain()
                .map(|(_, model)| model.into_active_model()),
        );
        if !self.rows.is_empty() {
            success =
                insert_multiple_progressive_action_state(state, conflict, &mut self.rows).await;
        }
        self.delete_ids.extend(self.deletes.drain());
        for ids in self.delete_ids.chunks(1000) {
            if let Err(error) = ::entity::progressive_action_state::Entity::delete_many()
                .filter(
                    ::entity::progressive_action_state::Column::EntityId.is_in(ids.iter().copied()),
                )
                .exec(&state.conn)
                .await
            {
                success = false;
                tracing::error!(ids = ?ids, %error, "Could not delete ProgressiveActionState");
            }
        }
        self.delete_ids.clear();
        // Preserve best-effort live persistence: failed operations are logged,
        // not retained forever and allowed to prevent worker shutdown.
        success
    }

    async fn reconcile_snapshot(
        &mut self,
        state: &AppState,
        conflict: &sea_query::OnConflict,
        data: Vec<ProgressiveActionState>,
        region: entity::shared::Region,
        batch_size: usize,
    ) {
        if !self.flush(state, conflict).await {
            tracing::error!(
                region,
                "Could not flush progressive actions before snapshot"
            );
            return;
        }
        if let Err(error) = crate::websocket::initial_sync::reconcile::<
            ::entity::progressive_action_state::Entity,
            _,
            _,
            _,
            _,
            _,
        >(
            &state.conn,
            ::entity::progressive_action_state::Column::Region,
            region,
            Writer::Fixed(snapshot_write::progressive_actions),
            batch_size,
            data.into_iter().map(|value| {
                ::entity::progressive_action_state::ModelBuilder::new(value)
                    .with_region(region)
                    .build()
            }),
            |model| model.entity_id,
            |ids| {
                sea_orm::Condition::all().add(
                    ::entity::progressive_action_state::Column::EntityId.is_in(ids.iter().copied()),
                )
            },
            |_, _| {},
        )
        .await
        {
            tracing::error!(%error, region, "Could not reconcile progressive actions");
        }
    }
}

async fn insert_multiple_progressive_action_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::progressive_action_state::ActiveModel>,
) -> bool {
    let insert = ::entity::progressive_action_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    let success = insert.is_ok();
    if let Err(err) = insert {
        tracing::error!("Error inserting ProgressiveActionState: {}", err)
    }

    messages.clear();
    success
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, DbErr, MockDatabase, MockExecResult};

    fn wire(id: u64, progress: i32) -> ProgressiveActionState {
        ProgressiveActionState {
            entity_id: id,
            building_entity_id: 2,
            function_type: 3,
            progress,
            recipe_id: 4,
            craft_count: 5,
            last_crit_outcome: 0,
            owner_entity_id: 6,
            lock_expiration: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            preparation: false,
        }
    }

    fn model(
        id: u64,
        progress: i32,
        region: entity::shared::Region,
    ) -> entity::progressive_action_state::Model {
        entity::progressive_action_state::ModelBuilder::new(wire(id, progress))
            .with_region(region)
            .build()
    }

    fn conflict() -> sea_query::OnConflict {
        use entity::progressive_action_state::Column;
        sea_query::OnConflict::column(Column::EntityId)
            .update_columns([Column::Progress, Column::Region])
            .to_owned()
    }

    #[tokio::test]
    async fn live_progress_notifies_the_claim_after_database_write() {
        use crate::websocket::OutboundWebSocketMessages;
        let building = entity::building_state::Model {
            entity_id: 2, claim_entity_id: 42, direction_index: 0,
            building_description_id: 3, constructed_by_player_entity_id: 0, region: 7,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![model(1, 200, 7)]])
            .append_query_results([vec![building]])
            .into_connection();
        let (out, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        let _client = state.subscribe_test_client(&[("claim_crafting", None), ("claim_craft_progress", Some(42)), ("player_craft_progress", Some(6))]).await;
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        start_worker_progressive_action_state(state.clone(), rx, 100, Duration::from_secs(60));
        tx.send(SpacetimeUpdateMessages::Update { old: wire(1, 100), new: wire(1, 200), database_name: 7, event: None }).unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.shutdown.drain()).await.unwrap();
        assert!(matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::CraftProgressChanged { claim_id: Some(claim), player_id, craft_id, progress: 200, .. } if claim == "42" && player_id == "6" && craft_id == "1"));
        assert!(notifications.try_recv().is_err());
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("SELECT"));
    }

    #[test]
    fn progress_updates_do_not_hide_structural_changes() {
        let old = wire(1, 10);
        let mut new = wire(1, 20);
        assert!(progress_only(&old, &new));
        new.recipe_id += 1;
        assert!(!progress_only(&old, &new));
        new = wire(1, 20);
        new.owner_entity_id += 1;
        assert!(!progress_only(&old, &new));
    }

    fn state(db: sea_orm::DatabaseConnection) -> AppState {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        AppState::new(db, tx, Default::default())
    }

    #[test]
    fn pending_actions_coalesce_and_supersede_by_primary_key() {
        let mut pending = PendingActions::default();
        pending.upsert(model(1, 1, 7));
        pending.upsert(model(2, 2, 7));
        for progress in 2..=100 {
            pending.upsert(model(1, progress, 8));
        }
        assert_eq!(pending.upserts.len(), 2);
        assert_eq!(pending.upserts[&1], model(1, 100, 8));
        pending.delete(1);
        pending.delete(1);
        assert_eq!(pending.deletes.len(), 1);
        assert!(!pending.upserts.contains_key(&1));
        pending.upsert(model(1, 200, 9));
        assert!(pending.deletes.is_empty());
        assert_eq!(pending.upserts[&1], model(1, 200, 9));
        assert_eq!(pending.upserts[&2], model(2, 2, 7));
    }

    #[tokio::test]
    async fn worker_drains_partial_batch_on_shutdown_without_waiting_for_timer() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![model(1, 200, 8)]])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let state = state(db.clone());
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        start_worker_progressive_action_state(state.clone(), rx, 100, Duration::from_secs(60));
        tx.send(SpacetimeUpdateMessages::Insert {
            new: wire(1, 100),
            database_name: 7,
            event: None,
        })
        .unwrap();
        tx.send(SpacetimeUpdateMessages::Update {
            old: wire(1, 100),
            new: wire(1, 200),
            database_name: 8,
            event: None,
        })
        .unwrap();
        tx.send(SpacetimeUpdateMessages::Remove {
            delete: wire(2, 50),
            database_name: 7,
            event: None,
        })
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.shutdown.drain())
            .await
            .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("DELETE FROM"));
    }

    #[tokio::test]
    async fn snapshot_reads_follow_pending_live_writes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![model(1, 200, 7)], vec![]])
            .into_connection();
        let mut pending = PendingActions::default();
        pending.upsert(model(1, 200, 7));
        pending
            .reconcile_snapshot(&state(db.clone()), &conflict(), vec![], 7, 100)
            .await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("SELECT"));
        assert!(pending.upserts.is_empty() && pending.rows.is_empty());
    }

    #[tokio::test]
    async fn failed_live_flush_aborts_snapshot_and_keeps_best_effort_policy() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("live failure".into())])
            .into_connection();
        let mut pending = PendingActions::default();
        pending.upsert(model(1, 200, 7));
        pending
            .reconcile_snapshot(&state(db.clone()), &conflict(), vec![wire(1, 300)], 7, 100)
            .await;
        assert_eq!(
            db.into_transaction_log().len(),
            1,
            "no snapshot queries after failed flush"
        );
        assert!(pending.upserts.is_empty() && pending.rows.is_empty());
    }

    #[tokio::test]
    async fn failed_live_delete_aborts_snapshot() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_errors([DbErr::Custom("delete failure".into())])
            .into_connection();
        let mut pending = PendingActions::default();
        pending.delete(1);
        pending
            .reconcile_snapshot(&state(db.clone()), &conflict(), vec![wire(1, 300)], 7, 100)
            .await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1, "no snapshot queries after failed delete");
        assert!(format!("{log:?}").contains("DELETE FROM"));
        assert!(pending.deletes.is_empty() && pending.delete_ids.is_empty());
    }

    #[tokio::test]
    async fn failed_snapshot_upsert_never_deletes_stale_rows() {
        let keys = std::collections::BTreeMap::from([(
            "entity_id".to_string(),
            sea_orm::Value::from(9i64),
        )]);
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![keys]])
            .append_exec_errors([DbErr::Custom("snapshot failure".into())])
            .into_connection();
        PendingActions::default()
            .reconcile_snapshot(&state(db.clone()), &conflict(), vec![wire(1, 300)], 7, 100)
            .await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(!format!("{log:?}").contains("DELETE FROM"));
    }

    #[tokio::test]
    async fn failed_snapshot_key_read_never_writes_or_deletes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("read failure".into())])
            .into_connection();
        PendingActions::default()
            .reconcile_snapshot(&state(db.clone()), &conflict(), vec![wire(1, 300)], 7, 100)
            .await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(format!("{log:?}").contains("SELECT"));
    }
}
