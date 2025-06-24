use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use futures::FutureExt;
use game_module::module_bindings::{PlayerState, PlayerUsernameState};
use kanal::AsyncReceiver;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, sea_query};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_player_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<PlayerState>>,
    batch_size: usize,
    time_limit: Duration,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::player_state::Column::EntityId)
            .update_columns([
                ::entity::player_state::Column::TimePlayed,
                ::entity::player_state::Column::SessionStartTimestamp,
                ::entity::player_state::Column::TimeSignedIn,
                ::entity::player_state::Column::SignInTimestamp,
                ::entity::player_state::Column::SignedIn,
                ::entity::player_state::Column::TeleportLocation,
                ::entity::player_state::Column::TravelerTasksExpiration,
            ])
            .to_owned();

        let mut currently_known_player_state = ::entity::player_state::Entity::find()
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
            let mut ids = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::player_state::Model = new.into();

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).increment(1);

                                ids.push(model.entity_id);
                                if !currently_known_player_state.is_empty() && currently_known_player_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, old, .. } => {
                                let model: ::entity::player_state::Model = new.into();


                                if model.signed_in != old.signed_in {
                                    metrics::gauge!("players_current_state", &[
                                        ("online", model.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).increment(1);
                                    metrics::gauge!("players_current_state", &[
                                        ("online", old.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).decrement(1);
                                }

                                ids.push(model.entity_id);
                                if !currently_known_player_state.is_empty() && currently_known_player_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::player_state::Model = delete.into();
                                let id = model.entity_id;

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).decrement(1);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerState = id, error = error.to_string(), "Could not delete PlayerState");
                                }

                                tracing::debug!("PlayerState::Remove");
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

                        let players_to_delete = currently_known_player_state.values().map(|ckps| ckps.entity_id).collect::<Vec<_>>();

                        tracing::info!("players_to_delete {} {:?}", players_to_delete.len(), players_to_delete);

                        let result = ::entity::player_state::Entity::delete_many()
                            .filter(::entity::player_state::Column::EntityId.is_in(players_to_delete))
                            .exec(&global_app_state.conn).await;

                        if let Err(error) = result {
                            tracing::error!("Error while cleanup of player_state {error}");
                        }

                        currently_known_player_state.clear();

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
                let _ = ::entity::player_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
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

pub(crate) fn start_worker_player_username_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
                .update_columns([::entity::player_username_state::Column::Username])
                .to_owned();

        let mut currently_known_player_username_state =
            ::entity::player_username_state::Entity::find()
                .all(&global_app_state.conn)
                .await
                .map_or(vec![], |aa| aa)
                .into_iter()
                .map(|value| (value.entity_id, value))
                .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let mut ids = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();

                                ids.push(model.entity_id);

                                if currently_known_player_username_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_username_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_username_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();
                                ids.push(model.entity_id);
                                if currently_known_player_username_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_username_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_username_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_username_state::Model = delete.into();
                                let id = model.entity_id;

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerUsernameState = id, error = error.to_string(), "Could not delete PlayerUsernameState");
                                }

                                tracing::debug!("PlayerUsernameState::Remove");
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
                let _ = ::entity::player_username_state::Entity::insert_many(messages.clone())
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
