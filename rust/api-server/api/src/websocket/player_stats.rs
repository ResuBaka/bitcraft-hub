use crate::{
    AppState,
    player_state::stats::{CharacterStatsChange, CharacterStatsDelta},
    websocket::{BroadcastTopics, OutboundWebSocketMessages},
};
use entity::shared::Region;
use game_module::module_bindings::*;
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};

pub(super) fn changed(state: &AppState, player_id: Option<u64>) {
    let topic_id = match player_id {
        Some(id) => match i64::try_from(id) {
            Ok(id) => Some(id),
            Err(_) => return,
        },
        None => None,
    };
    let _ = state.tx.send_if_interested(
        BroadcastTopics::new([("player_stats".into(), topic_id)]),
        || {
            let stats = player_id.and_then(|id| {
                let (region, signed_in) = {
                    let player = state.player_state.get(&i64::try_from(id).ok()?)?;
                    (player.region, player.signed_in)
                };
                Some(state.player_stats.snapshot(region, id, signed_in))
            });
            OutboundWebSocketMessages::PlayerStatsChanged {
                player_id: player_id.map(|id| id.to_string()),
                stats,
            }
        },
    );
}

pub(crate) fn character_changed(
    state: &AppState,
    player_id: Option<i64>,
    delta: Option<CharacterStatsDelta>,
) {
    let _ = state.tx.send_if_interested(
        BroadcastTopics::new([("player_character_stats".into(), player_id)]),
        || OutboundWebSocketMessages::PlayerCharacterStatsChanged {
            player_id: player_id.map(|id| id.to_string()),
            delta,
        },
    );
}

fn character_row_changed(
    state: &AppState,
    region: Region,
    id: u64,
    row: Option<CharacterStatsState>,
) {
    let Some(change) = state.player_stats.character_stats(region, id, row) else {
        return;
    };
    let Ok(id) = i64::try_from(id) else { return };
    if state
        .player_state
        .get(&id)
        .is_some_and(|player| player.region == region)
    {
        let maxima_changed = match &change {
            CharacterStatsChange::Reset => true,
            CharacterStatsChange::Delta(delta) => delta.changes.iter().any(|(index, _)| {
                *index == CharacterStatType::MaxHealth as u32
                    || *index == CharacterStatType::MaxStamina as u32
            }),
        };
        let delta = match change {
            CharacterStatsChange::Reset => None,
            CharacterStatsChange::Delta(delta) => Some(delta),
        };
        character_changed(state, Some(id), delta);
        if maxima_changed {
            changed(state, Some(id as u64));
        }
    }
}

pub(super) fn listen(ctx: &DbConnection, region: Region, state: AppState) {
    macro_rules! listen_table {
        ($table:ident, $update:ident, $convert:path) => {{
            let table = ctx.db.$table();
            let target = state.clone();
            table.on_initial_rows(move |_, rows| {
                for row in rows.iter() {
                    target
                        .player_stats
                        .$update(region, row.entity_id, Some($convert(row)));
                }
            });
            let target = state.clone();
            table.on_insert(move |ctx, row| {
                if !matches!(ctx.event, Event::SubscribeApplied) {
                    target
                        .player_stats
                        .$update(region, row.entity_id, Some($convert(row)));
                    changed(&target, Some(row.entity_id));
                }
            });
            let target = state.clone();
            table.on_update(move |_, _, row| {
                target
                    .player_stats
                    .$update(region, row.entity_id, Some($convert(row)));
                changed(&target, Some(row.entity_id));
            });
            let target = state.clone();
            table.on_delete(move |_, row| {
                target.player_stats.$update(region, row.entity_id, None);
                changed(&target, Some(row.entity_id));
            });
        }};
    }
    listen_table!(health_state, health, Clone::clone);
    listen_table!(stamina_state, stamina, Clone::clone);
    listen_table!(active_buff_state, buffs, std::convert::identity);
    let table = ctx.db.character_stats_state();
    let target = state.clone();
    table.on_initial_owned(move |_, rows| {
        for row in rows {
            let _ = target
                .player_stats
                .character_stats(region, row.entity_id, Some(row));
        }
    });
    let target = state.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            character_row_changed(&target, region, row.entity_id, Some(row.clone()));
        }
    });
    let target = state.clone();
    table.on_update(move |_, _, row| {
        character_row_changed(&target, region, row.entity_id, Some(row.clone()))
    });
    let target = state.clone();
    table.on_delete(move |_, row| character_row_changed(&target, region, row.entity_id, None));
    macro_rules! listen_desc {
        ($table:ident, $update:ident) => {{
            let table = ctx.db.$table();
            let target = state.clone();
            table.on_initial_rows(move |_, rows| {
                for row in rows.iter() {
                    target.player_stats.$update(row.id, Some(row.clone()));
                }
            });
            let target = state.clone();
            table.on_insert(move |ctx, row| {
                if !matches!(ctx.event, Event::SubscribeApplied) {
                    target.player_stats.$update(row.id, Some(row.clone()));
                    changed(&target, None);
                }
            });
            let target = state.clone();
            table.on_update(move |_, _, row| {
                target.player_stats.$update(row.id, Some(row.clone()));
                changed(&target, None);
            });
            let target = state.clone();
            table.on_delete(move |_, row| {
                target.player_stats.$update(row.id, None);
                changed(&target, None);
            });
        }};
    }
    listen_desc!(buff_desc, buff_desc);
    listen_desc!(buff_type_desc, buff_type);

    macro_rules! listen_tool_metadata {
        ($table:ident, $map:ident) => {{
            let table = ctx.db.$table();
            let target = state.clone();
            table.on_initial_rows(move |_, rows| {
                target.player_stats.update_tools(region, |index| {
                    index
                        .$map
                        .extend(rows.iter().map(|row| (row.id, row.clone())));
                });
            });
            let target = state.clone();
            table.on_insert(move |ctx, row| {
                if !matches!(ctx.event, Event::SubscribeApplied) {
                    target.player_stats.update_tools(region, |index| {
                        index.$map.insert(row.id, row.clone());
                    });
                }
            });
            let target = state.clone();
            table.on_update(move |_, _, row| {
                target.player_stats.update_tools(region, |index| {
                    index.$map.insert(row.id, row.clone());
                });
            });
            let target = state.clone();
            table.on_delete(move |_, row| {
                target.player_stats.update_tools(region, |index| {
                    index.$map.remove(&row.id);
                });
            });
        }};
    }
    listen_tool_metadata!(tool_desc, descriptions);
    listen_tool_metadata!(tool_type_desc, types);

    let table = ctx.db.food_desc();
    let target = state.clone();
    table.on_initial_rows(move |_, rows| {
        for row in rows.iter() {
            target
                .player_stats
                .food(region, row.item_id, Some(row.clone()));
        }
    });
    let target = state.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target
                .player_stats
                .food(region, row.item_id, Some(row.clone()));
            changed(&target, None);
        }
    });
    let target = state.clone();
    table.on_update(move |_, _, row| {
        target
            .player_stats
            .food(region, row.item_id, Some(row.clone()));
        changed(&target, None);
    });
    let target = state.clone();
    table.on_delete(move |_, row| {
        target.player_stats.food(region, row.item_id, None);
        changed(&target, None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

    #[tokio::test]
    async fn detailed_delta_only_reaches_its_player_topic() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("player_character_stats", Some(42))])
            .await;
        let delta = CharacterStatsDelta {
            base_revision: 12,
            revision: 19,
            changes: vec![(15, Some(1.2))],
        };
        character_changed(&state, Some(43), Some(delta.clone()));
        character_changed(&state, Some(42), Some(delta));
        let message = rx.try_recv().unwrap();
        assert_eq!(
            serde_json::to_value(message).unwrap(),
            serde_json::json!({
                "t": "PlayerCharacterStatsChanged",
                "c": {"player_id": "42", "delta": {
                    "base_revision": 12, "revision": 19, "changes": [[15, 1.2_f32]]
                }}
            })
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn notifications_only_reach_interested_topics() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("player_stats", Some(42)), ("player_stats", None)])
            .await;
        changed(&state, Some(43));
        changed(&state, Some(42));
        changed(&state, None);
        assert!(
            matches!(rx.try_recv().unwrap(), OutboundWebSocketMessages::PlayerStatsChanged { player_id: Some(id), stats: None } if id == "42")
        );
        assert!(matches!(
            rx.try_recv().unwrap(),
            OutboundWebSocketMessages::PlayerStatsChanged {
                player_id: None,
                stats: None
            }
        ));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn player_notifications_include_the_current_stats_snapshot() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let _client = state
            .subscribe_test_client(&[("player_stats", Some(42))])
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
        state.player_stats.reset(1);
        state.player_stats.ready(1);
        state.player_stats.health(
            1,
            42,
            Some(HealthState {
                entity_id: 42,
                last_health_decrease_timestamp:
                    spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
                health: 37.0,
                died_timestamp: 0,
            }),
        );
        changed(&state, Some(42));
        match rx.try_recv().unwrap() {
            OutboundWebSocketMessages::PlayerStatsChanged {
                player_id,
                stats: Some(stats),
            } => {
                assert_eq!(player_id.as_deref(), Some("42"));
                assert!(stats.available);
                assert_eq!(stats.health, Some(37.0));
            }
            other => panic!("Expected a player stats snapshot, got {other:?}"),
        }
    }
}
