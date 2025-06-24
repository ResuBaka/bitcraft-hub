use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use entity::{claim_local_state, claim_member_state, claim_state, claim_tech_state};
use futures::FutureExt;
use game_module::module_bindings::{
    ClaimLocalState, ClaimMemberState, ClaimState, ClaimTechDesc, ClaimTechState,
};
use kanal::AsyncReceiver;
use migration::sea_query;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, TryIntoModel};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_claim_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_state::Column::EntityId)
            .update_columns([
                claim_state::Column::OwnerPlayerEntityId,
                claim_state::Column::OwnerBuildingEntityId,
                claim_state::Column::Name,
                claim_state::Column::Neutral,
            ])
            .to_owned();

        let mut currently_known_claim_state = ::entity::claim_state::Entity::find()
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

                                let model: ::entity::claim_state::Model = new.into();

                                if currently_known_claim_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_state::Model = new.into();
                                if currently_known_claim_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimState = id, error = error.to_string(), "Could not delete ClaimState");
                                }

                                tracing::debug!("ClaimState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_state::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
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

pub(crate) fn start_worker_claim_local_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimLocalState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_local_state::Column::EntityId)
            .update_columns([
                claim_local_state::Column::Supplies,
                claim_local_state::Column::BuildingMaintenance,
                claim_local_state::Column::NumTiles,
                claim_local_state::Column::NumTileNeighbors,
                claim_local_state::Column::Location,
                claim_local_state::Column::Treasury,
                claim_local_state::Column::XpGainedSinceLastCoinMinting,
                claim_local_state::Column::SuppliesPurchaseThreshold,
                claim_local_state::Column::SuppliesPurchasePrice,
                claim_local_state::Column::BuildingDescriptionId,
            ])
            .to_owned();

        let mut currently_known_claim_local_state = ::entity::claim_local_state::Entity::find()
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
                                let org_id = new.entity_id;
                                let model: ::entity::claim_local_state::Model = new.into();
                                global_app_state.claim_local_state.insert(org_id, model.clone());

                                if currently_known_claim_local_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_local_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.clone().into_active_model());
                                    } else {
                                        currently_known_claim_local_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.clone().into_active_model());
                                }

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let org_id = new.entity_id;
                                let model: ::entity::claim_local_state::Model = new.into();

                                global_app_state.claim_local_state.insert(org_id, model.clone());

                                if currently_known_claim_local_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_local_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.clone().into_active_model());
                                    } else {
                                        currently_known_claim_local_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.clone().into_active_model());
                                }

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_local_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                global_app_state.claim_local_state.remove(&(model.entity_id as u64));

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimLocalState = id, error = error.to_string(), "Could not delete ClaimLocalState");
                                }

                                tracing::debug!("ClaimLocalState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_local_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
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

pub(crate) fn start_worker_claim_member_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimMemberState>>,
    batch_size: usize,
    time_limit: Duration,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_member_state::Column::EntityId)
            .update_columns([
                claim_member_state::Column::ClaimEntityId,
                claim_member_state::Column::PlayerEntityId,
                claim_member_state::Column::UserName,
                claim_member_state::Column::InventoryPermission,
                claim_member_state::Column::BuildPermission,
                claim_member_state::Column::OfficerPermission,
                claim_member_state::Column::CoOwnerPermission,
            ])
            .to_owned();

        let mut currently_known_claim_member_state = ::entity::claim_member_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        let cleanup_signal_future = cancel_token.cancelled().fuse();
        tokio::pin!(cleanup_signal_future);

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_member_state::Model = new.into();

                                global_app_state
                                    .add_claim_member(model.clone());

                                if currently_known_claim_member_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_member_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_member_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_member_state::Model = new.into();

                                global_app_state
                                    .add_claim_member(model.clone());

                                if currently_known_claim_member_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_member_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_member_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_member_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }


                                global_app_state.remove_claim_member(model.clone());

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimMemberState = id, error = error.to_string(), "Could not delete ClaimMemberState");
                                }

                                tracing::debug!("ClaimMemberState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    _ = &mut cleanup_signal_future => {
                        if global_app_state.connection_state.iter().filter(|a| a.eq(&true)).collect::<Vec<_>>().len() != global_app_state.connection_state.len() {
                            tracing::warn!("Cleanup did not run as not all servers have an active connection");
                            break;
                        }

                        let claim_member_state_to_delete = currently_known_claim_member_state.values().map(|ckps| {
                            global_app_state.remove_claim_member(ckps.clone());

                            ckps.entity_id
                        }).collect::<Vec<_>>();

                        tracing::info!("claim_member_state to delete {} {:?}", claim_member_state_to_delete.len(), claim_member_state_to_delete);

                        let result = ::entity::claim_member_state::Entity::delete_many()
                            .filter(::entity::claim_member_state::Column::EntityId.is_in(claim_member_state_to_delete))
                            .exec(&global_app_state.conn).await;

                        if let Err(error) = result {
                            tracing::error!("Error while cleanup of player_state {error}");
                        }

                        currently_known_claim_member_state.clear();

                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_member_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| {
                            global_app_state
                                .add_claim_member(value.clone().try_into_model().unwrap());
                            value.clone().into_active_model()
                        })
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
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

pub(crate) fn start_worker_claim_tech_state(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimTechState>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([claim_tech_state::Column::EntityId])
            .update_columns([
                claim_tech_state::Column::Learned,
                claim_tech_state::Column::Researching,
                claim_tech_state::Column::StartTimestamp,
                claim_tech_state::Column::ScheduledId,
            ])
            .to_owned();

        let mut currently_known_claim_tech_state = ::entity::claim_tech_state::Entity::find()
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
                                let model: ::entity::claim_tech_state::Model = new.into();

                                if currently_known_claim_tech_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_tech_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_state::Model = new.into();
                                if currently_known_claim_tech_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_tech_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechState = id, error = error.to_string(), "Could not delete ClaimTechState");
                                }

                                tracing::debug!("ClaimTechState::Remove");
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
                    "ClaimTechState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_state::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechState: {}", insert.unwrap_err())
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

pub(crate) fn start_worker_claim_tech_desc(
    global_app_state: AppState,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimTechDesc>>,
    batch_size: usize,
    time_limit: Duration,
    _cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([::entity::claim_tech_desc::Column::Id])
            .update_columns([
                ::entity::claim_tech_desc::Column::Description,
                ::entity::claim_tech_desc::Column::Tier,
                ::entity::claim_tech_desc::Column::SuppliesCost,
                ::entity::claim_tech_desc::Column::ResearchTime,
                ::entity::claim_tech_desc::Column::Requirements,
                ::entity::claim_tech_desc::Column::Input,
                ::entity::claim_tech_desc::Column::Members,
                ::entity::claim_tech_desc::Column::Area,
                ::entity::claim_tech_desc::Column::Supplies,
                ::entity::claim_tech_desc::Column::XpToMintHexCoin,
            ])
            .to_owned();

        let mut currently_known_claim_tech_desc = ::entity::claim_tech_desc::Entity::find()
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
                                let model: ::entity::claim_tech_desc::Model = new.into();

                                if currently_known_claim_tech_desc.contains_key(&model.id) {
                                    let value = currently_known_claim_tech_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();
                                if currently_known_claim_tech_desc.contains_key(&model.id) {
                                    let value = currently_known_claim_tech_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechDesc = id, error = error.to_string(), "Could not delete ClaimTechDesc");
                                }

                                tracing::debug!("ClaimTechDesc::Remove");
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
                    "ClaimTechDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechDesc: {}", insert.unwrap_err())
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
