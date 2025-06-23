use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::UserState;
use kanal::AsyncReceiver;
use std::sync::Arc;

pub(crate) fn start_worker_user_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<UserState>>,
) {
    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Insert { new, .. } => {
                    global_app_state
                        .user_state
                        .insert(new.identity, new.entity_id);
                }
                SpacetimeUpdateMessages::Update { new, .. } => {
                    global_app_state
                        .user_state
                        .insert(new.identity, new.entity_id);
                }
                SpacetimeUpdateMessages::Remove { delete, .. } => {
                    global_app_state.user_state.remove(&delete.identity);
                }
            }
        }
    });
}
