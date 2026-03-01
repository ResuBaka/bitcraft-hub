use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use game_module::module_bindings::TravelerTaskState;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_traveler_task_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::traveler_task_state::Column::EntityId)
                .update_columns([
                    ::entity::traveler_task_state::Column::PlayerEntityId,
                    ::entity::traveler_task_state::Column::TravelerId,
                    ::entity::traveler_task_state::Column::TaskId,
                    ::entity::traveler_task_state::Column::Completed,
                    ::entity::traveler_task_state::Column::Region,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                let mut buffer = vec![];

                let fill_buffer_with = if messages.len() > messages_delete.len() {
                    batch_size
                        .saturating_sub(buffer.len())
                        .saturating_sub(messages.len())
                } else {
                    batch_size
                        .saturating_sub(buffer.len())
                        .saturating_sub(messages_delete.len())
                };

                tokio::select! {
                    _count = rx.recv_many(&mut buffer, fill_buffer_with) => {
                        for msg in buffer {
                            match msg {
                                SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                    let mut local_messages = Vec::with_capacity(batch_size + 10);
                                    let mut currently_known_traveler_task_state = ::entity::traveler_task_state::Entity::find()
                                        .filter(::entity::traveler_task_state::Column::Region.eq(database_name.to_string()))
                                        .all(&global_app_state.conn)
                                        .await
                                        .map_or_else(|error| {
                                            tracing::error!(
                                                error = error.to_string(),
                                                "Error while query whole traveler_task_state state"
                                            );
                                            vec![]
                                        },|aa| aa)
                                        .into_iter()
                                        .map(|value| (value.entity_id, value))
                                        .collect::<HashMap<_, _>>();

                                    for model in data.into_iter().map(|value| {
                                        let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                        model
                                    }) {
                                        use std::collections::hash_map::Entry;
                                        match currently_known_traveler_task_state.entry(model.entity_id) {
                                            Entry::Occupied(entry) => {
                                                let existing_model = entry.get();
                                                if &model != existing_model {
                                                    local_messages.push(model);
                                                }
                                                entry.remove();
                                            }
                                            Entry::Vacant(_entry) => {
                                                local_messages.push(model);
                                            }
                                        }
                                        if local_messages.len() >= batch_size {
                                           insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                        }
                                    };
                                    if !local_messages.is_empty() {
                                        insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }

                                    for chunk_ids in currently_known_traveler_task_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                        let chunk_ids = chunk_ids.to_vec();
                                        if let Err(error) = ::entity::traveler_task_state::Entity::delete_many().filter(::entity::traveler_task_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                            tracing::error!(TravelerTaskState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete TravelerTaskState");
                                        }
                                    }
                                }
                                SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                    let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                    let _ = global_app_state.tx.send(WebSocketMessages::TravelerTaskState(model.clone()));

                                    if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                        messages_delete.remove(index);
                                    }
                                    messages.push(model);
                                    if messages.len() >= batch_size {
                                        insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                    let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                    let _ = global_app_state.tx.send(WebSocketMessages::TravelerTaskState(model.clone()));

                                    if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                        messages.remove(index);
                                    }

                                    if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                        messages_delete.remove(index);
                                    }
                                    messages.push(model);
                                    if messages.len() >= batch_size {
                                        insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Remove { delete, .. } => {
                                    let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(delete).build();
                                    let id = model.entity_id;
                                    let _ = global_app_state.tx.send(WebSocketMessages::TravelerTaskStateDelete(model.clone()));

                                    if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                        messages.remove(index);
                                    }

                                    messages_delete.push(id);
                                    if messages_delete.len() >= batch_size {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ = &mut timer => {
                        break;
                    }
                    else => {
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            if !messages_delete.is_empty() {
                tracing::debug!("TravelerTaskState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::traveler_task_state::Entity::delete_many()
                        .filter(
                            ::entity::traveler_task_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            TravelerTaskState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete TravelerTaskState"
                        );
                    }
                }
            }

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_traveler_task_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::traveler_task_state::Model>,
) {
    let insert = ::entity::traveler_task_state::Entity::insert_many(
        messages
            .iter()
            .map(|model| model.clone().into_active_model()),
    )
    .on_conflict(on_conflict.clone())
    .exec(&global_app_state.conn)
    .await;

    if insert.is_err() {
        tracing::error!("Error inserting TravelerTaskState: {}", insert.unwrap_err())
    }

    messages.clear();
}
