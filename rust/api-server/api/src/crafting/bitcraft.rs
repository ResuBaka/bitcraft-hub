use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use game_module::module_bindings::ProgressiveActionState;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_progressive_action_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ProgressiveActionState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::progressive_action_state::Column::EntityId)
                .update_columns([
                    ::entity::progressive_action_state::Column::BuildingEntityId,
                    ::entity::progressive_action_state::Column::FunctionType,
                    ::entity::progressive_action_state::Column::Progress,
                    ::entity::progressive_action_state::Column::RecipeId,
                    ::entity::progressive_action_state::Column::CraftCount,
                    ::entity::progressive_action_state::Column::LastCritOutcome,
                    ::entity::progressive_action_state::Column::OwnerEntityId,
                    ::entity::progressive_action_state::Column::LockExpiration,
                    ::entity::progressive_action_state::Column::Preparation,
                    ::entity::progressive_action_state::Column::Region,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut ids = vec![];
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("progressive_action_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_progressive_action_state = ::entity::progressive_action_state::Entity::find()
                                    .filter(::entity::progressive_action_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(
                                            error = error.to_string(),
                                            "Error while query whole progressive_action_state state"
                                        );
                                        vec![]
                                    })
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::progressive_action_state::Model = ::entity::progressive_action_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();
                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_progressive_action_state.entry(model.entity_id) {
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
                                       insert_multiple_progressive_action_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_progressive_action_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_progressive_action_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::progressive_action_state::Entity::delete_many()
                                        .filter(::entity::progressive_action_state::Column::EntityId.is_in(chunk_ids.clone()))
                                        .filter(::entity::progressive_action_state::Column::Region.eq(database_name.to_string()))
                                        .exec(&global_app_state.conn)
                                        .await
                                    {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(ProgressiveActionState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ProgressiveActionState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::progressive_action_state::Model = ::entity::progressive_action_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }

                                if ids.contains(&model.entity_id) {
                                    if let Some(index) = messages.iter().position(|value: &::entity::progressive_action_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                ids.push(model.entity_id);
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::progressive_action_state::Model = ::entity::progressive_action_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                if ids.contains(&model.entity_id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }

                                ids.push(model.entity_id);

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, reducer_name, .. } => {
                                let model: ::entity::progressive_action_state::Model = ::entity::progressive_action_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                #[allow(clippy::single_match)]
                                match reducer_name {
                                    Some("transfer_player_delayed") => {
                                        continue
                                    }
                                    _ => {}
                                }

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
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

                insert_multiple_progressive_action_state(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("ProgressiveActionState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::progressive_action_state::Entity::delete_many()
                        .filter(
                            ::entity::progressive_action_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            ProgressiveActionState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete ProgressiveActionState"
                        );
                    }
                }
                messages_delete.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_progressive_action_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::progressive_action_state::ActiveModel>,
) {
    let insert = ::entity::progressive_action_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ProgressiveActionState: {}", err)
    }

    messages.clear();
}
