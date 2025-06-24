use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::NpcDesc;
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_npc_desc(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<NpcDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
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

        let mut currently_known_npc_desc = ::entity::npc_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.npc_type, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::npc_desc::Model = new.into();
                                global_app_state.npc_desc.insert(model.npc_type, model.clone());
                                if currently_known_npc_desc.contains_key(&model.npc_type) {
                                    let value = currently_known_npc_desc.get(&model.npc_type).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_npc_desc.remove(&model.npc_type);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::npc_desc::Model = new.into();
                                global_app_state.npc_desc.insert(model.npc_type, model.clone());
                                if currently_known_npc_desc.contains_key(&model.npc_type) {
                                    let value = currently_known_npc_desc.get(&model.npc_type).unwrap();
                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_npc_desc.remove(&model.npc_type);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
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
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(NpcDesc = id, error = error.to_string(), "Could not delete NpcDesc");
                                }
                                tracing::debug!("NpcDesc::Remove");
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
                let _ = ::entity::npc_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;
            }

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
