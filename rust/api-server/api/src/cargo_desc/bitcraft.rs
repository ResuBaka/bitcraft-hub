use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::cargo_desc;
use game_module::module_bindings::CargoDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_cargo_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<CargoDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
            .update_columns([
                cargo_desc::Column::Name,
                cargo_desc::Column::Description,
                cargo_desc::Column::Volume,
                cargo_desc::Column::SecondaryKnowledgeId,
                cargo_desc::Column::ModelAssetName,
                cargo_desc::Column::IconAssetName,
                cargo_desc::Column::CarriedModelAssetName,
                cargo_desc::Column::PickUpAnimationStart,
                cargo_desc::Column::PickUpAnimationEnd,
                cargo_desc::Column::DropAnimationStart,
                cargo_desc::Column::DropAnimationEnd,
                cargo_desc::Column::PickUpTime,
                cargo_desc::Column::PlaceTime,
                cargo_desc::Column::AnimatorState,
                cargo_desc::Column::MovementModifier,
                cargo_desc::Column::BlocksPath,
                cargo_desc::Column::OnDestroyYieldCargos,
                cargo_desc::Column::DespawnTime,
                cargo_desc::Column::Tier,
                cargo_desc::Column::Tag,
                cargo_desc::Column::Rarity,
                cargo_desc::Column::NotPickupable,
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
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_cargo_desc = ::entity::cargo_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::cargo_desc::Model = value.into();

                                    model
                                }) {
                                    global_app_state.cargo_desc.insert(model.id, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_cargo_desc.entry(model.id) {
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
                                       insert_multiple_cargo_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_cargo_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_cargo_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::cargo_desc::Entity::delete_many().filter(::entity::cargo_desc::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(CargoDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete CargoDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::cargo_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.cargo_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CargoDesc = id, error = error.to_string(), "Could not delete CargoDesc");
                                }

                                tracing::debug!("CargoDesc::Remove");
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

                insert_multiple_cargo_desc(&global_app_state, &on_conflict, &mut messages).await;
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_cargo_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::cargo_desc::ActiveModel>,
) {
    let result = ::entity::cargo_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(error = error.to_string(), "Error while saving cargo_desc");
    }

    messages.clear();
}
