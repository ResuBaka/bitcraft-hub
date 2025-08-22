use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::TravelerTaskDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_traveler_task_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::traveler_task_desc::Column::Id)
            .update_columns([
                ::entity::traveler_task_desc::Column::SkillId,
                ::entity::traveler_task_desc::Column::MinLevel,
                ::entity::traveler_task_desc::Column::MaxLevel,
                ::entity::traveler_task_desc::Column::RequiredItems,
                ::entity::traveler_task_desc::Column::RewardedItems,
                ::entity::traveler_task_desc::Column::RewardedExperience,
                ::entity::traveler_task_desc::Column::Description,
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
                                let mut currently_known_traveler_task_desc = ::entity::traveler_task_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::traveler_task_desc::Model = value.into();

                                    model
                                }) {
                                    global_app_state.traveler_task_desc.insert(model.id, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_traveler_task_desc.entry(model.id) {
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
                                       insert_multiple_traveler_task_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_traveler_task_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_traveler_task_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::traveler_task_desc::Entity::delete_many().filter(::entity::traveler_task_desc::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(TravelerTaskDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete TravelerTaskDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                global_app_state.traveler_task_desc.insert(model.id, model.clone());

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                global_app_state.traveler_task_desc.insert(model.id, model.clone());

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::traveler_task_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.traveler_task_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(TravelerTaskDesc = id, error = error.to_string(), "Could not delete TravelerTaskDesc");
                                }
                                tracing::debug!("TravelerTaskDesc::Remove");
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
                insert_multiple_traveler_task_desc(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_traveler_task_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::traveler_task_desc::ActiveModel>,
) {
    let insert = ::entity::traveler_task_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting TravelerTaskDesc: {}", insert.unwrap_err())
    }

    messages.clear();
}
