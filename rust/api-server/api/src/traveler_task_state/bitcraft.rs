use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::TravelerTaskState;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_traveler_task_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::traveler_task_state::Column::EntityId)
                .update_columns([
                    ::entity::traveler_task_state::Column::PlayerEntityId,
                    ::entity::traveler_task_state::Column::TravelerId,
                    ::entity::traveler_task_state::Column::TaskId,
                    ::entity::traveler_task_state::Column::Completed,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_traveler_task_state = ::entity::traveler_task_state::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::traveler_task_state::Model = value.into();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_traveler_task_state.entry(model.entity_id) {
                                        Entry::Occupied(entry) => {
                                            let existing_model = entry.get();
                                            if &model != existing_model {
                                                local_messages.push(model.into_active_model());
                                            }
                                            entry.remove();
                                        }
                                        Entry::Vacant(_entry) => {
                                            local_messages.push(model.into_active_model());
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
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::traveler_task_state::Model = new.into();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::traveler_task_state::Model = new.into();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::traveler_task_state::Model = delete.into();
                                let id = model.entity_id;
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(TravelerTaskState = id, error = error.to_string(), "Could not delete TravelerTaskState");
                                }
                                tracing::debug!("TravelerTaskState::Remove");
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

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_traveler_task_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::traveler_task_state::ActiveModel>,
) {
    let insert = ::entity::traveler_task_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting TravelerTaskState: {}", insert.unwrap_err())
    }

    messages.clear();
}
