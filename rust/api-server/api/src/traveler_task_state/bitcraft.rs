use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::TravelerTaskState;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_traveler_task_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<TravelerTaskState>>,
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

        let mut currently_known_traveler_task_state = ::entity::traveler_task_state::Entity::find()
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
                                let model: ::entity::traveler_task_state::Model = new.into();
                                if currently_known_traveler_task_state.contains_key(&model.entity_id) {
                                    let value = currently_known_traveler_task_state.get(&model.entity_id).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_traveler_task_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::traveler_task_state::Model = new.into();
                                if currently_known_traveler_task_state.contains_key(&model.entity_id) {
                                    let value = currently_known_traveler_task_state.get(&model.entity_id).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_traveler_task_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
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
                let _ = ::entity::traveler_task_state::Entity::insert_many(messages.clone())
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
