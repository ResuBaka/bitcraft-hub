use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::building_state::ActiveModel;
use game_module::module_bindings::{BuildingDesc, BuildingNicknameState, BuildingState};
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, DbErr, EntityTrait, InsertManyResult, IntoActiveModel, QueryFilter};
use std::collections::HashMap;
use std::time::Duration;

pub(crate) fn start_worker_building_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("building_state");
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_state::Column::EntityId])
                .update_columns([
                    ::entity::building_state::Column::ClaimEntityId,
                    ::entity::building_state::Column::DirectionIndex,
                    ::entity::building_state::Column::BuildingDescriptionId,
                    ::entity::building_state::Column::ConstructedByPlayerEntityId,
                    ::entity::building_state::Column::Region,
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
                                // Flush older live events before applying a newer snapshot.
                                if !messages.is_empty() {
                                    if let Err(error) = insert_multiple_building_state(&global_app_state, &on_conflict, &mut messages).await {
                                        tracing::error!(%error, "Could not flush building updates before initial sync");
                                    }
                                }
                                if !messages_delete.is_empty() {
                                    if let Err(error) = ::entity::building_state::Entity::delete_many()
                                        .filter(::entity::building_state::Column::EntityId.is_in(std::mem::take(&mut messages_delete)))
                                        .exec(&global_app_state.conn).await {
                                        tracing::error!(%error, "Could not flush building deletes before initial sync");
                                    }
                                }
                                let incoming = data.into_iter().map(|value| ::entity::building_state::ModelBuilder::new(value).with_region(database_name).build());
                                if let Err(error) = crate::websocket::initial_sync::reconcile::<::entity::building_state::Entity, _, _, _, _, _>(
                                    &global_app_state.conn,
                                    ::entity::building_state::Column::Region,
                                    database_name,
                                    &on_conflict,
                                    batch_size,
                                    incoming,
                                    |model| model.entity_id,
                                    |ids| crate::websocket::initial_sync::bigint_keys_filter(::entity::building_state::Column::EntityId, ids),
                                    |_, _| {},
                                ).await {
                                    tracing::error!(%error, region = database_name, "Could not reconcile initial buildings");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(new).with_region(database_name).build();

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::building_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(new).with_region(database_name).build();

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::building_state::Model = ::entity::building_state::ModelBuilder::new(delete).with_region(database_name).build();
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
                tracing::debug!(
                    "BuildingState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert =
                    insert_multiple_building_state(&global_app_state, &on_conflict, &mut messages)
                        .await;

                if let Err(err) = insert {
                    tracing::error!("Error inserting ItemListDesc: {}", err)
                }

                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("BuildingState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::building_state::Entity::delete_many()
                        .filter(::entity::building_state::Column::EntityId.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            BuildingState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete BuildingState"
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

async fn insert_multiple_building_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_state::ActiveModel>,
) -> Result<InsertManyResult<ActiveModel>, DbErr> {
    let insert = ::entity::building_state::Entity::insert_many(std::mem::take(messages))
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    insert
}

pub(crate) fn start_worker_building_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("building_desc");
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
                                use crate::websocket::initial_sync::{flush_before_snapshot, reconcile_global, update_cache_if_changed};
                                let filter = |keys: &[i64]| sea_orm::Condition::all().add(::entity::building_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::building_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush building_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_global::<::entity::building_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    |model, _| update_cache_if_changed(&global_app_state.building_desc, model.id, model),
                                ).await {
                                    tracing::error!(%error, "Could not reconcile building_desc snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();

                                crate::websocket::initial_sync::update_cache_if_changed(&global_app_state.building_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();
                                crate::websocket::initial_sync::update_cache_if_changed(&global_app_state.building_desc, model.id, &model);
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
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
                tracing::debug!(
                    "BuildingDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );

                insert_multiple_build_desc(&global_app_state, &on_conflict, &mut messages).await;
                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("BuildingDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::building_desc::Entity::delete_many()
                        .filter(::entity::building_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            BuildingDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete BuildingDesc"
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

async fn insert_multiple_build_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_desc::ActiveModel>,
) {
    let insert = ::entity::building_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting BuildingDesc: {}", err)
    }

    messages.clear();
}

pub(crate) fn start_worker_building_nickname_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<BuildingNicknameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("building_nickname_state");
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_nickname_state::Column::EntityId])
                .update_columns([
                    ::entity::building_nickname_state::Column::Nickname,
                    ::entity::building_nickname_state::Column::Region,
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
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_building_nickname_state = ::entity::building_nickname_state::Entity::find()
                                    .filter(::entity::building_nickname_state::Column::Region.eq(database_name))
                                    .all(&global_app_state.conn)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(
                                            error = error.to_string(),
                                            "Error while query whole building_nickname_state state"
                                        );
                                        vec![]
                                    })
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(value).with_region(database_name).build();

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
                                    if let Err(error) = ::entity::building_nickname_state::Entity::delete_many()
                                        .filter(::entity::building_nickname_state::Column::EntityId.is_in(chunk_ids.clone()))
                                        .filter(::entity::building_nickname_state::Column::Region.eq(database_name))
                                        .exec(&global_app_state.conn).await
                                    {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(BuildingNicknameState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete BuildingNicknameState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(new).with_region(database_name).build();

                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(new).with_region(database_name).build();
                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::building_nickname_state::Model = ::entity::building_nickname_state::ModelBuilder::new(delete).with_region(database_name).build();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.building_nickname_state.remove(&id);
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

            if !messages_delete.is_empty() {
                tracing::debug!("BuildingNicknameState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::building_nickname_state::Entity::delete_many()
                        .filter(
                            ::entity::building_nickname_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            BuildingNicknameState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete BuildingNicknameState"
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

async fn insert_multiple_building_nickname_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::building_nickname_state::ActiveModel>,
) {
    let insert = ::entity::building_nickname_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting BuildingNicknameState: {}", err)
    }

    messages.clear();
}
