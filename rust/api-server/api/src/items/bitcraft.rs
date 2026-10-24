use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{
    flush_before_snapshot, reconcile_global, update_cache_if_changed,
};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use entity::item_desc;
use game_module::module_bindings::ItemDesc;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_item_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ItemDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict = sea_query::OnConflict::column(item_desc::Column::Id)
            .update_columns([
                item_desc::Column::Name,
                item_desc::Column::Description,
                item_desc::Column::Volume,
                item_desc::Column::Durability,
                item_desc::Column::ConvertToOnDurabilityZero,
                item_desc::Column::SecondaryKnowledgeId,
                item_desc::Column::ModelAssetName,
                item_desc::Column::IconAssetName,
                item_desc::Column::Tier,
                item_desc::Column::Tag,
                item_desc::Column::Rarity,
                item_desc::Column::CompendiumEntry,
                item_desc::Column::ItemListId,
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
                        record_worker_received("item_desc", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let filter = |keys: &[i32]| sea_orm::Condition::all().add(item_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<item_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush item_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<item_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    |model, _| update_cache_if_changed(&global_app_state.item_desc, model.id, model),
                                ).await {
                                    tracing::error!(%error, "Could not reconcile item_desc snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.item_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.item_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::item_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.item_desc.remove(&id);
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
                insert_multiple_item_desc(&global_app_state, &on_conflict, &mut messages).await;

                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("ItemDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::item_desc::Entity::delete_many()
                        .filter(::entity::item_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            ItemDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete ItemDesc"
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

async fn insert_multiple_item_desc(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::item_desc::ActiveModel>,
) {
    let insert = ::entity::item_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ItemDesc: {}", err)
    }

    messages.clear();
}
