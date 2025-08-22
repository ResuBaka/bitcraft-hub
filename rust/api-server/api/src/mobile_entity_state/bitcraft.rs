use crate::AppState;
use crate::websocket::WebSocketMessages;
use entity::mobile_entity_state;
use game_module::module_bindings::MobileEntityState;
use tokio::sync::mpsc::UnboundedReceiver;

pub(crate) fn start_worker_mobile_entity_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<crate::websocket::SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tokio::spawn(async move {
        loop {
            let mut buffer = vec![];

            let _count = rx.recv_many(&mut buffer, 4000).await;
            for msg in buffer {
                match msg {
                    crate::websocket::SpacetimeUpdateMessages::Initial {
                        data,
                        database_name,
                        ..
                    } => {
                        for entry in data {
                            let model: mobile_entity_state::Model =
                                ::entity::mobile_entity_state::ModelBuilder::new(entry)
                                    .with_region(database_name.to_string())
                                    .build();
                            global_app_state
                                .mobile_entity_state
                                .insert(model.entity_id, model.clone());

                            let _ = global_app_state
                                .tx
                                .send(WebSocketMessages::MobileEntityState(model));
                        }
                    }
                    crate::websocket::SpacetimeUpdateMessages::Insert {
                        new,
                        database_name,
                        ..
                    } => {
                        let model: mobile_entity_state::Model =
                            ::entity::mobile_entity_state::ModelBuilder::new(new)
                                .with_region(database_name.to_string())
                                .build();

                        global_app_state
                            .mobile_entity_state
                            .insert(model.entity_id, model.clone());

                        let _ = global_app_state
                            .tx
                            .send(WebSocketMessages::MobileEntityState(model));
                    }
                    crate::websocket::SpacetimeUpdateMessages::Update {
                        new,
                        database_name,
                        ..
                    } => {
                        let model: mobile_entity_state::Model =
                            ::entity::mobile_entity_state::ModelBuilder::new(new)
                                .with_region(database_name.to_string())
                                .build();

                        global_app_state
                            .mobile_entity_state
                            .insert(model.entity_id, model.clone());

                        let _ = global_app_state
                            .tx
                            .send(WebSocketMessages::MobileEntityState(model));
                    }
                    crate::websocket::SpacetimeUpdateMessages::Remove { delete, .. } => {
                        global_app_state
                            .mobile_entity_state
                            .remove(&delete.entity_id);
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    });
}
