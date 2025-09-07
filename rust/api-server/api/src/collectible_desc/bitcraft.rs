use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::collectible_desc;
use game_module::module_bindings::CollectibleDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_collectible_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<CollectibleDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(collectible_desc::Column::Id)
            .update_columns([
                collectible_desc::Column::Name,
                collectible_desc::Column::Description,
                collectible_desc::Column::CollectibleType,
                collectible_desc::Column::InvalidatesType,
                collectible_desc::Column::AutoCollect,
                collectible_desc::Column::CollectibleRarity,
                collectible_desc::Column::StartingLoadout,
                collectible_desc::Column::Locked,
                collectible_desc::Column::Variant,
                collectible_desc::Column::Color,
                collectible_desc::Column::Emission,
                collectible_desc::Column::MaxEquipCount,
                collectible_desc::Column::ModelAssetName,
                collectible_desc::Column::VariantMaterial,
                collectible_desc::Column::IconAssetName,
                collectible_desc::Column::Tag,
                collectible_desc::Column::DisplayString,
                collectible_desc::Column::ItemDeedId,
                collectible_desc::Column::RequiredKnowledgesToUse,
                collectible_desc::Column::RequiredKnowledgesToConvert,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_collectible_desc = ::entity::collectible_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::collectible_desc::Model = value.into();

                                    model
                                }) {
                                    // global_app_state.collectible_desc.insert(model.id, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_collectible_desc.entry(model.id) {
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
                                       insert_multiple_collectible_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_collectible_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_collectible_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::collectible_desc::Entity::delete_many().filter(::entity::collectible_desc::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(CollectibleDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete CollectibleDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::collectible_desc::Model = new.into();
                                // global_app_state.collectible_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::collectible_desc::Model = new.into();
                                // global_app_state.collectible_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::collectible_desc::Model = delete.into();
                                let id = model.id;
                                // global_app_state.collectible_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CollectibleDesc = id, error = error.to_string(), "Could not delete CollectibleDesc");
                                }

                                tracing::debug!("CollectibleDesc::Remove");
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

                insert_multiple_collectible_desc(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_collectible_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::collectible_desc::ActiveModel>,
) {
    let result = ::entity::collectible_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving collectible_desc"
        );
    }

    messages.clear();
}
