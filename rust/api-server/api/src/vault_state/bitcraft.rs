use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::vault_state_collectibles;
use game_module::module_bindings::VaultState;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_vault_state_collectibles(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<VaultState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            vault_state_collectibles::Column::EntityId,
            vault_state_collectibles::Column::Id,
        ])
        .update_columns([
            vault_state_collectibles::Column::Activated,
            vault_state_collectibles::Column::Count,
        ])
        .to_owned();

        let mut currently_known_vault_state_collectibles =
            ::entity::vault_state_collectibles::Entity::find()
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
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles();

                                for model in models {
                                    if currently_known_vault_state_collectibles.contains_key(&model.entity_id) {
                                        let value = currently_known_vault_state_collectibles.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_vault_state_collectibles.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles();
                                for model in models {
                                    if currently_known_vault_state_collectibles.contains_key(&model.entity_id) {
                                        let value = currently_known_vault_state_collectibles.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_vault_state_collectibles.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = delete.into();
                                let models= raw_model.to_model_collectibles();
                                for model in models {

                                    let id = model.entity_id;

                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }

                                    if let Err(error) = model.delete(&global_app_state.conn).await {
                                        tracing::error!(VaultState = id, error = error.to_string(), "Could not delete VaultState");
                                    }
                                }

                                tracing::debug!("VaultState::Remove");
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
                let _ = ::entity::vault_state_collectibles::Entity::insert_many(messages.clone())
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
