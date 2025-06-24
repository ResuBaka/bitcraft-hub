use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::item_desc;
use game_module::module_bindings::ItemDesc;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_item_desc(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ItemDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(item_desc::Column::Id)
            .update_columns([
                item_desc::Column::Name,
                item_desc::Column::Description,
                item_desc::Column::Volume,
                item_desc::Column::Durability,
                item_desc::Column::ConvertToOnDurabilityZero,
                item_desc::Column::SecondaryKnowledgeId,
                item_desc::Column::ModelAssetName,
                item_desc::Column::IconAssetName,
                item_desc::Column::Tier,
                item_desc::Column::Tag,
                item_desc::Column::Rarity,
                item_desc::Column::CompendiumEntry,
                item_desc::Column::ItemListId,
            ])
            .to_owned();

        let mut currently_known_item_desc = ::entity::item_desc::Entity::find()
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
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                if currently_known_item_desc.contains_key(&model.id) {
                                    let value = currently_known_item_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                if currently_known_item_desc.contains_key(&model.id) {
                                    let value = currently_known_item_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::item_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.item_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ItemDesc = id, error = error.to_string(), "Could not delete ItemDesc");
                                }

                                tracing::debug!("ItemDesc::Remove");
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
                let _ = ::entity::item_desc::Entity::insert_many(messages.clone())
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
