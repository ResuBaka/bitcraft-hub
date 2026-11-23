use crate::{
    AppState,
    websocket::{BroadcastTopics, OutboundWebSocketMessages},
};
use game_module::module_bindings::{PassiveCraftState, PassiveCraftStatus, ProgressiveActionState};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::collections::HashSet;

pub(crate) fn all(state: &AppState) {
    let _ = state
        .tx
        .send(OutboundWebSocketMessages::ClaimCraftingChanged { claim_id: None });
    let _ = state
        .tx
        .send(OutboundWebSocketMessages::PlayerCraftingChanged { player_id: None });
}

pub(crate) fn players(state: &AppState, ids: &mut HashSet<i64>) {
    for id in ids.drain() {
        if id != 0 {
            let _ = state
                .tx
                .send(OutboundWebSocketMessages::PlayerCraftingChanged {
                    player_id: Some(id.to_string()),
                });
        }
    }
}

pub(crate) async fn progress(state: &AppState, updates: &mut HashMap<i64, ProgressiveActionState>) {
    if updates.is_empty() {
        return;
    }
    // Check after persistence: a client can subscribe while this batch is pending.
    if !state.tx.has_subscribers() {
        updates.clear();
        return;
    }
    let claim_views = state
        .tx
        .is_interested(&BroadcastTopics::new([("claim_crafting".into(), None)]));
    let mut claims = HashMap::new();
    let mut unknown_claims = false;
    if claim_views {
        let ids: HashSet<_> = updates
            .values()
            .map(|row| row.building_entity_id as i64)
            .collect();
        match entity::building_state::Entity::find()
            .filter(entity::building_state::Column::EntityId.is_in(ids.iter().copied()))
            .all(&state.conn)
            .await
        {
            Ok(rows) => {
                unknown_claims = rows.len() < ids.len();
                claims.extend(
                    rows.into_iter()
                        .map(|row| (row.entity_id, row.claim_entity_id)),
                );
            }
            Err(error) => {
                tracing::warn!(%error, "Could not resolve crafting progress buildings");
                unknown_claims = true;
            }
        }
    }
    for (_, row) in updates.drain() {
        let Some(timestamp) = chrono::DateTime::from_timestamp_micros(
            row.lock_expiration.to_micros_since_unix_epoch(),
        ) else {
            all(state);
            continue;
        };
        let claim_id = claims
            .get(&(row.building_entity_id as i64))
            .filter(|id| **id != 0)
            .map(ToString::to_string);
        let _ = state
            .tx
            .send(OutboundWebSocketMessages::CraftProgressChanged {
                claim_id,
                player_id: row.owner_entity_id.to_string(),
                craft_id: row.entity_id.to_string(),
                progress: row.progress,
                lock_expiration: timestamp.to_rfc3339(),
            });
    }
    if unknown_claims {
        all(state);
    }
}

pub(crate) async fn passive(state: &AppState, updates: &mut HashMap<i64, PassiveCraftState>) {
    if updates.is_empty() {
        return;
    }
    if !state.tx.has_subscribers() {
        updates.clear();
        return;
    }
    let claim_views = state
        .tx
        .is_interested(&BroadcastTopics::new([("claim_crafting".into(), None)]));
    let mut claims = HashMap::new();
    let mut unknown_claims = false;
    if claim_views {
        let ids: HashSet<_> = updates
            .values()
            .map(|row| row.building_entity_id as i64)
            .collect();
        match entity::building_state::Entity::find()
            .filter(entity::building_state::Column::EntityId.is_in(ids.iter().copied()))
            .all(&state.conn)
            .await
        {
            Ok(rows) => {
                unknown_claims = rows.len() < ids.len();
                claims.extend(
                    rows.into_iter()
                        .map(|row| (row.entity_id, row.claim_entity_id)),
                );
            }
            Err(error) => {
                tracing::warn!(%error, "Could not resolve passive craft buildings");
                unknown_claims = true;
            }
        }
    }
    for (_, row) in updates.drain() {
        let Some(timestamp) =
            chrono::DateTime::from_timestamp_micros(row.timestamp.to_micros_since_unix_epoch())
        else {
            all(state);
            continue;
        };
        let claim_id = claims
            .get(&(row.building_entity_id as i64))
            .filter(|id| **id != 0)
            .map(ToString::to_string);
        let status = match row.status {
            PassiveCraftStatus::Queued => "Queued",
            PassiveCraftStatus::Processing => "Processing",
            PassiveCraftStatus::Complete => "Complete",
        };
        let _ = state
            .tx
            .send(OutboundWebSocketMessages::PassiveCraftChanged {
                claim_id,
                player_id: row.owner_entity_id.to_string(),
                craft_id: row.entity_id.to_string(),
                status: status.into(),
                slot: row.slot,
                timestamp: timestamp.to_rfc3339(),
            });
    }
    if unknown_claims {
        all(state);
    }
}

pub(crate) fn claim(state: &AppState, claim_id: i64) {
    if claim_id != 0 {
        let _ = state
            .tx
            .send(OutboundWebSocketMessages::ClaimCraftingChanged {
                claim_id: Some(claim_id.to_string()),
            });
    }
}

pub(crate) async fn buildings(state: &AppState, ids: &mut HashSet<i64>) {
    if ids.is_empty() {
        return;
    }
    // The global topic is subscribed by every crafting view and doubles as an
    // interest check, avoiding a query when no crafting view is open.
    if !state
        .tx
        .is_interested(&BroadcastTopics::new([("claim_crafting".into(), None)]))
    {
        ids.clear();
        return;
    }
    let count = ids.len();
    match entity::building_state::Entity::find()
        .filter(entity::building_state::Column::EntityId.is_in(ids.drain()))
        .all(&state.conn)
        .await
    {
        Ok(rows) => {
            let missing = rows.len() < count;
            for claim_id in rows
                .into_iter()
                .map(|row| row.claim_entity_id)
                .collect::<HashSet<_>>()
            {
                claim(state, claim_id);
            }
            // A new/deleted building might not yet be in the local DB.
            if missing {
                all(state);
            }
        }
        Err(error) => {
            tracing::warn!(%error, "Could not resolve crafting notification buildings");
            all(state);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../../benches/craft-notification-interest/replay.rs"]
mod replay;

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, DbErr, MockDatabase};
    use spacetimedb_sdk::Timestamp;
    use std::time::Duration;

    fn row(progress: i32) -> ProgressiveActionState {
        ProgressiveActionState {
            entity_id: 17,
            building_entity_id: 2,
            function_type: 3,
            progress,
            recipe_id: 4,
            craft_count: 5,
            last_crit_outcome: 0,
            owner_entity_id: 6,
            lock_expiration: Timestamp::from_micros_since_unix_epoch(1_000_001),
            preparation: false,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn mid_batch_subscriber_receives_last_progress_only_if_persisted() {
        use crate::websocket::{SpacetimeUpdateMessages, worker_channel::shutdown_channel};
        for success in [true, false] {
            let mock = MockDatabase::new(DbBackend::Postgres);
            let db = if success {
                let model = entity::progressive_action_state::ModelBuilder::new(row(3))
                    .with_region(1)
                    .build();
                mock.append_query_results([vec![model]])
            } else {
                mock.append_query_errors([DbErr::Custom("failed upsert".into())])
            }
            .into_connection();
            let (out, mut notifications) = tokio::sync::mpsc::unbounded_channel();
            let state = AppState::new(db.clone(), out, Default::default());
            let (tx, rx) = shutdown_channel(&state.shutdown);
            crate::crafting::bitcraft::start_worker_progressive_action_state(
                state.clone(),
                rx,
                100,
                Duration::from_secs(60),
            );
            for p in [2, 3] {
                tx.send(SpacetimeUpdateMessages::Update {
                    old: row(p - 1),
                    new: row(p),
                    database_name: 1,
                    event: None,
                })
                .unwrap();
            }
            // The paused runtime advances to this short sleep only once the
            // worker is parked again. Its coalesced update is still pending.
            tokio::time::sleep(Duration::from_millis(1)).await;
            let client = state
                .subscribe_test_client(&[("player_craft_progress", Some(6))])
                .await;
            state.shutdown.drain().await;
            if success {
                let msg = notifications.try_recv().unwrap();
                state
                    .clients_state
                    .broadcast_topics(msg.topics().unwrap(), &msg)
                    .await;
                assert!(matches!(
                    client.try_recv().unwrap(),
                    Some(OutboundWebSocketMessages::CraftProgressChanged {
                        claim_id: None, player_id, craft_id, progress: 3, lock_expiration,
                    }) if player_id == "6" && craft_id == "17" && lock_expiration == "1970-01-01T00:00:01.000001+00:00"
                ));
            }
            assert!(notifications.try_recv().is_err());
            assert!(client.try_recv().unwrap().is_none());
            let log = db.into_transaction_log();
            assert_eq!(log.len(), 1);
            assert!(log[0].statements()[0].sql.starts_with("INSERT"));
        }
    }

    #[tokio::test]
    async fn drains_progress_and_passive_deltas_without_subscribers() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let mut updates = HashMap::from([(17, row(2))]);
        let mut passive_updates = HashMap::from([(
            8,
            PassiveCraftState {
                entity_id: 8,
                owner_entity_id: 6,
                recipe_id: 3,
                building_entity_id: 2,
                timestamp: Timestamp::from_micros_since_unix_epoch(1),
                status: PassiveCraftStatus::Complete,
                slot: None,
            },
        )]);
        progress(&state, &mut updates).await;
        passive(&state, &mut passive_updates).await;
        assert!(updates.is_empty() && passive_updates.is_empty());
        assert!(rx.try_recv().is_err());
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn routes_changed_buildings_to_claim_and_falls_back_for_missing() {
        let building = entity::building_state::Model {
            entity_id: 2,
            claim_entity_id: 42,
            direction_index: 0,
            building_description_id: 3,
            constructed_by_player_entity_id: 0,
            region: 1,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![building]])
            .into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("claim_crafting", None), ("claim_crafting", Some(42))])
            .await;
        let mut ids = HashSet::from([2, 3]);
        buildings(&state, &mut ids).await;
        assert!(ids.is_empty());
        assert!(
            matches!(rx.try_recv().unwrap(), OutboundWebSocketMessages::ClaimCraftingChanged { claim_id: Some(id) } if id == "42")
        );
        assert!(matches!(
            rx.try_recv().unwrap(),
            OutboundWebSocketMessages::ClaimCraftingChanged { claim_id: None }
        ));
    }

    #[tokio::test]
    async fn skips_database_lookup_without_crafting_subscribers() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let mut ids = HashSet::from([2]);
        buildings(&state, &mut ids).await;
        assert!(ids.is_empty());
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn player_notifications_are_scoped_to_changed_owner() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("player_crafting", Some(42))])
            .await;
        let mut owners = HashSet::from([42, 43]);
        players(&state, &mut owners);
        assert!(owners.is_empty());
        assert!(
            matches!(rx.try_recv().unwrap(), OutboundWebSocketMessages::PlayerCraftingChanged { player_id: Some(id) } if id == "42")
        );
        assert!(rx.try_recv().is_err());
    }
}
