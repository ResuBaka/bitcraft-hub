use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{flush_before_snapshot, reconcile_global};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::collectible_desc;
use game_module::module_bindings::CollectibleDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_collectible_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<CollectibleDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("collectible_desc");
        let on_conflict = sea_query::OnConflict::column(collectible_desc::Column::Id)
            .update_columns([
                collectible_desc::Column::Name,
                collectible_desc::Column::Description,
                collectible_desc::Column::CollectibleType,
                collectible_desc::Column::InvalidatesType,
                collectible_desc::Column::AutoCollect,
                collectible_desc::Column::CollectibleRarity,
                collectible_desc::Column::StartingLoadout,
                collectible_desc::Column::Locked,
                collectible_desc::Column::Variant,
                collectible_desc::Column::Color,
                collectible_desc::Column::Emission,
                collectible_desc::Column::MaxEquipCount,
                collectible_desc::Column::ModelAssetName,
                collectible_desc::Column::VariantMaterial,
                collectible_desc::Column::IconAssetName,
                collectible_desc::Column::Tag,
                collectible_desc::Column::DisplayString,
                collectible_desc::Column::ItemDeedId,
                collectible_desc::Column::RequiredKnowledgesToUse,
                collectible_desc::Column::RequiredKnowledgesToConvert,
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
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let filter = |keys: &[i32]| sea_orm::Condition::all().add(collectible_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<collectible_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush collectible_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<collectible_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter, |_, _| {},
                                ).await {
                                    tracing::error!(%error, "Could not reconcile collectible_desc snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::collectible_desc::Model = new.into();
                                // global_app_state.collectible_desc.insert(model.id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::collectible_desc::Model = new.into();
                                // global_app_state.collectible_desc.insert(model.id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::collectible_desc::Model = delete.into();
                                let id = model.id;
                                // global_app_state.collectible_desc.remove(&id);
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

                insert_multiple_collectible_desc(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            if !messages_delete.is_empty() {
                tracing::debug!("CollectibleDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::collectible_desc::Entity::delete_many()
                        .filter(::entity::collectible_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            CollectibleDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete CollectibleDesc"
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

async fn insert_multiple_collectible_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::collectible_desc::ActiveModel>,
) {
    let result = ::entity::collectible_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving collectible_desc"
        );
    }

    messages.clear();
}
