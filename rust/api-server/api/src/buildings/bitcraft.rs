use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::{BuildingDesc, BuildingNicknameState, BuildingState};
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_building_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingState>>,
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

        loop {
            let mut messages = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_building_state = ::entity::building_state::Entity::find()
                                    .filter(::entity::building_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;

                                    match currently_known_building_state.entry(model.entity_id) {
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
                                       insert_multiple_building_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_building_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_building_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::building_state::Entity::delete_many().filter(::entity::building_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(BuildingState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete BuildingState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

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
                insert_multiple_building_state(&global_app_state, &on_conflict, &mut messages)
                    .await;

                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_building_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_state::ActiveModel>,
) {
    let insert = ::entity::building_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting ItemListDesc: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_building_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingDesc>>,
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

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_building_desc = ::entity::building_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::building_desc::Model = value.into();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    global_app_state.building_desc.insert(model.id, model.clone());
                                    match currently_known_building_desc.entry(model.id) {
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
                                       insert_multiple_build_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_build_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_building_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::building_desc::Entity::delete_many().filter(::entity::building_desc::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(BuildingDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete BuildingDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();

                                global_app_state.building_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();
                                global_app_state.building_desc.insert(model.id, model.clone());
                                messages.push(model.into_active_model());

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

                insert_multiple_build_desc(&global_app_state, &on_conflict, &mut messages).await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_build_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_desc::ActiveModel>,
) {
    let insert = ::entity::building_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting BuildingDesc: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_building_nickname_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingNicknameState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_nickname_state::Column::EntityId])
                .update_columns([::entity::building_nickname_state::Column::Nickname])
                .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_building_nickname_state = ::entity::building_nickname_state::Entity::find()
                                    .filter(::entity::building_nickname_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                    match currently_known_building_nickname_state.entry(model.entity_id) {
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
                                       insert_multiple_building_nickname_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_building_nickname_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_building_nickname_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::building_nickname_state::Entity::delete_many().filter(::entity::building_nickname_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(BuildingNicknameState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete BuildingNicknameState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
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
                insert_multiple_building_nickname_state(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_building_nickname_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_nickname_state::ActiveModel>,
) {
    let insert = ::entity::building_nickname_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!(
            "Error inserting BuildingNicknameState: {}",
            insert.unwrap_err()
        )
    }

    messages.clear();
}
