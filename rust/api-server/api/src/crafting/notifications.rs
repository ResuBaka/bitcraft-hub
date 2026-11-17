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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

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
