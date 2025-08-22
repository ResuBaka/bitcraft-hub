use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::crafting_recipe;
use migration::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_crafting_recipe_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<
        SpacetimeUpdateMessages<game_module::module_bindings::CraftingRecipeDesc>,
    >,
    batch_size: usize,
    time_limit: Duration,
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
                                let mut currently_known_crafting_recipe = ::entity::crafting_recipe::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::crafting_recipe::Model = value.into();

                                    model
                                }) {
                                    global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_crafting_recipe.entry(model.id) {
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
                                       insert_multiple_crafting_recipe(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_crafting_recipe(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_crafting_recipe.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::crafting_recipe::Entity::delete_many().filter(::entity::crafting_recipe::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(CraftingRecipeDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete CraftingRecipeDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();

                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();
                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());

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

                insert_multiple_crafting_recipe(&global_app_state, &on_conflict, &mut messages)
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

async fn insert_multiple_crafting_recipe(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::crafting_recipe::ActiveModel>,
) {
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

    messages.clear();
}
