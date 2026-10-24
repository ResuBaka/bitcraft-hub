use crate::AppState;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{OutboundWebSocketMessages, record_worker_received};
use entity::mobile_entity_state;
use game_module::module_bindings::MobileEntityState;

pub(crate) fn start_worker_mobile_entity_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<crate::websocket::SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tracing::info!(
        worker = "mobile_entity_state",
        "Scheduling mobile entity worker"
    );
    global_app_state.shutdown.clone().spawn_worker(async move {
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
            record_worker_received("mobile_entity_state", count);
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
                            global_app_state
                                .mobile_entity_state
                                .insert(model.entity_id, model.clone());

                            let _ = global_app_state
                                .tx
                                .send(OutboundWebSocketMessages::MobileEntityState(model));
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

                        global_app_state
                            .mobile_entity_state
                            .insert(model.entity_id, model.clone());

                        let _ = global_app_state
                            .tx
                            .send(OutboundWebSocketMessages::MobileEntityState(model));
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

                        global_app_state
                            .mobile_entity_state
                            .insert(model.entity_id, model.clone());

                        let _ = global_app_state
                            .tx
                            .send(OutboundWebSocketMessages::MobileEntityState(model));
                    }
                    crate::websocket::SpacetimeUpdateMessages::Remove { delete, .. } => {
                        global_app_state
                            .mobile_entity_state
                            .remove(&delete.entity_id);
                    }
                }
            }
        }
    });
}
