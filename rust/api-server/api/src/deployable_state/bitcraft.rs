use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::deployable_state;
use game_module::module_bindings::DeployableStateV2;
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel};
use std::time::Duration;

pub(crate) fn start_worker_deployable_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<DeployableStateV2>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("deployable_state_v2");
        let on_conflict = sea_query::OnConflict::column(deployable_state::Column::EntityId)
            .update_columns([
                deployable_state::Column::OwnerId,
                deployable_state::Column::ClaimEntityId,
                deployable_state::Column::Direction,
                deployable_state::Column::DeployableDescriptionId,
                deployable_state::Column::Nickname,
                deployable_state::Column::Hidden,
                deployable_state::Column::Region,
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
                        receive_metrics.record(1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                insert_multiple_deployable_state(&global_app_state, &on_conflict, &mut messages).await;
                                if !messages_delete.is_empty() {
                                    if let Err(error) = deployable_state::Entity::delete_many()
                                        .filter(deployable_state::Column::EntityId.is_in(std::mem::take(&mut messages_delete)))
                                        .exec(&global_app_state.conn).await {
                                        tracing::error!(%error, "Could not flush deployable deletes before initial sync");
                                    }
                                }
                                let incoming = data.into_iter().map(|value| deployable_state::ModelBuilder::new(value).with_region(database_name).build());
                                if let Err(error) = crate::websocket::initial_sync::reconcile::<deployable_state::Entity, _, _, _, _, _>(
                                    &global_app_state.conn,
                                    deployable_state::Column::Region,
                                    database_name,
                                    &on_conflict,
                                    batch_size,
                                    incoming,
                                    |model| model.entity_id,
                                    |ids| sea_query::Condition::all().add(deployable_state::Column::EntityId.is_in(ids.iter().copied())),
                                    |model, _| { global_app_state.deployable_state.insert(model.entity_id, model.clone()); },
                                ).await {
                                    tracing::error!(%error, region = database_name, "Could not reconcile initial deployables");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::deployable_state::Model = ::entity::deployable_state::ModelBuilder::new(new).with_region(database_name).build();
                                global_app_state.deployable_state.insert(model.entity_id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::deployable_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::deployable_state::Model = ::entity::deployable_state::ModelBuilder::new(new).with_region(database_name).build();
                                global_app_state.deployable_state.insert(model.entity_id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::deployable_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::deployable_state::Model = ::entity::deployable_state::ModelBuilder::new(delete).with_region(database_name).build();
                                global_app_state.deployable_state.remove(&model.entity_id);
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                insert_multiple_deployable_state(&global_app_state, &on_conflict, &mut messages)
                    .await;
                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("DeployableStateV2::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::deployable_state::Entity::delete_many()
                        .filter(
                            ::entity::deployable_state::Column::EntityId.is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            DeployableStateV2 = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete DeployableStateV2"
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

async fn insert_multiple_deployable_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::deployable_state::ActiveModel>,
) {
    if messages.is_empty() {
        return;
    }
    let insert = ::entity::deployable_state::Entity::insert_many(std::mem::take(messages))
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting deployable_state: {}", err)
    }
}
