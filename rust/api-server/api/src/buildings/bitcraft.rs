use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::{BuildingDesc, BuildingNicknameState, BuildingState};
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_building_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_state::Column::EntityId])
                .update_columns([
                    ::entity::building_state::Column::ClaimEntityId,
                    ::entity::building_state::Column::DirectionIndex,
                    ::entity::building_state::Column::BuildingDescriptionId,
                    ::entity::building_state::Column::ConstructedByPlayerEntityId,
                ])
                .to_owned();

        let mut currently_known_building_state = ::entity::building_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = HashMap::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_state::Model = new.into();

                                if currently_known_building_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.insert(model.entity_id, model);
                                    } else {
                                        currently_known_building_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.insert(model.entity_id, model);
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_state::Model = new.into();

                                if currently_known_building_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.insert(model.entity_id, model);
                                    } else {
                                        currently_known_building_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.insert(model.entity_id, model);
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_state::Model = delete.into();
                                let id = model.entity_id;

                                messages.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingState = id, error = error.to_string(), "Could not delete BuildingState");
                                }

                                tracing::debug!("BuildingState::Remove");
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
                tracing::debug!(
                    "BuildingState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::building_state::Entity::insert_many(
                    messages
                        .values()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting BuildingState: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

pub(crate) fn start_worker_building_desc(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([::entity::building_desc::Column::Id])
            .update_columns([
                ::entity::building_desc::Column::Functions,
                ::entity::building_desc::Column::Name,
                ::entity::building_desc::Column::Description,
                ::entity::building_desc::Column::RestedBuffDuration,
                ::entity::building_desc::Column::LightRadius,
                ::entity::building_desc::Column::ModelAssetName,
                ::entity::building_desc::Column::IconAssetName,
                ::entity::building_desc::Column::Unenterable,
                ::entity::building_desc::Column::Wilderness,
                ::entity::building_desc::Column::Footprint,
                ::entity::building_desc::Column::MaxHealth,
                ::entity::building_desc::Column::IgnoreDamage,
                ::entity::building_desc::Column::DefenseLevel,
                ::entity::building_desc::Column::Decay,
                ::entity::building_desc::Column::Maintenance,
                ::entity::building_desc::Column::BuildPermission,
                ::entity::building_desc::Column::InteractPermission,
                ::entity::building_desc::Column::HasAction,
                ::entity::building_desc::Column::ShowInCompendium,
                ::entity::building_desc::Column::IsRuins,
                ::entity::building_desc::Column::NotDeconstructible,
            ])
            .to_owned();

        let mut currently_known_building_desc = ::entity::building_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
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
                                let model: ::entity::building_desc::Model = new.into();

                                global_app_state.building_desc.insert(model.id, model.clone());
                                if currently_known_building_desc.contains_key(&model.id) {
                                    let value = currently_known_building_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();
                               global_app_state.building_desc.insert(model.id, model.clone());
                                if currently_known_building_desc.contains_key(&model.id) {
                                    let value = currently_known_building_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                global_app_state.building_desc.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingDesc = id, error = error.to_string(), "Could not delete BuildingDesc");
                                }

                                tracing::debug!("BuildingDesc::Remove");
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
                tracing::debug!(
                    "BuildingDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::building_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!("Error inserting BuildingDesc: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

pub(crate) fn start_worker_building_nickname_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingNicknameState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_nickname_state::Column::EntityId])
                .update_columns([::entity::building_nickname_state::Column::Nickname])
                .to_owned();

        let mut currently_known_building_nickname_state =
            ::entity::building_nickname_state::Entity::find()
                .all(&global_app_state.conn)
                .await
                .map_or(vec![], |aa| aa)
                .into_iter()
                .map(|value| (value.entity_id, value))
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
                                let model: ::entity::building_nickname_state::Model = new.into();

                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if currently_known_building_nickname_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_nickname_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_nickname_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_nickname_state::Model = new.into();
                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if currently_known_building_nickname_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_nickname_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_nickname_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_nickname_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.building_nickname_state.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingNicknameState = id, error = error.to_string(), "Could not delete BuildingNicknameState");
                                }

                                tracing::debug!("BuildingNicknameState::Remove");
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
                tracing::debug!(
                    "BuildingNicknameState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert =
                    ::entity::building_nickname_state::Entity::insert_many(messages.clone())
                        .on_conflict(on_conflict.clone())
                        .exec(&global_app_state.conn)
                        .await;

                if insert.is_err() {
                    tracing::error!(
                        "Error inserting BuildingNicknameState: {}",
                        insert.unwrap_err()
                    )
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
