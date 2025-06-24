use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use chrono::DateTime;
use entity::inventory_changelog::TypeOfChange;
use futures::FutureExt;
use game_module::module_bindings::InventoryState;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, NotSet, Set};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_inventory_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<InventoryState>>,
    batch_size: usize,
    time_limit: Duration,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::inventory::Column::EntityId)
            .update_columns([
                ::entity::inventory::Column::Pockets,
                ::entity::inventory::Column::InventoryIndex,
                ::entity::inventory::Column::CargoIndex,
                ::entity::inventory::Column::OwnerEntityId,
                ::entity::inventory::Column::PlayerOwnerEntityId,
            ])
            .to_owned();
        let on_conflict_changelog =
            sea_query::OnConflict::column(::entity::inventory_changelog::Column::Id)
                .update_columns([
                    ::entity::inventory_changelog::Column::EntityId,
                    ::entity::inventory_changelog::Column::UserId,
                    ::entity::inventory_changelog::Column::PocketNumber,
                    ::entity::inventory_changelog::Column::OldItemId,
                    ::entity::inventory_changelog::Column::OldItemType,
                    ::entity::inventory_changelog::Column::OldItemQuantity,
                    ::entity::inventory_changelog::Column::NewItemId,
                    ::entity::inventory_changelog::Column::NewItemType,
                    ::entity::inventory_changelog::Column::NewItemQuantity,
                    ::entity::inventory_changelog::Column::TypeOfChange,
                    ::entity::inventory_changelog::Column::Timestamp,
                ])
                .to_owned();
        let mut currently_known_inventory = ::entity::inventory::Entity::find()
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
            let mut messages_changed = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::inventory::Model = new.into();

                                global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                if currently_known_inventory.contains_key(&model.entity_id) {
                                    let value = currently_known_inventory.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_inventory.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, old, caller_identity, timestamp, .. } => {
                                let new_model = new.clone();
                                let model: ::entity::inventory::Model = new.into();
                                global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                if currently_known_inventory.contains_key(&model.entity_id) {
                                    let value = currently_known_inventory.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_inventory.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }


                                if let Some(caller_identity) = caller_identity {
                                    let user_id = global_app_state.user_state.get(&caller_identity).map(|entity_id| entity_id.to_owned() as i64);
                                    for (pocket_index, new_pocket) in new_model.pockets.iter().enumerate() {
                                        let old_pocket = &old.pockets[pocket_index];

                                        let new_item_id = new_pocket.contents.as_ref().map(|c| c.item_id);
                                        let new_item_type = new_pocket.contents.as_ref().map(|c| c.item_type.into());
                                        let new_item_quantity = new_pocket.contents.as_ref().map(|c| c.quantity);

                                        let old_item_id = old_pocket.contents.as_ref().map(|c| c.item_id);
                                        let old_item_type = old_pocket.contents.as_ref().map(|c| c.item_type.into());
                                        let old_item_quantity = old_pocket.contents.as_ref().map(|c| c.quantity);

                                        if new_item_id == old_item_id  && new_item_type == old_item_type && new_item_quantity == old_item_quantity {
                                            continue
                                        }

                                        let type_of_change = match (old_item_id, new_item_id) {
                                            (Some(_), None) => TypeOfChange::Remove,
                                            (None, Some(_)) => TypeOfChange::Add,
                                            (Some(old), Some(new)) => {
                                                if old != new {
                                                    TypeOfChange::AddAndRemove
                                                } else {
                                                    TypeOfChange::Update
                                                }
                                            },
                                            _ => unreachable!("This type of change should never happen for an inventory")
                                        };

                                        messages_changed.push(::entity::inventory_changelog::ActiveModel {
                                            id: NotSet,
                                            entity_id: Set(new_model.entity_id as i64),
                                            user_id: Set(user_id),
                                            pocket_number: Set(pocket_index as i32),
                                            old_item_id: Set(old_item_id),
                                            old_item_type: Set(old_item_type),
                                            old_item_quantity: Set(old_item_quantity),
                                            new_item_id: Set(new_item_id),
                                            new_item_type: Set(new_item_type),
                                            new_item_quantity: Set(new_item_quantity),
                                            type_of_change: Set(type_of_change),
                                            timestamp: Set(DateTime::from_timestamp_micros(timestamp.unwrap().to_micros_since_unix_epoch()).unwrap())
                                        })
                                    }
                                }

                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::inventory::Model = delete.into();
                                let id = model.entity_id;

                                global_app_state.inventory_state.remove(&model.entity_id);
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(InventoryState = id, error = error.to_string(), "Could not delete InventoryState");
                                }

                                tracing::debug!("InventoryState::Remove");
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

                        let inventory_to_delete = currently_known_inventory.values().map(|ckps| {
                            global_app_state.inventory_state.remove(&ckps.entity_id);

                            ckps.entity_id
                        }).collect::<Vec<_>>();

                        tracing::info!("inventory to delete {} {:?}", inventory_to_delete.len(), inventory_to_delete);

                        let result = ::entity::inventory::Entity::delete_many()
                            .filter(::entity::inventory::Column::EntityId.is_in(inventory_to_delete))
                            .exec(&global_app_state.conn).await;

                        if let Err(error) = result {
                            tracing::error!("Error while cleanup of player_state {error}");
                        }

                        currently_known_inventory.clear();

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
                let _ = ::entity::inventory::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;
                // Your batch processing logic here
            }

            if !messages_changed.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ =
                    ::entity::inventory_changelog::Entity::insert_many(messages_changed.clone())
                        .on_conflict(on_conflict_changelog.clone())
                        .exec(&global_app_state.conn)
                        .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_changed.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
