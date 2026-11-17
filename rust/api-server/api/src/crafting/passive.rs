use crate::{
    AppState,
    websocket::{
        SpacetimeUpdateMessages, WorkerReceiveMetrics, batched_worker::BatchTimer,
        worker_channel::UnboundedReceiver,
    },
};
use entity::passive_craft_state::{Column, Entity, Model};
use game_module::module_bindings::PassiveCraftState;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, sea_query};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

#[derive(Default)]
struct Pending {
    upserts: HashMap<i64, Model>,
    deletes: HashSet<i64>,
}
impl Pending {
    fn upsert(&mut self, row: Model) {
        self.deletes.remove(&row.entity_id);
        self.upserts.insert(row.entity_id, row);
    }
    fn delete(&mut self, id: i64) {
        self.upserts.remove(&id);
        self.deletes.insert(id);
    }
    async fn flush(&mut self, state: &AppState, conflict: &sea_query::OnConflict) -> bool {
        let rows: Vec<_> = self
            .upserts
            .drain()
            .map(|(_, row)| row.into_active_model())
            .collect();
        let mut success = true;
        if !rows.is_empty()
            && let Err(error) = Entity::insert_many(rows)
                .on_conflict(conflict.clone())
                .exec(&state.conn)
                .await
        {
            tracing::error!(%error, "Could not persist passive crafts");
            success = false;
        }
        let ids: Vec<_> = self.deletes.drain().collect();
        for chunk in ids.chunks(1000) {
            if let Err(error) = Entity::delete_many()
                .filter(Column::EntityId.is_in(chunk.iter().copied()))
                .exec(&state.conn)
                .await
            {
                tracing::error!(%error, "Could not delete passive crafts");
                success = false;
            }
        }
        success
    }
    async fn snapshot(
        &mut self,
        state: &AppState,
        conflict: &sea_query::OnConflict,
        rows: Vec<PassiveCraftState>,
        region: entity::shared::Region,
        batch_size: usize,
    ) {
        if !self.flush(state, conflict).await {
            tracing::error!(region, "Passive craft snapshot aborted after failed flush");
            return;
        }
        if let Err(error) = crate::websocket::initial_sync::reconcile::<Entity, _, _, _, _, _>(
            &state.conn,
            Column::Region,
            region,
            conflict,
            batch_size,
            rows.into_iter().map(|row| Model::from_wire(row, region)),
            |row| row.entity_id,
            |ids| sea_orm::Condition::all().add(Column::EntityId.is_in(ids.iter().copied())),
            |_, _| {},
        )
        .await
        {
            tracing::error!(%error, region, "Could not reconcile passive crafts");
        }
    }
}
pub(crate) fn start_worker(
    state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PassiveCraftState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    state.shutdown.clone().spawn_worker(async move {
        let metrics = WorkerReceiveMetrics::new("passive_craft_state");
        let conflict = sea_query::OnConflict::column(Column::EntityId).update_columns([
            Column::OwnerEntityId, Column::RecipeId, Column::BuildingEntityId, Column::Timestamp, Column::Status, Column::Slot, Column::Region,
        ]).to_owned();
        let mut pending = Pending::default();
        let mut changed_buildings = HashSet::new();
        let mut changed_players = HashSet::new();
        let mut passive_updates = HashMap::new();
        loop {
            let mut timer = BatchTimer::new(time_limit);
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break; }; timer.start(); metrics.record(1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => { pending.snapshot(&state, &conflict, data, database_name, batch_size).await; changed_buildings.clear(); changed_players.clear(); passive_updates.clear(); crate::crafting::notifications::all(&state); },
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => { changed_buildings.insert(new.building_entity_id as i64); changed_players.insert(new.owner_entity_id as i64); pending.upsert(Model::from_wire(new, database_name)); },
                            SpacetimeUpdateMessages::Update { old, new, database_name, .. } => {
                                if old.entity_id == new.entity_id && old.owner_entity_id == new.owner_entity_id && old.building_entity_id == new.building_entity_id && old.recipe_id == new.recipe_id {
                                    passive_updates.insert(new.entity_id as i64, new.clone());
                                } else {
                                    changed_buildings.insert(old.building_entity_id as i64); changed_buildings.insert(new.building_entity_id as i64); changed_players.insert(old.owner_entity_id as i64); changed_players.insert(new.owner_entity_id as i64); passive_updates.remove(&(new.entity_id as i64));
                                }
                                pending.upsert(Model::from_wire(new, database_name));
                            },
                            SpacetimeUpdateMessages::Remove { delete, .. } => { changed_buildings.insert(delete.building_entity_id as i64); changed_players.insert(delete.owner_entity_id as i64); passive_updates.remove(&(delete.entity_id as i64)); pending.delete(delete.entity_id as i64); },
                        }
                        if pending.upserts.len() >= batch_size || pending.deletes.len() >= batch_size { break; }
                    }
                    _ = timer.elapsed() => break,
                    else => break,
                }
            }
            if pending.flush(&state, &conflict).await { crate::crafting::notifications::passive(&state, &mut passive_updates).await; crate::crafting::notifications::players(&state, &mut changed_players); crate::crafting::notifications::buildings(&state, &mut changed_buildings).await; } else { changed_buildings.clear(); changed_players.clear(); passive_updates.clear(); }
            if rx.is_closed() && rx.is_empty() { break; }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::PassiveCraftStatus;
    use sea_orm::{DbBackend, DbErr, MockDatabase};

    fn wire(id: u64) -> PassiveCraftState {
        PassiveCraftState {
            entity_id: id,
            owner_entity_id: 2,
            recipe_id: 3,
            building_entity_id: 4,
            timestamp: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            status: PassiveCraftStatus::Queued,
            slot: None,
        }
    }
    fn state(db: sea_orm::DatabaseConnection) -> AppState {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        AppState::new(db, tx, Default::default())
    }
    fn conflict() -> sea_query::OnConflict {
        sea_query::OnConflict::column(Column::EntityId)
            .update_columns([Column::Status, Column::Region])
            .to_owned()
    }
    #[test]
    fn coalesces_updates_and_deletes() {
        let mut pending = Pending::default();
        pending.upsert(Model::from_wire(wire(1), 4));
        pending.delete(1);
        assert!(pending.upserts.is_empty());
        pending.upsert(Model::from_wire(wire(1), 5));
        assert!(pending.deletes.is_empty());
        assert_eq!(pending.upserts[&1].region, 5);
    }
    #[tokio::test]
    async fn snapshot_flushes_before_reconciliation() {
        let model = Model::from_wire(wire(1), 4);
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
    async fn failed_flush_prevents_snapshot_reconciliation() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("failure".into())])
            .into_connection();
        let mut pending = Pending::default();
        pending.upsert(Model::from_wire(wire(1), 4));
        pending
            .snapshot(&state(db.clone()), &conflict(), vec![wire(1)], 4, 100)
            .await;
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[tokio::test]
    async fn live_update_notifies_claim_after_persistence() {
        use crate::websocket::OutboundWebSocketMessages;
        let project = Model::from_wire(wire(1), 4);
        let building = entity::building_state::Model {
            entity_id: 4,
            claim_entity_id: 42,
            direction_index: 0,
            building_description_id: 3,
            constructed_by_player_entity_id: 0,
            region: 4,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![project]])
            .append_query_results([vec![building]])
            .into_connection();
        let (out, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        let _client = state
            .subscribe_test_client(&[("claim_crafting", None), ("claim_crafting", Some(42))])
            .await;
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        start_worker(state.clone(), rx, 100, Duration::from_secs(60));
        tx.send(SpacetimeUpdateMessages::Insert {
            new: wire(1),
            database_name: 4,
            event: None,
        })
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.shutdown.drain())
            .await
            .unwrap();
        assert!(
            matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::ClaimCraftingChanged { claim_id: Some(id) } if id == "42")
        );
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("SELECT"));
    }

    #[tokio::test]
    async fn passive_status_update_sends_delta_after_persistence() {
        use crate::websocket::OutboundWebSocketMessages;
        let mut updated = wire(1);
        updated.status = PassiveCraftStatus::Processing;
        updated.slot = Some(2);
        let building = entity::building_state::Model {
            entity_id: 4,
            claim_entity_id: 42,
            direction_index: 0,
            building_description_id: 3,
            constructed_by_player_entity_id: 0,
            region: 4,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![Model::from_wire(updated.clone(), 4)]])
            .append_query_results([vec![building]])
            .into_connection();
        let (out, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, out, Default::default());
        let _client = state
            .subscribe_test_client(&[("claim_crafting", None), ("claim_passive_craft", Some(42))])
            .await;
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        start_worker(state.clone(), rx, 100, Duration::from_secs(60));
        tx.send(SpacetimeUpdateMessages::Update {
            old: wire(1),
            new: updated,
            database_name: 4,
            event: None,
        })
        .unwrap();
        tokio::time::timeout(Duration::from_secs(1), state.shutdown.drain())
            .await
            .unwrap();
        assert!(
            matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::PassiveCraftChanged { claim_id: Some(id), status, slot: Some(2), .. } if id == "42" && status == "Processing")
        );
        assert!(notifications.try_recv().is_err());
    }
}
