use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::vault_state_collectibles;
use game_module::module_bindings::VaultState;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_vault_state_collectibles(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<VaultState>>,
    batch_size: usize,
    time_limit: Duration,
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

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_vault_state_collectibles = ::entity::vault_state_collectibles::Entity::find()
                                    .filter(::entity::vault_state_collectibles::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().flat_map(|value| {
                                    let model: ::entity::vault_state_collectibles::RawVaultState = value.into();

                                    model.to_model_collectibles(database_name.to_string())
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_vault_state_collectibles.entry(model.entity_id) {
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
                                       insert_multiple_vault_state_collectibles(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_vault_state_collectibles(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_vault_state_collectibles.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::vault_state_collectibles::Entity::delete_many().filter(::entity::vault_state_collectibles::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(VaultState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete VaultState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles(database_name.to_string());

                                for model in models {

                                    if let Some(index) = messages.iter().position(|value: &::entity::vault_state_collectibles::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }

                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles(database_name.to_string());
                                for model in models {

                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }

                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = delete.into();
                                let models= raw_model.to_model_collectibles(database_name.to_string());
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
                insert_multiple_vault_state_collectibles(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
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

async fn insert_multiple_vault_state_collectibles(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::vault_state_collectibles::ActiveModel>,
) {
    let insert = ::entity::vault_state_collectibles::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting VaultState: {}", insert.unwrap_err())
    }

    messages.clear();
}
