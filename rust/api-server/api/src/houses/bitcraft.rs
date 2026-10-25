use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::{
    DimensionDescriptionState, InteriorNetworkDesc, PermissionState, PlayerHousingState,
    PortalState,
};
use migration::{OnConflict, sea_query};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    Iterable, QueryFilter, TransactionTrait,
};
use std::time::Duration;

// Advisory budget for ActiveModel + SQL expression/bind storage, not a hard
// process-memory limit. A single oversized row must still be written intact.
const WRITE_BATCH_ESTIMATED_BYTES: usize = 256 * 1024;
const ESTIMATED_SQL_BYTES_PER_COLUMN: usize = 128;

fn write_batch_len<T>(rows: &[T], budget: usize, estimate: impl Fn(&T) -> usize) -> usize {
    let mut bytes = 0usize;
    let mut count = 0;
    for row in rows {
        let next = bytes.saturating_add(estimate(row).max(1));
        if count != 0 && next > budget {
            break;
        }
        bytes = next;
        count += 1;
    }
    count
}

async fn write_housing_chunk<E: EntityTrait>(
    conn: &impl ConnectionTrait,
    on_conflict: &OnConflict,
    rows: &[E::ActiveModel],
) -> Result<u64, DbErr> {
    E::insert_many(rows.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec_without_returning(conn)
        .await
}

async fn write_housing_batches<E: EntityTrait>(
    conn: &DatabaseConnection,
    on_conflict: &OnConflict,
    rows: &[E::ActiveModel],
    extra_payload_bytes: impl Fn(&E::ActiveModel) -> usize + Send + Sync,
    budget: usize,
) -> Result<(), DbErr> {
    if rows.is_empty() {
        return Ok(());
    }
    let fixed_bytes = std::mem::size_of::<E::ActiveModel>()
        .saturating_add(E::Column::iter().count() * ESTIMATED_SQL_BYTES_PER_COLUMN);
    let estimate = |row: &E::ActiveModel| fixed_bytes.saturating_add(extra_payload_bytes(row));
    let first_len = write_batch_len(rows, budget, &estimate);
    if first_len == rows.len() {
        if write_housing_chunk::<E>(conn, on_conflict, rows).await? == 0 {
            // exec() previously reported this as RecordNotInserted. Do not
            // accidentally clear a retry batch when removing RETURNING.
            return Err(DbErr::RecordNotInserted);
        }
    } else {
        // Keep the original helper's all-or-nothing write behavior when a batch
        // is split. Failed/cancelled transactions leave the caller's rows intact.
        let transaction = conn.begin().await?;
        let mut remaining = rows;
        let mut affected = 0u64;
        while !remaining.is_empty() {
            let len = write_batch_len(remaining, budget, &estimate);
            affected = affected.saturating_add(
                write_housing_chunk::<E>(&transaction, on_conflict, &remaining[..len]).await?,
            );
            remaining = &remaining[len..];
        }
        if affected == 0 {
            return Err(DbErr::RecordNotInserted);
        }
        transaction.commit().await?;
    }
    Ok(())
}

fn interior_payload_bytes(model: &::entity::interior_network_desc::ActiveModel) -> usize {
    use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
    match &model.child_interior_instances {
        // JSON i32s require at most 11 digits/sign characters plus a comma.
        Set(value) | Unchanged(value) => value.0.len().saturating_mul(12).saturating_add(2),
        NotSet => 0,
    }
}

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
        let receive_metrics = WorkerReceiveMetrics::new("interior_network_desc");
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
                        receive_metrics.record(1);
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

    write_housing_batches::<::entity::interior_network_desc::Entity>(
        &global_app_state.conn,
        on_conflict,
        messages,
        interior_payload_bytes,
        WRITE_BATCH_ESTIMATED_BYTES,
    )
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
        let receive_metrics = WorkerReceiveMetrics::new("dimension_description_state");
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
                        receive_metrics.record(1);
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

    write_housing_batches::<::entity::dimension_description_state::Entity>(
        &global_app_state.conn,
        on_conflict,
        messages,
        |_| 0,
        WRITE_BATCH_ESTIMATED_BYTES,
    )
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
        let receive_metrics = WorkerReceiveMetrics::new("player_housing_state");
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
                        receive_metrics.record(1);
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

    write_housing_batches::<::entity::player_housing_state::Entity>(
        &global_app_state.conn,
        on_conflict,
        messages,
        |_| 0,
        WRITE_BATCH_ESTIMATED_BYTES,
    )
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
        let receive_metrics = WorkerReceiveMetrics::new("permission_state");
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
                        receive_metrics.record(1);
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

    write_housing_batches::<::entity::permission_state::Entity>(
        &global_app_state.conn,
        on_conflict,
        messages,
        |_| 0,
        WRITE_BATCH_ESTIMATED_BYTES,
    )
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
        let receive_metrics = WorkerReceiveMetrics::new("portal_state");
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
                        receive_metrics.record(1);
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

    write_housing_batches::<::entity::portal_state::Entity>(
        &global_app_state.conn,
        on_conflict,
        messages,
        |_| 0,
        WRITE_BATCH_ESTIMATED_BYTES,
    )
    .await?;

    messages.clear();
    Ok(())
}

#[cfg(test)]
mod write_batch_tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase, MockExecResult, QueryTrait, Set, Transaction};

    // Exercise the real helpers: failed writes keep the exact batch for retry,
    // successful retries clear it without releasing its reusable allocation.
    macro_rules! retry_test {
        ($test:ident, $helper:ident, $table:ident, $field:ident, $column:ident, $id:ty) => {
            #[tokio::test]
            async fn $test() {
                use entity::$table::{ActiveModel, Column, Entity};
                let db = MockDatabase::new(DbBackend::Postgres)
                    .append_exec_errors([DbErr::Custom("write failed".into())])
                    .append_exec_results([MockExecResult {
                        last_insert_id: 0,
                        rows_affected: 2,
                    }])
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
                let expected = Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .build(DbBackend::Postgres);
                assert!(!expected.sql.contains("RETURNING"));

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

    #[test]
    fn byte_budget_handles_variable_rows_oversized_rows_and_zero_budget() {
        assert_eq!(write_batch_len(&[4, 6, 2], 10, |bytes| *bytes), 2);
        assert_eq!(write_batch_len(&[11, 1], 10, |bytes| *bytes), 1);
        assert_eq!(write_batch_len(&[0, 0], 0, |bytes| *bytes), 1);
        assert_eq!(write_batch_len::<usize>(&[], 10, |bytes| *bytes), 0);
        assert_eq!(write_batch_len(&[usize::MAX, 1], 10, |bytes| *bytes), 1);
    }

    #[tokio::test]
    async fn split_writes_commit_together_without_returning_rows() {
        use entity::player_housing_state::{ActiveModel, Column, Entity};
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results((0..3).map(|_| MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }))
            .into_connection();
        let rows = (1..=3)
            .map(|id| ActiveModel {
                entity_id: Set(id),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let conflict = OnConflict::column(Column::EntityId)
            .update_column(Column::Rank)
            .to_owned();
        write_housing_batches::<Entity>(&db, &conflict, &rows, |_| 0, 1)
            .await
            .unwrap();
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("BEGIN") && log.contains("COMMIT"));
        assert_eq!(log.matches("INSERT INTO").count(), 3);
        assert!(!log.contains("RETURNING"));
        assert_eq!(rows.len(), 3, "the caller clears only after success");
    }

    #[tokio::test]
    async fn split_write_failure_rolls_back_and_preserves_all_original_rows() {
        use entity::player_housing_state::{ActiveModel, Column, Entity};
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .append_exec_errors([DbErr::Custom("second chunk failed".into())])
            .into_connection();
        let rows = (1..=3)
            .map(|id| ActiveModel {
                entity_id: Set(id),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let original = rows.clone();
        let allocation = rows.as_ptr();
        let conflict = OnConflict::column(Column::EntityId)
            .update_column(Column::Rank)
            .to_owned();
        assert!(
            write_housing_batches::<Entity>(&db, &conflict, &rows, |_| 0, 1)
                .await
                .is_err()
        );
        assert_eq!(rows, original);
        assert_eq!(rows.as_ptr(), allocation);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("ROLLBACK"));
        assert!(!log.contains("COMMIT"));
        assert_eq!(
            log.matches("INSERT INTO").count(),
            2,
            "stop after the failed chunk"
        );
    }

    #[tokio::test]
    async fn zero_affected_rows_still_retain_the_retry_batch() {
        use entity::player_housing_state::{ActiveModel, Column};
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 0,
            }])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let mut rows = vec![ActiveModel {
            entity_id: Set(1),
            ..Default::default()
        }];
        let original = rows.clone();
        let conflict = OnConflict::column(Column::EntityId)
            .update_column(Column::Rank)
            .to_owned();
        assert_eq!(
            insert_many_player_housing_state(&state, &conflict, &mut rows)
                .await
                .unwrap_err(),
            DbErr::RecordNotInserted
        );
        assert_eq!(rows, original);
    }

    #[test]
    fn interior_json_payloads_contribute_to_the_budget_without_serialization() {
        use entity::interior_network_desc::{ActiveModel, ChildInteriorInstances};
        let mut row = ActiveModel::default();
        assert_eq!(interior_payload_bytes(&row), 0);
        row.child_interior_instances = Set(ChildInteriorInstances(vec![i32::MIN; 100]));
        let estimate = interior_payload_bytes(&row);
        assert!(
            estimate
                >= serde_json::to_vec(&ChildInteriorInstances(vec![i32::MIN; 100]))
                    .unwrap()
                    .len()
        );
    }
}
