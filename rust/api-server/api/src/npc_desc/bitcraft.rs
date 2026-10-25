use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{
    flush_before_snapshot, reconcile_global, update_cache_if_changed,
};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::NpcDesc;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_npc_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<NpcDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("npc_desc");
        let on_conflict = sea_query::OnConflict::column(::entity::npc_desc::Column::NpcType)
            .update_columns([
                ::entity::npc_desc::Column::Name,
                ::entity::npc_desc::Column::Population,
                ::entity::npc_desc::Column::Speed,
                ::entity::npc_desc::Column::MinTimeAtRuin,
                ::entity::npc_desc::Column::MaxTimeAtRuin,
                ::entity::npc_desc::Column::PrefabAddress,
                ::entity::npc_desc::Column::IconAddress,
                ::entity::npc_desc::Column::ForceMarketMode,
                ::entity::npc_desc::Column::TaskSkillCheck,
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
                                let filter = |keys: &[i32]| sea_orm::Condition::all().add(::entity::npc_desc::Column::NpcType.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::npc_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush npc_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<::entity::npc_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.npc_type, filter,
                                    |model, _| update_cache_if_changed(&global_app_state.npc_desc, model.npc_type, model),
                                ).await {
                                    tracing::error!(%error, "Could not reconcile npc_desc snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::npc_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.npc_desc, model.npc_type, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.npc_type) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::npc_desc::Model = new.into();
                                update_cache_if_changed(&global_app_state.npc_desc, model.npc_type, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.npc_type) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::npc_desc::Model = delete.into();
                                let id = model.npc_type;
                                global_app_state.npc_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.npc_type.as_ref() == &model.npc_type) {
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
                insert_multiple_npc_desc(&global_app_state, &on_conflict, &mut messages).await;
            }

            if !messages_delete.is_empty() {
                tracing::debug!("NpcDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::npc_desc::Entity::delete_many()
                        .filter(::entity::npc_desc::Column::NpcType.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            NpcDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete NpcDesc"
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

async fn insert_multiple_npc_desc(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::npc_desc::ActiveModel>,
) {
    let insert = ::entity::npc_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting NpcDesc: {}", err)
    }

    messages.clear();
}
