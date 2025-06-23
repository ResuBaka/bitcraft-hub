use crate::AppState;
use crate::websocket::WebSocketMessages;
use entity::mobile_entity_state;
use game_module::module_bindings::MobileEntityState;
use kanal::AsyncReceiver;
use std::sync::Arc;

pub(crate) fn start_worker_mobile_entity_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<crate::websocket::SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            match update {
                crate::websocket::SpacetimeUpdateMessages::Insert { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                crate::websocket::SpacetimeUpdateMessages::Update { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                crate::websocket::SpacetimeUpdateMessages::Remove { delete, .. } => {
                    global_app_state
                        .mobile_entity_state
                        .remove(&delete.entity_id);
                }
            }
        }
    });
}
