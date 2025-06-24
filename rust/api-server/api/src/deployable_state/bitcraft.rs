use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::deployable_state;
use futures::FutureExt;
use game_module::module_bindings::DeployableState;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_deployable_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<DeployableState>>,
    batch_size: usize,
    time_limit: Duration,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(deployable_state::Column::EntityId)
            .update_columns([
                deployable_state::Column::OwnerId,
                deployable_state::Column::ClaimEntityId,
                deployable_state::Column::Direction,
                deployable_state::Column::DeployableDescriptionId,
                deployable_state::Column::Nickname,
                deployable_state::Column::Hidden,
            ])
            .to_owned();

        let mut currently_known_deployable_state = ::entity::deployable_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        let cleanup_signal_future = cancel_token.cancelled().fuse();
        tokio::pin!(cleanup_signal_future);

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::deployable_state::Model = new.into();

                                if currently_known_deployable_state.contains_key(&model.entity_id) {
                                    let value = currently_known_deployable_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_deployable_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::deployable_state::Model = new.into();
                                if currently_known_deployable_state.contains_key(&model.entity_id) {
                                    let value = currently_known_deployable_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_deployable_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::deployable_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(DeployableState = id, error = error.to_string(), "Could not delete DeployableState");
                                }

                                tracing::debug!("DeployableState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    _ = &mut cleanup_signal_future => {
                        if global_app_state.connection_state.iter().filter(|a| a.eq(&true)).collect::<Vec<_>>().len() != global_app_state.connection_state.len() {
                            tracing::warn!("Cleanup did not run as not all servers have an active connection");
                            break;
                        }

                        let deployable_to_delete = currently_known_deployable_state.values().map(|ckds| {
                            ckds.entity_id
                        }).collect::<Vec<_>>();

                        tracing::info!("deployable_state to delete {} {:?}", deployable_to_delete.len(), deployable_to_delete);

                        let result = ::entity::deployable_state::Entity::delete_many()
                            .filter(::entity::deployable_state::Column::EntityId.is_in(deployable_to_delete))
                            .exec(&global_app_state.conn).await;

                        if let Err(error) = result {
                            tracing::error!("Error while cleanup of deployable_state {error}");
                        }

                        currently_known_deployable_state.clear();

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
                let _ = ::entity::deployable_state::Entity::insert_many(messages.clone())
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
