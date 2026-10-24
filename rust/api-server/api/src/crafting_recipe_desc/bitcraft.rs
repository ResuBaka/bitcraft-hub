use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{
    flush_before_snapshot, reconcile_global, update_cache_if_changed,
};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use entity::crafting_recipe;
use migration::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, sea_query};
use std::time::Duration;

pub(crate) fn start_worker_crafting_recipe_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<
        SpacetimeUpdateMessages<game_module::module_bindings::CraftingRecipeDesc>,
    >,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
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

        let mut messages = Vec::new();
        let mut messages_delete = Vec::new();
        loop {
            let mut timer = BatchTimer::new(time_limit);

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break; };
                        timer.start();
                        record_worker_received("crafting_recipe_desc", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let filter = |keys: &[i32]| sea_orm::Condition::all().add(::entity::crafting_recipe::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::crafting_recipe::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush crafting_recipe before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<::entity::crafting_recipe::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    |model, _| update_cache_if_changed(&global_app_state.crafting_recipe_desc, model.id, model),
                                ).await {
                                    tracing::error!(%error, "Could not reconcile crafting_recipe snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();

                                update_cache_if_changed(&global_app_state.crafting_recipe_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();
                                update_cache_if_changed(&global_app_state.crafting_recipe_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
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
                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => {
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

            if !messages_delete.is_empty() {
                tracing::debug!("CraftingRecipeDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::crafting_recipe::Entity::delete_many()
                        .filter(::entity::crafting_recipe::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            CraftingRecipeDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete CraftingRecipeDesc"
                        );
                    }
                }
                messages_delete.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() && rx.is_empty()
            {
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
    let insert = ::entity::crafting_recipe::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting CraftingRecipeDesc: {}", err)
    }

    messages.clear();
}
