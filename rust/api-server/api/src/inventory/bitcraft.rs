use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use chrono::DateTime;
use entity::inventory_changelog::TypeOfChange;
use futures::FutureExt;
use game_module::module_bindings::InventoryState;
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, NotSet, Set};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_inventory_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<InventoryState>>,
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

        let cleanup_signal_future = cancel_token.cancelled().fuse();
        tokio::pin!(cleanup_signal_future);

        loop {
            let mut messages = Vec::new();
            let mut messages_changed = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                let mut buffer = vec![];
                let fill_buffer_with = batch_size
                    .saturating_sub(buffer.len())
                    .saturating_sub(messages.len());

                tokio::select! {
                    _count = rx.recv_many(&mut buffer, fill_buffer_with) => {
                        for msg in buffer {
                            match msg {
                                SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                    tracing::info!("Count of inventory amount to work on {}", data.len());
                                    let mut local_messages = vec![];
                                    let mut currently_known_inventory = ::entity::inventory::Entity::find()
                                        .filter(::entity::inventory::Column::Region.eq(database_name.to_string()))
                                        .all(&global_app_state.conn)
                                        .await
                                        .map_or(vec![], |aa| aa)
                                        .into_iter()
                                        .map(|value| (value.entity_id, value))
                                        .collect::<HashMap<_, _>>();

                                    for model in data.into_iter().map(|value| {
                                        let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                        model
                                    }) {
                                        use std::collections::hash_map::Entry;
                                        match currently_known_inventory.entry(model.entity_id) {
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
                                           insert_multiple_inventory(&global_app_state, &on_conflict, &mut local_messages).await;
                                        }
                                    };
                                    if !local_messages.is_empty() {
                                        insert_multiple_inventory(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }

                                    tracing::info!("Count of inventory amount to delete {}", currently_known_inventory.len());

                                    for chunk_ids in currently_known_inventory.into_keys().collect::<Vec<_>>().chunks(1000) {
                                        let chunk_ids = chunk_ids.to_vec();
                                        if let Err(error) = ::entity::inventory::Entity::delete_many().filter(::entity::inventory::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                            tracing::error!(Inventory = chunk_ids_str.join(","), error = error.to_string(), "Could not delete Inventory");
                                        }
                                    }
                                }
                                SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                    let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                    // global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                    if let Some(index) = messages.iter().position(|value: &::entity::inventory::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                                SpacetimeUpdateMessages::Update { new, old, caller_identity, timestamp, database_name, .. } => {
                                    let new_model = new.clone();
                                    let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                    // global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());



                                    if let Some(caller_identity) = caller_identity {
                                        let user_id = global_app_state.user_state.get(&caller_identity).map(|entity_id| entity_id.to_owned() as i64);
                                        for (pocket_index, new_pocket) in new_model.pockets.iter().enumerate() {
                                            if pocket_index >= old.pockets.len() {
                                                tracing::warn!(
                                                    "Inventory new pocket amount is less then before ?!? Player {}, EntityId {}, OwnerEntityId {}, Pockets New {}, Pockets Old {} :: {} {}",
                                                    new_model.player_owner_entity_id,
                                                    new_model.entity_id,
                                                    new_model.owner_entity_id,
                                                    new_model.pockets.len(),
                                                    old.pockets.len(),
                                                    old.pockets.len(),
                                                    pocket_index
                                                );
                                                break;
                                            }

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

                                    if messages.len() >= batch_size {
                                        break;
                                    }

                                    if messages_changed.len() >= batch_size {
                                        break;
                                    }

                                }
                                SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                    let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                    let id = model.entity_id;

                                    // global_app_state.inventory_state.remove(&model.entity_id);
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
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    _ = &mut cleanup_signal_future => {

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
                insert_multiple_inventory(&global_app_state, &on_conflict, &mut messages).await;
                // Your batch processing logic here
            }

            if !messages_changed.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                insert_multiple_inventory_changelog(
                    &global_app_state,
                    &on_conflict_changelog,
                    &mut messages_changed,
                )
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

async fn insert_multiple_inventory(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::inventory::ActiveModel>,
) {
    let insert = ::entity::inventory::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting InventoryState: {}", insert.unwrap_err())
    }

    messages.clear();
}

async fn insert_multiple_inventory_changelog(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::inventory_changelog::ActiveModel>,
) {
    let insert = ::entity::inventory_changelog::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!(
            "Error inserting InventoryChangelog: {}",
            insert.unwrap_err()
        )
    }

    messages.clear();
}
