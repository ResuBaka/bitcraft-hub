use crate::{
    AppState,
    websocket::{BroadcastTopics, OutboundWebSocketMessages},
};
use entity::shared::Region;
use game_module::module_bindings::*;
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};

pub(super) fn changed(state: &AppState, player_id: Option<u64>) {
    let topic = match player_id {
        Some(id) => match i64::try_from(id) {
            Ok(id) => Some(id),
            Err(_) => return,
        },
        None => None,
    };
    let _ = state.tx.send_if_interested(
        BroadcastTopics::new([("player_activity".into(), topic)]),
        || OutboundWebSocketMessages::PlayerActivityChanged {
            player_id: player_id.map(|id| id.to_string()),
            activity: player_id
                .and_then(|id| crate::player_state::activity::player_snapshot(state, id)),
        },
    );
}

pub(super) fn listen(ctx: &DbConnection, region: Region, state: AppState) {
    let table = ctx.db.player_action_state();
    let target = state.clone();
    table.on_initial_rows(move |_, rows| {
        for row in rows.iter() {
            target.player_activity.insert(region, row.clone());
        }
    });
    let target = state.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target.player_activity.insert(region, row.clone());
            changed(&target, Some(row.entity_id));
        }
    });
    let target = state.clone();
    table.on_update(move |_, old, row| {
        if old.entity_id != row.entity_id || old.auto_id != row.auto_id {
            target
                .player_activity
                .remove(region, old.entity_id, old.auto_id);
            changed(&target, Some(old.entity_id));
        }
        target.player_activity.insert(region, row.clone());
        changed(&target, Some(row.entity_id));
    });
    let target = state;
    table.on_delete(move |_, row| {
        target
            .player_activity
            .remove(region, row.entity_id, row.auto_id);
        changed(&target, Some(row.entity_id));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

    #[tokio::test]
    async fn sends_current_actions_only_to_the_requested_player_topic() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("player_activity", Some(42))])
            .await;
        state.player_state.insert(
            42,
            entity::player_state::Model {
                entity_id: 42,
                region: 1,
                time_played: 0,
                session_start_timestamp: 0,
                time_signed_in: 0,
                sign_in_timestamp: 0,
                signed_in: true,
                traveler_tasks_expiration: 0,
                teleport_location: entity::player_state::TeleportLocation {
                    location: entity::player_state::OffsetCoordinatesSmallMessage {
                        x: 0,
                        z: 0,
                        dimension: 1,
                    },
                    location_type: entity::player_state::TeleportLocationType::BirthLocation,
                },
            },
        );
        state.player_activity.reset(1);
        state.player_activity.insert(
            1,
            PlayerActionState {
                auto_id: 10,
                chunk_index: 0,
                entity_id: 42,
                start_time: 1000,
                duration: 2000,
                target: None,
                recipe_id: Some(7),
                action_type: PlayerActionType::Extract,
                layer: PlayerActionLayer::Base,
                last_action_result: PlayerActionResult::Success,
                client_cancel: false,
                was_consumed: false,
                pad_1: 0,
                pad_2: 0,
                pad_3: 0,
            },
        );
        state.player_activity.ready(1);
        changed(&state, Some(43));
        changed(&state, Some(42));
        match rx.try_recv().unwrap() {
            OutboundWebSocketMessages::PlayerActivityChanged {
                player_id,
                activity: Some(activity),
            } => {
                assert_eq!(player_id.as_deref(), Some("42"));
                let actions = activity.actions.unwrap();
                assert_eq!(actions[0].action_type, "Extract");
                assert_eq!(actions[0].recipe_id, Some(7));
            }
            other => panic!("Expected an activity snapshot, got {other:?}"),
        }
        assert!(rx.try_recv().is_err());
    }
}
