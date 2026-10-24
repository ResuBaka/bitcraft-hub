use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use game_module::module_bindings::{
    DimensionDescriptionState, InteriorNetworkDesc, PermissionState, PlayerHousingState,
    PortalState,
};
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, DbErr, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Interior Network Desc Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_interior_network_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<InteriorNetworkDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::interior_network_desc::Column::BuildingId])
                .update_columns([
                    ::entity::interior_network_desc::Column::DimensionType,
                    ::entity::interior_network_desc::Column::ChildInteriorInstances,
                    ::entity::interior_network_desc::Column::Region,
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
                        record_worker_received("interior_network_desc", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::interior_network_desc::ModelBuilder::new(entry)
                                        .with_region(database_name)
                                        .build();
                                    if let Some(index) = local_messages.iter().position(|value: &::entity::interior_network_desc::ActiveModel| value.building_id.as_ref() == &model.building_id) {
                                        local_messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_interior_network_desc(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting InteriorNetworkDesc: {}", e);
                                        }
                                    }
                                }
                                if !local_messages.is_empty() {
                                    let insert = insert_many_interior_network_desc(
                                        &global_app_state,
                                        &on_conflict,
                                        &mut local_messages,
                                    )
                                    .await;
                                    if let Err(e) = insert {
                                        tracing::error!("Error inserting InteriorNetworkDesc: {}", e);
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(new).with_region(database_name).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.building_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::interior_network_desc::ActiveModel| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(new).with_region(database_name).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.building_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(delete).with_region(database_name).build();
                                if let Some(index) = messages.iter().position(|value| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(model.building_id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                tracing::debug!(
                    "InteriorNetworkDesc -> Processing {} messages in batch",
                    messages.len()
                );
                let insert = insert_many_interior_network_desc(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting InteriorNetworkDesc: {}", e);
                }
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::interior_network_desc::Entity::delete_many()
                        .filter(
                            ::entity::interior_network_desc::Column::BuildingId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            InteriorNetworkDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete InteriorNetworkDesc"
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

async fn insert_many_interior_network_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::interior_network_desc::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::interior_network_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Dimension Description State Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_dimension_description_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<DimensionDescriptionState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::dimension_description_state::Column::EntityId,
        ])
        .update_columns([
            ::entity::dimension_description_state::Column::DimensionNetworkEntityId,
            ::entity::dimension_description_state::Column::DimensionId,
            ::entity::dimension_description_state::Column::DimensionType,
            ::entity::dimension_description_state::Column::InteriorInstanceId,
            ::entity::dimension_description_state::Column::Region,
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
                        record_worker_received("dimension_description_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::dimension_description_state::ModelBuilder::new(entry)
                                        .with_region(database_name)
                                        .build();
                                    if let Some(index) = local_messages.iter().position(|value: &::entity::dimension_description_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        local_messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_dimension_description_state(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting DimensionDescriptionState: {}", e);
                                        }
                                    }
                                }
                                if !local_messages.is_empty() {
                                    let insert = insert_many_dimension_description_state(
                                        &global_app_state,
                                        &on_conflict,
                                        &mut local_messages,
                                    )
                                    .await;
                                    if let Err(e) = insert {
                                        tracing::error!("Error inserting DimensionDescriptionState: {}", e);
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::dimension_description_state::ModelBuilder::new(new).with_region(database_name).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::dimension_description_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::dimension_description_state::ModelBuilder::new(new).with_region(database_name).build();
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
                                let model = ::entity::dimension_description_state::ModelBuilder::new(delete).with_region(database_name).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(model.entity_id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                let insert = insert_many_dimension_description_state(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting DimensionDescriptionState: {}", e);
                }
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::dimension_description_state::Entity::delete_many()
                        .filter(
                            ::entity::dimension_description_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            DimensionDescriptionState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete DimensionDescriptionState"
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

async fn insert_many_dimension_description_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::dimension_description_state::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::dimension_description_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Player Housing State Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_player_housing_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerHousingState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::player_housing_state::Column::EntityId])
                .update_columns([
                    ::entity::player_housing_state::Column::EntranceBuildingEntityId,
                    ::entity::player_housing_state::Column::NetworkEntityId,
                    ::entity::player_housing_state::Column::ExitPortalEntityId,
                    ::entity::player_housing_state::Column::Rank,
                    ::entity::player_housing_state::Column::LockedUntil,
                    ::entity::player_housing_state::Column::IsEmpty,
                    ::entity::player_housing_state::Column::RegionIndex,
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
                        record_worker_received("player_housing_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::player_housing_state::ModelBuilder::new(entry)
                                        .build();
                                    if let Some(index) = local_messages.iter().position(|value: &::entity::player_housing_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        local_messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_player_housing_state(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting PlayerHousingState: {}", e);
                                        }
                                    }
                                }
                                if !local_messages.is_empty() {
                                    let insert = insert_many_player_housing_state(
                                        &global_app_state,
                                        &on_conflict,
                                        &mut local_messages,
                                    )
                                    .await;
                                    if let Err(e) = insert {
                                        tracing::error!("Error inserting PlayerHousingState: {}", e);
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(new).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::player_housing_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(new).build();
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
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(delete).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(model.entity_id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                let insert = insert_many_player_housing_state(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting PlayerHousingState: {}", e);
                }
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::player_housing_state::Entity::delete_many()
                        .filter(
                            ::entity::player_housing_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            PlayerHousingState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete PlayerHousingState"
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

// Keep originals until exec succeeds, but clone rows lazily while SeaORM builds
// the query rather than allocating a second full ActiveModel batch.
async fn insert_many_player_housing_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::player_housing_state::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::player_housing_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Permission State Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_permission_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PermissionState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::permission_state::Column::EntityId])
                .update_columns([
                    ::entity::permission_state::Column::OrdainedEntityId,
                    ::entity::permission_state::Column::AllowedEntityId,
                    ::entity::permission_state::Column::Group,
                    ::entity::permission_state::Column::Rank,
                    ::entity::permission_state::Column::Region,
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
                        record_worker_received("permission_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::permission_state::ModelBuilder::new(entry)
                                        .with_region(database_name)
                                        .build();
                                    if let Some(index) = local_messages.iter().position(|value: &::entity::permission_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        local_messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_permission_state(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting PermissionState: {}", e);
                                        }
                                    }
                                }
                                if !local_messages.is_empty() {
                                    let insert = insert_many_permission_state(
                                        &global_app_state,
                                        &on_conflict,
                                        &mut local_messages,
                                    )
                                    .await;
                                    if let Err(e) = insert {
                                        tracing::error!("Error inserting PermissionState: {}", e);
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::permission_state::ModelBuilder::new(new).with_region(database_name).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::permission_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::permission_state::ModelBuilder::new(new).with_region(database_name).build();
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
                                let model = ::entity::permission_state::ModelBuilder::new(delete).with_region(database_name).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(model.entity_id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                let insert =
                    insert_many_permission_state(&global_app_state, &on_conflict, &mut messages)
                        .await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting PermissionState: {}", e);
                }
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::permission_state::Entity::delete_many()
                        .filter(
                            ::entity::permission_state::Column::EntityId.is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            PermissionState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete PermissionState"
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

async fn insert_many_permission_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::permission_state::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::permission_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Portal State Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_portal_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PortalState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::portal_state::Column::EntityId])
                .update_columns([
                    ::entity::portal_state::Column::TargetBuildingEntityId,
                    ::entity::portal_state::Column::DestinationX,
                    ::entity::portal_state::Column::DestinationZ,
                    ::entity::portal_state::Column::DestinationDimension,
                    ::entity::portal_state::Column::Enabled,
                    ::entity::portal_state::Column::AllowDeployables,
                    ::entity::portal_state::Column::Region,
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
                        record_worker_received("portal_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::portal_state::ModelBuilder::new(entry)
                                        .with_region(database_name)
                                        .build();
                                    if let Some(index) = local_messages.iter().position(|value: &::entity::portal_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        local_messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_portal_state(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting PortalState: {}", e);
                                        }
                                    }
                                }
                                if !local_messages.is_empty() {
                                    let insert = insert_many_portal_state(
                                        &global_app_state,
                                        &on_conflict,
                                        &mut local_messages,
                                    )
                                    .await;
                                    if let Err(e) = insert {
                                        tracing::error!("Error inserting PortalState: {}", e);
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::portal_state::ModelBuilder::new(new).with_region(database_name).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                if let Some(index) = messages.iter().position(|value: &::entity::portal_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::portal_state::ModelBuilder::new(new).with_region(database_name).build();
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
                                let model = ::entity::portal_state::ModelBuilder::new(delete).with_region(database_name).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(model.entity_id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                tracing::debug!(
                    "PortalState -> Processing {} messages in batch",
                    messages.len()
                );
                let insert =
                    insert_many_portal_state(&global_app_state, &on_conflict, &mut messages).await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting PortalState: {}", e);
                }
            }

            if !messages_delete.is_empty() {
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::portal_state::Entity::delete_many()
                        .filter(::entity::portal_state::Column::EntityId.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            PortalState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete PortalState"
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

async fn insert_many_portal_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::portal_state::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::portal_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

#[cfg(test)]
mod write_batch_tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase, QueryTrait, Set, Transaction};

    // Exercise the real helpers: failed writes keep the exact batch for retry,
    // successful retries clear it without releasing its reusable allocation.
    macro_rules! retry_test {
        ($test:ident, $helper:ident, $table:ident, $field:ident, $column:ident, $id:ty) => {
            #[tokio::test]
            async fn $test() {
                use entity::$table::{ActiveModel, Column, Entity};
                let db = MockDatabase::new(DbBackend::Postgres)
                    .append_query_errors([DbErr::Custom("write failed".into())])
                    .append_query_results([vec![std::collections::BTreeMap::from([(
                        stringify!($field).to_owned(),
                        sea_orm::Value::from(2 as $id),
                    )])]])
                    .into_connection();
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                let state = AppState::new(db, tx, Default::default());
                let on_conflict = OnConflict::column(Column::$column)
                    .update_column(Column::$column)
                    .to_owned();
                let mut messages = vec![
                    ActiveModel {
                        $field: Set(1),
                        ..Default::default()
                    },
                    ActiveModel {
                        $field: Set(2),
                        ..Default::default()
                    },
                ];
                let original = messages.clone();
                let allocation = messages.as_ptr();
                let capacity = messages.capacity();
                let mut expected = Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .build(DbBackend::Postgres);
                expected
                    .sql
                    .push_str(concat!(" RETURNING \"", stringify!($field), "\""));

                assert!($helper(&state, &on_conflict, &mut messages).await.is_err());
                assert_eq!(messages, original);
                assert_eq!(messages.as_ptr(), allocation);
                assert_eq!(messages.capacity(), capacity);

                $helper(&state, &on_conflict, &mut messages).await.unwrap();
                assert!(messages.is_empty());
                assert_eq!(messages.as_ptr(), allocation);
                assert_eq!(messages.capacity(), capacity);
                // Empty batches must not issue an INSERT.
                $helper(&state, &on_conflict, &mut messages).await.unwrap();
                assert_eq!(
                    state.conn.into_transaction_log(),
                    [
                        Transaction::one(expected.clone()),
                        Transaction::one(expected)
                    ]
                );
            }
        };
    }

    retry_test!(
        housing_retry,
        insert_many_player_housing_state,
        player_housing_state,
        entity_id,
        EntityId,
        i64
    );
    retry_test!(
        interior_retry,
        insert_many_interior_network_desc,
        interior_network_desc,
        building_id,
        BuildingId,
        i32
    );
    retry_test!(
        dimension_retry,
        insert_many_dimension_description_state,
        dimension_description_state,
        entity_id,
        EntityId,
        i64
    );
    retry_test!(
        permission_retry,
        insert_many_permission_state,
        permission_state,
        entity_id,
        EntityId,
        i64
    );
    retry_test!(
        portal_retry,
        insert_many_portal_state,
        portal_state,
        entity_id,
        EntityId,
        i64
    );
}
