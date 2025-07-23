use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::UserState;
use tokio::sync::mpsc::UnboundedReceiver;

pub(crate) fn start_worker_user_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<UserState>>,
) {
    tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Initial { data, .. } => {
                    for item in data {
                        global_app_state
                            .user_state
                            .insert(item.identity, item.entity_id);
                    }
                }
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
