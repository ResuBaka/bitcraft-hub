use crate::AppState;
use crate::leaderboard::experience_to_level;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use game_module::module_bindings::ExperienceState;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_experience_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ExperienceState>>,
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

        let mut currently_known_experience_state = ::entity::experience_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let id = new.entity_id;
                                new.experience_stacks.iter().for_each(|es| {
                                    let model = ::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    };

                                    if currently_known_experience_state.contains_key(&model.entity_id) {
                                        let value = currently_known_experience_state.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_experience_state.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, old, .. } => {
                                let id = new.entity_id;

                                let mut new_level_vec = vec![];

                                new.experience_stacks.iter().for_each(|es| {
                                    new_level_vec.push((
                                        es.clone(),
                                        experience_to_level(es.quantity as i64),
                                    ));

                                    let model = ::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    };

                                    if currently_known_experience_state.contains_key(&model.entity_id) {
                                        let value = currently_known_experience_state.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_experience_state.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                });
                                old.experience_stacks.iter().for_each(|es| {
                                    let old_level =
                                        experience_to_level(es.quantity as i64);

                                    let new_level = new_level_vec.iter().find(|new_level| new_level.0.skill_id.eq(&es.skill_id));
                                    let skill_name = global_app_state.skill_desc.get(&(es.skill_id as i64));

                                    if let Some(skill_name) = skill_name {
                                        if let Some(new_level) = new_level {
                                            if old_level != new_level.1 {

                                                    global_app_state.tx.send(WebSocketMessages::Level {
                                                        level: new_level.1 as u64,
                                                        skill_name: skill_name.to_owned().name,
                                                        user_id: id as i64,
                                                    })
                                                    .expect("TODO: panic message");
                                                }

                                            if new_level.0.quantity > es.quantity {
                                                global_app_state.tx.send(WebSocketMessages::Experience {
                                                    level: new_level.1 as u64,
                                                    experience: new_level.0.quantity as u64,
                                                    rank: 0,
                                                    skill_name: skill_name.to_owned().name,
                                                    user_id: id as i64,
                                                })
                                                .expect("TODO: panic message");
                                            }
                                        }
                                    }
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let id = delete.entity_id as i64;
                                let vec_es = delete.experience_stacks.iter().map(|es| {
                                    if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                        messages.remove(index);
                                    }

                                    ::entity::experience_state::Model {
                                        entity_id: id,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::experience_state::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
