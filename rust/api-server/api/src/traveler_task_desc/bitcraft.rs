use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::TravelerTaskDesc;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_traveler_task_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<TravelerTaskDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
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

        let mut currently_known_traveler_task_desc = ::entity::traveler_task_desc::Entity::find()
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
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                global_app_state.traveler_task_desc.insert(model.id, model.clone());
                                if currently_known_traveler_task_desc.contains_key(&model.id) {
                                    let value = currently_known_traveler_task_desc.get(&model.id).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_traveler_task_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                global_app_state.traveler_task_desc.insert(model.id, model.clone());
                                if currently_known_traveler_task_desc.contains_key(&model.id) {
                                    let value = currently_known_traveler_task_desc.get(&model.id).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_traveler_task_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
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
                let _ = ::entity::traveler_task_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;
            }

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
