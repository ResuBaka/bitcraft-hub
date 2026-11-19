use crate::{
    AppState,
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
    listen_table!(character_stats_state, character_stats, Clone::clone);
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
