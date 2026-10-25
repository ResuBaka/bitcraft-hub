use crate::AppState;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::UserState;

pub(crate) fn start_worker_user_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<UserState>>,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("user_state");
        while let Some(update) = rx.recv().await {
            receive_metrics.record(1);
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
