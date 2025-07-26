use crate::AppState;
use crate::leaderboard::experience_to_level;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use game_module::module_bindings::ExperienceState;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_experience_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([::entity::experience_state::Column::Experience])
        .to_owned();

        tracing::warn!(
            "Start ExperienceState worker with amount \"{}\" of messages queued",
            rx.len()
        );

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                let mut buffer = vec![];

                tokio::select! {
                    _count = rx.recv_many(&mut buffer, batch_size) => {
                        for msg in buffer {
                            match msg {
                                SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                    tracing::warn!("Processed Initial ExperienceState {}", data.len());
                                    let mut local_messages = vec![];
                                    let mut currently_known_experience_state = ::entity::experience_state::Entity::find()
                                        .filter(::entity::building_state::Column::Region.eq(database_name.to_string()))
                                        .all(&global_app_state.conn)
                                        .await
                                        .map_or(vec![], |aa| aa)
                                        .into_iter()
                                        .map(|value| {
                                            let entity_id = value.entity_id;
                                            let skill_id = value.skill_id;
                                            (format!("{entity_id}:{skill_id}"), value)
                                        })
                                        .collect::<HashMap<_, _>>();

                                    for model in data.into_iter().flat_map(|value| {
                                        let id = value.entity_id;
                                        let model: Vec<::entity::experience_state::Model> = value.experience_stacks.iter().map(|exp_stack| {
                                                ::entity::experience_state::Model {
                                                    entity_id: id as i64,
                                                    skill_id: exp_stack.skill_id,
                                                    experience: exp_stack.quantity,
                                                    region: database_name.to_string()
                                                }
                                            }).collect();

                                        model
                                    }) {
                                        let key = format!("{}:{}", model.entity_id, model.skill_id);
                                        use std::collections::hash_map::Entry;
                                        match currently_known_experience_state.entry(key) {
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
                                           tracing::warn!("Initial Processing {} messages in batch", local_messages.len());
                                           insert_multiple_experience_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                        }
                                    };
                                    if !local_messages.is_empty() {
                                        tracing::warn!("Last Initial Processing {} messages in batch", local_messages.len());
                                        insert_multiple_experience_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }

                                    for chunk_ids in currently_known_experience_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                        let chunk_ids = chunk_ids.to_vec();
                                        if let Err(error) = ::entity::experience_state::Entity::delete_many().filter(::entity::experience_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                            tracing::error!(ExperienceState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ExperienceState");
                                        }
                                    }
                                    tracing::warn!("Processed Initial ExperienceState");
                                    break;
                                }
                                SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                    let id = new.entity_id as i64;
                                    new.experience_stacks.iter().for_each(|es| {
                                        let model = ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        };

                                        if let Some(index) = messages.iter().position(|value: &::entity::experience_state::ActiveModel| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }
                                        messages.push(model.into_active_model());
                                    });

                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                                SpacetimeUpdateMessages::Update { new, old, database_name, .. } => {
                                    let id = new.entity_id as i64;

                                    let mut new_level_vec = vec![];

                                    new.experience_stacks.iter().for_each(|es| {
                                        new_level_vec.push((
                                            es.clone(),
                                            experience_to_level(es.quantity as i64),
                                        ));

                                        let model = ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        };

                                        if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }
                                        messages.push(model.into_active_model());
                                    });
                                    for es in old.experience_stacks.iter() {
                                        let old_level =
                                            experience_to_level(es.quantity as i64);

                                        let new_level = new_level_vec.iter().find(|new_level| new_level.0.skill_id.eq(&es.skill_id));
                                        let skill_name = global_app_state.skill_desc.get(&(es.skill_id as i64));

                                        if let Some(skill_name) = skill_name {
                                            if let Some(new_level) = new_level {
                                                if old_level != new_level.1 {
                                                    let _ = global_app_state.tx.send(WebSocketMessages::Level {
                                                        level: new_level.1 as u64,
                                                        skill_name: skill_name.to_owned().name,
                                                        user_id: id,
                                                    });
                                                }

                                                if new_level.0.quantity > es.quantity {
                                                    let _ = global_app_state.tx.send(WebSocketMessages::Experience {
                                                        level: new_level.1 as u64,
                                                        experience: new_level.0.quantity as u64,
                                                        rank: 0,
                                                        skill_name: skill_name.to_owned().name,
                                                        user_id: id,
                                                    });
                                                }
                                            }
                                        }
                                    };

                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                                SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                    let id = delete.entity_id as i64;
                                    let vec_es = delete.experience_stacks.iter().map(|es| {
                                        if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }

                                        ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        }
                                    }).collect::<Vec<_>>();

                                    for es in vec_es {
                                        if let Err(error) = es.delete(&global_app_state.conn).await {
                                            tracing::error!(ExperienceState = id, error = error.to_string(), "Could not delete ExperienceState");
                                        }
                                    }
                                    tracing::debug!("ExperienceState::Remove");
                                }
                            }
                        }
                    }
                    _ = &mut timer => {

                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!("Processing {} messages in batch", messages.len());
                insert_multiple_experience_state(&global_app_state, &on_conflict, &mut messages)
                    .await;

                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                tracing::warn!(
                    "Shutting down ExperienceState worker as there no messages and rx is closed"
                );
                break;
            }
        }
    });
}

async fn insert_multiple_experience_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::experience_state::ActiveModel>,
) {
    let result = ::entity::experience_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving experience_state"
        );
    }

    messages.clear();
}
