use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::skill_desc;
use game_module::module_bindings::SkillDesc;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_skill_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<SkillDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
            .update_columns([
                skill_desc::Column::Name,
                skill_desc::Column::Description,
                skill_desc::Column::IconAssetName,
                skill_desc::Column::Title,
                skill_desc::Column::SkillCategory,
                skill_desc::Column::Skill,
            ])
            .to_owned();

        let mut currently_known_skill_desc = ::entity::skill_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
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
                                let model: ::entity::skill_desc::Model = new.into();

                                global_app_state.skill_desc.insert(model.id, model.clone());
                                if currently_known_skill_desc.contains_key(&model.id) {
                                    let value = currently_known_skill_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_skill_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::skill_desc::Model = new.into();
                                global_app_state.skill_desc.insert(model.id, model.clone());
                                if currently_known_skill_desc.contains_key(&model.id) {
                                    let value = currently_known_skill_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_skill_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::skill_desc::Model = delete.into();
                                let id = model.id;

                                global_app_state.skill_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(SkillDesc = id, error = error.to_string(), "Could not delete SkillDesc");
                                }

                                tracing::debug!("SkillDesc::Remove");
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
                let _ = ::entity::skill_desc::Entity::insert_many(messages.clone())
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
