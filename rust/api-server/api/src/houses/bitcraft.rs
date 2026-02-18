use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::{
    DimensionDescriptionState, InteriorNetworkDesc, PermissionState, PlayerHousingState,
    PortalState,
};
use migration::{OnConflict, sea_query};
use sea_orm::{
    ColumnTrait, DbErr, EntityTrait, InsertResult, IntoActiveModel, ModelTrait, QueryFilter,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

// ─────────────────────────────────────────────────────────────────────────────
// Interior Network Desc Worker
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn start_worker_interior_network_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<InteriorNetworkDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::interior_network_desc::Column::BuildingId])
                .update_columns([
                    ::entity::interior_network_desc::Column::DimensionType,
                    ::entity::interior_network_desc::Column::ChildInteriorInstances,
                    ::entity::interior_network_desc::Column::Region,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model = ::entity::interior_network_desc::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::interior_network_desc::ActiveModel| value.building_id.as_ref() == &model.building_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value: &::entity::interior_network_desc::ActiveModel| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::interior_network_desc::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.building_id.as_ref() == &model.building_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete InteriorNetworkDesc");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
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

            if messages.is_empty() && rx.is_closed() {
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

    for chunk in messages.chunks(1000) {
        ::entity::interior_network_desc::Entity::insert_many(chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(&global_app_state.conn)
            .await?;
    }

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
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model = ::entity::dimension_description_state::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::dimension_description_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                    break;
                                }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::dimension_description_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value: &::entity::dimension_description_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::dimension_description_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::dimension_description_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete DimensionDescriptionState");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
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

            if messages.is_empty() && rx.is_closed() {
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

    for chunk in messages.chunks(1000) {
        ::entity::dimension_description_state::Entity::insert_many(chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(&global_app_state.conn)
            .await?;
    }

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
    tokio::spawn(async move {
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
                    ::entity::player_housing_state::Column::Region,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model = ::entity::player_housing_state::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::player_housing_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value: &::entity::player_housing_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                 if messages.len() >= batch_size {
                                        break;
                                    }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::player_housing_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete PlayerHousingState");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
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

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_many_player_housing_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::player_housing_state::ActiveModel>,
) -> Result<(), DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    for chunk in messages.chunks(1000) {
        ::entity::player_housing_state::Entity::insert_many(chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(&global_app_state.conn)
            .await?;
    }

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
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model = ::entity::permission_state::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::permission_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::permission_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value: &::entity::permission_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::permission_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::permission_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete PermissionState");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
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

            if messages.is_empty() && rx.is_closed() {
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

    for chunk in messages.chunks(1000) {
        ::entity::permission_state::Entity::insert_many(chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(&global_app_state.conn)
            .await?;
    }

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
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model = ::entity::portal_state::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::portal_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.into_active_model());
                                    if messages.len() >= batch_size {
                                        break;
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::portal_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value: &::entity::portal_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::portal_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::portal_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete PortalState");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
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

            if messages.is_empty() && rx.is_closed() {
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

    for chunk in messages.chunks(1000) {
        ::entity::portal_state::Entity::insert_many(chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(&global_app_state.conn)
            .await?;
    }

    messages.clear();
    Ok(())
}
