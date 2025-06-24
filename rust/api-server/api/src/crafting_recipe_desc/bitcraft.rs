use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::crafting_recipe;
use kanal::AsyncReceiver;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_crafting_recipe_desc(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<game_module::module_bindings::CraftingRecipeDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
            .update_columns([
                crafting_recipe::Column::Name,
                crafting_recipe::Column::TimeRequirement,
                crafting_recipe::Column::StaminaRequirement,
                crafting_recipe::Column::ToolDurabilityLost,
                crafting_recipe::Column::BuildingRequirement,
                crafting_recipe::Column::LevelRequirements,
                crafting_recipe::Column::ToolRequirements,
                crafting_recipe::Column::ConsumedItemStacks,
                crafting_recipe::Column::DiscoveryTriggers,
                crafting_recipe::Column::RequiredKnowledges,
                crafting_recipe::Column::RequiredClaimTechId,
                crafting_recipe::Column::FullDiscoveryScore,
                crafting_recipe::Column::ExperiencePerProgress,
                crafting_recipe::Column::AllowUseHands,
                crafting_recipe::Column::CraftedItemStacks,
                crafting_recipe::Column::IsPassive,
                crafting_recipe::Column::ActionsRequired,
                crafting_recipe::Column::ToolMeshIndex,
                crafting_recipe::Column::RecipePerformanceId,
            ])
            .to_owned();

        let mut currently_known_crafting_recipe = ::entity::crafting_recipe::Entity::find()
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
                                let model: ::entity::crafting_recipe::Model = new.into();

                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                if currently_known_crafting_recipe.contains_key(&model.id) {
                                    let value = currently_known_crafting_recipe.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_crafting_recipe.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();
                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                if currently_known_crafting_recipe.contains_key(&model.id) {
                                    let value = currently_known_crafting_recipe.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_crafting_recipe.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::crafting_recipe::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                global_app_state.crafting_recipe_desc.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CraftingRecipeDesc = id, error = error.to_string(), "Could not delete BuildingNicknameState");
                                }

                                tracing::debug!("CraftingRecipeDesc::Remove");
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
                tracing::debug!(
                    "CraftingRecipeDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::crafting_recipe::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!(
                        "Error inserting CraftingRecipeDesc: {}",
                        insert.unwrap_err()
                    )
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
