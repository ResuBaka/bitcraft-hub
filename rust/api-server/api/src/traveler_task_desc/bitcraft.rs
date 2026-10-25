use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{
    flush_before_snapshot, reconcile_global, update_cache_if_changed,
};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::TravelerTaskDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_traveler_task_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("traveler_task_desc");
        let on_conflict = sea_query::OnConflict::column(::entity::traveler_task_desc::Column::Id)
            .update_columns([
                ::entity::traveler_task_desc::Column::SkillId,
                ::entity::traveler_task_desc::Column::MinLevel,
                ::entity::traveler_task_desc::Column::MaxLevel,
                ::entity::traveler_task_desc::Column::RequiredItems,
                ::entity::traveler_task_desc::Column::RewardedItems,
                ::entity::traveler_task_desc::Column::RewardedExperience,
                ::entity::traveler_task_desc::Column::Description,
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
                                let filter = |keys: &[i32]| sea_orm::Condition::all().add(::entity::traveler_task_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::traveler_task_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush traveler_task_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<::entity::traveler_task_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    |model, _| update_cache_if_changed(&global_app_state.traveler_task_desc, model.id, model),
                                ).await {
                                    tracing::error!(%error, "Could not reconcile traveler_task_desc snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.traveler_task_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::traveler_task_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.traveler_task_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::traveler_task_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.traveler_task_desc.remove(&id);
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
                        break;
                    }
                    else => {
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                insert_multiple_traveler_task_desc(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            if !messages_delete.is_empty() {
                tracing::debug!("TravelerTaskDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::traveler_task_desc::Entity::delete_many()
                        .filter(::entity::traveler_task_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            TravelerTaskDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete TravelerTaskDesc"
                        );
                    }
                }
                messages_delete.clear();
            }

            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() && rx.is_empty()
            {
                break;
            }
        }
    });
}

async fn insert_multiple_traveler_task_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::traveler_task_desc::ActiveModel>,
) {
    let insert = ::entity::traveler_task_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting TravelerTaskDesc: {}", err)
    }

    messages.clear();
}
