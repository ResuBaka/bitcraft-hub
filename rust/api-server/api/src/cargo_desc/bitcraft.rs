use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::cargo_desc;
use game_module::module_bindings::CargoDesc;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_cargo_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<CargoDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
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

        let mut currently_known_cargo_desc = ::entity::cargo_desc::Entity::find()
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
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                if currently_known_cargo_desc.contains_key(&model.id) {
                                    let value = currently_known_cargo_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_cargo_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                if currently_known_cargo_desc.contains_key(&model.id) {
                                    let value = currently_known_cargo_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_cargo_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
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
                let result = ::entity::cargo_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if let Err(error) = result {
                    tracing::error!(error = error.to_string(), "Error while saving cargo_desc");
                }
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
