use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use game_module::module_bindings::NpcDesc;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_npc_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<NpcDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("npc_desc", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_npc_desc = ::entity::npc_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(
                                            error = error.to_string(),
                                            "Error while query whole npc_desc state"
                                        );
                                        vec![]
                                    })
                                    .into_iter()
                                    .map(|value| (value.npc_type, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::npc_desc::Model = value.into();

                                    model
                                }) {
                                    global_app_state.npc_desc.insert(model.npc_type, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_npc_desc.entry(model.npc_type) {
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
                                       insert_multiple_npc_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_npc_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_npc_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::npc_desc::Entity::delete_many().filter(::entity::npc_desc::Column::NpcType.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(NpcDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete NpcDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::npc_desc::Model = new.into();
                                global_app_state.npc_desc.insert(model.npc_type, model.clone());
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
                                global_app_state.npc_desc.insert(model.npc_type, model.clone());
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
                    _ = &mut timer => {
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

            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
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
    let insert = ::entity::npc_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting NpcDesc: {}", err)
    }

    messages.clear();
}
