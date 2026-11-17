use crate::AppState;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{BroadcastTopics, OutboundWebSocketMessages, WorkerReceiveMetrics};
use entity::mobile_entity_state;
use game_module::module_bindings::MobileEntityState;

fn apply_location(state: &AppState, model: mobile_entity_state::Model) {
    state
        .mobile_entity_state
        .insert(model.entity_id, model.clone());
    let _ = state
        .tx
        .send_if_interested(BroadcastTopics::player_location(model.entity_id), || {
            OutboundWebSocketMessages::PlayerLocation(crate::player_state::resolve_player_location(
                state,
                model.entity_id,
                Some(model.clone()),
            ))
        });
    let _ = state
        .tx
        .send_if_interested(BroadcastTopics::mobile_entity(model.entity_id), || {
            OutboundWebSocketMessages::MobileEntityState(model)
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cache_updates_without_listeners_and_only_requested_payload_is_queued() {
        let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(Default::default(), tx, Default::default());
        let mut model = mobile_entity_state::Model {
            entity_id: 7,
            chunk_index: 5,
            timestamp: 0,
            location_x: 1000,
            location_z: 2000,
            destination_x: 1000,
            destination_z: 2000,
            dimension: 1,
            is_walking: false,
            region: 7,
        };
        apply_location(&state, model.clone());
        assert_eq!(state.mobile_entity_state.get(&7).unwrap().location_x, 1000);
        assert!(notifications.try_recv().is_err());
        let _subscriber = state
            .subscribe_test_client(&[("mobile_entity_state", Some(7))])
            .await;
        model.location_x = 2000;
        apply_location(&state, model.clone());
        assert!(
            matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::MobileEntityState(value) if value.location_x == 2000)
        );
        assert!(
            notifications.try_recv().is_err(),
            "player_location was not subscribed"
        );
        state
            .clients_state
            .add_topic_to_client(
                &"test-subscriber".into(),
                &"player_location".into(),
                Some(7),
            )
            .await;
        apply_location(&state, model);
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::PlayerLocation(_)
        ));
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::MobileEntityState(_)
        ));
        assert!(notifications.try_recv().is_err());
    }
}

pub(crate) fn start_worker_mobile_entity_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<crate::websocket::SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tracing::info!(
        worker = "mobile_entity_state",
        "Scheduling mobile entity worker"
    );
    let shutdown = global_app_state.shutdown.clone();
    let worker = shutdown.spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("mobile_entity_state");
        tracing::info!(
            worker = "mobile_entity_state",
            backend = crate::websocket::worker_channel::BACKEND,
            "Mobile entity worker started"
        );
        let mut buffer = Vec::new();
        loop {
            buffer.shrink_to(500);
            let count = rx.recv_many(&mut buffer, 500).await;
            if count == 0 {
                if global_app_state.shutdown.stop.is_cancelled() {
                    tracing::info!(
                        worker = "mobile_entity_state",
                        channel_closed = rx.is_closed(),
                        "Mobile entity worker input drained during shutdown; exiting"
                    );
                } else {
                    tracing::error!(
                        worker = "mobile_entity_state",
                        channel_closed = rx.is_closed(),
                        "Mobile entity worker input ended unexpectedly; exiting"
                    );
                }
                break;
            }
            receive_metrics.record(count);
            for msg in buffer.drain(..) {
                match msg {
                    crate::websocket::SpacetimeUpdateMessages::Initial {
                        data,
                        database_name,
                        ..
                    } => {
                        for entry in data {
                            let model: mobile_entity_state::Model =
                                ::entity::mobile_entity_state::ModelBuilder::new(entry)
                                    .with_region(database_name)
                                    .build();
                            apply_location(&global_app_state, model);
                        }
                    }
                    crate::websocket::SpacetimeUpdateMessages::Insert {
                        new,
                        database_name,
                        ..
                    } => {
                        let model: mobile_entity_state::Model =
                            ::entity::mobile_entity_state::ModelBuilder::new(new)
                                .with_region(database_name)
                                .build();

                        apply_location(&global_app_state, model);
                    }
                    crate::websocket::SpacetimeUpdateMessages::Update {
                        new,
                        database_name,
                        ..
                    } => {
                        let model: mobile_entity_state::Model =
                            ::entity::mobile_entity_state::ModelBuilder::new(new)
                                .with_region(database_name)
                                .build();

                        apply_location(&global_app_state, model);
                    }
                    crate::websocket::SpacetimeUpdateMessages::Remove { delete, .. } => {
                        global_app_state
                            .mobile_entity_state
                            .remove(&delete.entity_id);
                        let _ = global_app_state.tx.send_if_interested(
                            BroadcastTopics::player_location(delete.entity_id),
                            || {
                                OutboundWebSocketMessages::PlayerLocation(
                                    crate::player_state::resolve_player_location(
                                        &global_app_state,
                                        delete.entity_id,
                                        None,
                                    ),
                                )
                            },
                        );
                    }
                }
            }
        }
    });
    tokio::spawn(async move {
        match worker.await {
            Ok(()) if shutdown.stop.is_cancelled() => {
                tracing::info!(
                    worker = "mobile_entity_state",
                    "Mobile entity worker stopped during shutdown"
                );
            }
            Ok(()) => {
                tracing::error!(
                    worker = "mobile_entity_state",
                    "Mobile entity worker returned unexpectedly"
                );
            }
            Err(error) => {
                tracing::error!(
                    worker = "mobile_entity_state",
                    panicked = error.is_panic(),
                    cancelled = error.is_cancelled(),
                    %error,
                    "Mobile entity worker task failed"
                );
            }
        }
    });
}
