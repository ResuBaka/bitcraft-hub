use crate::AppState;
use crate::websocket::snapshot_write::{self, Writer};
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::building_nickname_state as nickname;
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
                                if !messages.is_empty()
                                    && let Err(error) = insert_multiple_building_state(&global_app_state, &on_conflict, &mut messages).await {
                                    tracing::error!(%error, "Could not flush building updates before initial sync");
                                }
                                if !messages_delete.is_empty()
                                    && let Err(error) = ::entity::building_state::Entity::delete_many()
                                        .filter(::entity::building_state::Column::EntityId.is_in(std::mem::take(&mut messages_delete)))
                                        .exec(&global_app_state.conn).await {
                                    tracing::error!(%error, "Could not flush building deletes before initial sync");
                                }
                                let incoming = data.into_iter().map(|value| ::entity::building_state::ModelBuilder::new(value).with_region(database_name).build());
                                if let Err(error) = crate::websocket::initial_sync::reconcile::<::entity::building_state::Entity, _, _, _, _, _>(
                                    &global_app_state.conn,
                                    ::entity::building_state::Column::Region,
                                    database_name,
                                    Writer::Fixed(snapshot_write::buildings),
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
    ::entity::building_state::Entity::insert_many(std::mem::take(messages))
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await
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
                                use crate::websocket::initial_sync::{flush_before_snapshot, reconcile_cached};
                                let filter = |keys: &[i64]| sea_orm::Condition::all().add(::entity::building_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::building_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush building_desc before snapshot");
                                    continue;
                                }
                                if let Err(error) = reconcile_cached::<::entity::building_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    &global_app_state.building_desc, sea_orm::Condition::all(), |_| true,
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

        // One final operation per PostgreSQL conflict key, in receive order.
        let mut pending = HashMap::new();
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
                                if let Err(error) = flush_building_nicknames(&global_app_state, &on_conflict, &mut pending).await {
                                    tracing::error!(%error, "Could not flush building nicknames before snapshot");
                                    continue;
                                }
                                let mut unique = std::collections::HashSet::new();
                                if let Err(error) = crate::websocket::initial_sync::reconcile_cached::<nickname::Entity, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().rev().filter(|v| unique.insert(v.entity_id))
                                        .map(|v| nickname::ModelBuilder::new(v).with_region(database_name).build()),
                                    |m| m.entity_id, |ids| crate::websocket::initial_sync::bigint_keys_filter(nickname::Column::EntityId, ids),
                                    &global_app_state.building_nickname_state,
                                    sea_orm::Condition::all().add(nickname::Column::Region.eq(database_name)), |m| m.region == database_name,
                                ).await {
                                    tracing::error!(%error, "Could not reconcile building nickname snapshot");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. }
                            | SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = nickname::ModelBuilder::new(new).with_region(database_name).build();
                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                pending.insert(model.entity_id, Some(model));
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let id = delete.entity_id as i64;
                                global_app_state.building_nickname_state.remove(&id);
                                pending.insert(id, None);
                            }
                        }
                        if pending.len() >= batch_size {
                            break;
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

            if let Err(error) = flush_building_nicknames(&global_app_state, &on_conflict, &mut pending).await {
                tracing::error!(%error, "Could not persist building nicknames");
            }

            if rx.is_closed() && rx.is_empty() {
                break;
            }
        }
    });
}

async fn flush_building_nicknames(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    pending: &mut HashMap<i64, Option<nickname::Model>>,
) -> Result<(), DbErr> {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for (id, model) in pending.drain() {
        match model {
            Some(model) => upserts.push(model.into_active_model()),
            None => deletes.push(id),
        }
    }
    crate::websocket::initial_sync::flush_before_snapshot::<nickname::Entity, _>(
        &global_app_state.conn,
        on_conflict,
        &mut upserts,
        &mut deletes,
        |ids| crate::websocket::initial_sync::bigint_keys_filter(nickname::Column::EntityId, ids),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase, Values};

    #[tokio::test]
    async fn nickname_upserts_keep_latest_row_per_entity_before_and_during_snapshot() {
        let row = |name: &str| BuildingNicknameState {
            entity_id: 42,
            nickname: name.into(),
        };
        let stored = nickname::ModelBuilder::new(row("renamed"))
            .with_region(1)
            .build();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![stored.clone()], vec![], vec![stored]])
            .into_connection();
        let (out, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        for (id, region) in [(100, 1), (200, 2)] {
            let model = nickname::ModelBuilder::new(BuildingNicknameState {
                entity_id: id, nickname: "cache only".into(),
            }).with_region(region).build();
            state.building_nickname_state.insert(model.entity_id, model);
        }
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        for msg in [
            SpacetimeUpdateMessages::Insert {
                new: row("first"),
                database_name: 1,
                event: None,
            },
            SpacetimeUpdateMessages::Update {
                old: row("first"),
                new: row("renamed"),
                database_name: 1,
                event: None,
            },
            SpacetimeUpdateMessages::Initial {
                data: vec![row("older snapshot row"), row("latest")],
                database_name: 1,
                batches: None,
            },
        ] {
            tx.send(msg).unwrap();
        }
        start_worker_building_nickname_state(state.clone(), rx, 100, Duration::from_secs(60));
        state.shutdown.drain().await;

        assert_eq!(state.building_nickname_state.get(&42).unwrap().nickname, "latest");
        assert!(!state.building_nickname_state.contains_key(&100));
        assert!(state.building_nickname_state.contains_key(&200));
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 3);
        assert!(log[1].statements()[0].sql.starts_with("SELECT"));
        for (index, name) in [(0, "renamed"), (2, "latest")] {
            let stmt = &log[index].statements()[0];
            assert!(stmt.sql.starts_with("INSERT"));
            assert_eq!(
                stmt.values,
                Some(Values(vec![42i64.into(), name.into(), 1i16.into()]))
            );
        }
    }
}
