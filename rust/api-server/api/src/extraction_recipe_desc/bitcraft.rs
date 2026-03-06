use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use entity::extraction_recipe_desc;
use migration::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, sea_query};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_extraction_recipe_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<
        SpacetimeUpdateMessages<game_module::module_bindings::ExtractionRecipeDesc>,
    >,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(extraction_recipe_desc::Column::Id)
            .update_columns([
                extraction_recipe_desc::Column::ResourceId,
                extraction_recipe_desc::Column::ExtractedItemStacks,
                extraction_recipe_desc::Column::ToolRequirements,
                extraction_recipe_desc::Column::AllowUseHands,
                extraction_recipe_desc::Column::TimeRequirement,
                extraction_recipe_desc::Column::StaminaRequirement,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("extraction_recipe_desc", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::extraction_recipe_desc::Model = value.into();
                                    model
                                }) {
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                       insert_multiple(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                }
                                if !local_messages.is_empty() {
                                    insert_multiple(&global_app_state, &on_conflict, &mut local_messages).await;
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::extraction_recipe_desc::Model = new.into();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::extraction_recipe_desc::Model = new.into();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::extraction_recipe_desc::Model = delete.into();
                                let id = model.id;
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = &mut timer => {
                        break;
                    }
                    else => {
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                insert_multiple(&global_app_state, &on_conflict, &mut messages).await;
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::extraction_recipe_desc::Entity::delete_many()
                        .filter(::entity::extraction_recipe_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(ExtractionRecipeDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ExtractionRecipeDesc");
                    }
                }
                messages_delete.clear();
            }

            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::extraction_recipe_desc::ActiveModel>,
) {
    let insert = ::entity::extraction_recipe_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ExtractionRecipeDesc: {}", err);
    }

    messages.clear();
}
