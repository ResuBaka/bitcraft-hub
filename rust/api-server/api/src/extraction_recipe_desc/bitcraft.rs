use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use entity::extraction_recipe_desc;
use migration::OnConflict;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait, sea_query};
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
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
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
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::extraction_recipe_desc::Model = new.into();
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::extraction_recipe_desc::Model = delete.into();
                                let id = model.id;
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ExtractionRecipeDesc = id, error = error.to_string(), "Could not delete ExtractionRecipeDesc");
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

            if messages.is_empty() && rx.is_closed() {
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
