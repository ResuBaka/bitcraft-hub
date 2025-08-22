use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use entity::{claim_local_state, claim_member_state, claim_state, claim_tech_state};
use futures::FutureExt;
use game_module::module_bindings::{
    ClaimLocalState, ClaimMemberState, ClaimState, ClaimTechDesc, ClaimTechState,
};
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, TryIntoModel};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_claim_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimState>>,
    batch_size: usize,
    time_limit: Duration,
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
                                let mut currently_known_claim_state = ::entity::claim_state::Entity::find()
                                    .filter(::entity::claim_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    global_app_state.claim_state.insert(model.entity_id, model.clone());
                                    use std::collections::hash_map::Entry;
                                    match currently_known_claim_state.entry(model.entity_id) {
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
                                       insert_multiple_claim_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_claim_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_claim_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::claim_state::Entity::delete_many().filter(::entity::claim_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(ClaimState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ClaimState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                global_app_state.claim_state.insert(model.entity_id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.claim_state.insert(model.entity_id, model.clone());
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.claim_state.remove(&id);
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
                insert_multiple_claim_state(&global_app_state, &on_conflict, &mut messages).await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_claim_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_state::ActiveModel>,
) {
    let insert = ::entity::claim_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting ClaimState: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_claim_local_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimLocalState>>,
    batch_size: usize,
    time_limit: Duration,
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

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                let mut buffer = vec![];
                let fill_buffer_with = batch_size
                    .saturating_sub(buffer.len())
                    .saturating_sub(messages.len());

                tokio::select! {
                    _count = rx.recv_many(&mut buffer, fill_buffer_with) => {
                        for msg in buffer {
                            match msg {
                                SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                    let mut local_messages = vec![];
                                    let mut currently_known_claim_local_state = ::entity::claim_local_state::Entity::find()
                                        .filter(::entity::claim_local_state::Column::Region.eq(database_name.to_string()))
                                        .all(&global_app_state.conn)
                                        .await
                                        .map_or(vec![], |aa| aa)
                                        .into_iter()
                                        .map(|value| (value.entity_id, value))
                                        .collect::<HashMap<_, _>>();

                                    for model in data.into_iter().map(|value| {
                                        let model: ::entity::claim_local_state::Model = ::entity::claim_local_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                        model
                                    }) {
                                        let org_id = model.entity_id;
                                        global_app_state.claim_local_state.insert(org_id as u64, model.clone());
                                        let _ = global_app_state.tx
                                            .send(WebSocketMessages::ClaimLocalState(
                                                model.clone(),
                                            ));

                                        use std::collections::hash_map::Entry;
                                        match currently_known_claim_local_state.entry(model.entity_id) {
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
                                           insert_multiple_claim_local_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                        }
                                    };
                                    if !local_messages.is_empty() {
                                        insert_multiple_claim_local_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }

                                    for chunk_ids in currently_known_claim_local_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                        let chunk_ids = chunk_ids.to_vec();
                                        if let Err(error) = ::entity::claim_local_state::Entity::delete_many().filter(::entity::claim_local_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                            tracing::error!(ClaimLocalState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ClaimLocalState");
                                        }
                                    }
                                }
                                SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                    let org_id = new.entity_id;
                                    let model: ::entity::claim_local_state::Model = ::entity::claim_local_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                    global_app_state.claim_local_state.insert(org_id, model.clone());

                                    if let Some(index) = messages.iter().position(|value: &::entity::claim_local_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.clone().into_active_model());

                                    let _ = global_app_state.tx
                                        .send(WebSocketMessages::ClaimLocalState(
                                            model,
                                        ));

                                    if messages.len() >= batch_size {
                                        insert_multiple_claim_local_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                    let org_id = new.entity_id;
                                    let model: ::entity::claim_local_state::Model = ::entity::claim_local_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                    global_app_state.claim_local_state.insert(org_id, model.clone());

                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    messages.push(model.clone().into_active_model());

                                    let _ = global_app_state.tx
                                        .send(WebSocketMessages::ClaimLocalState(
                                            model,
                                        ));

                                    if messages.len() >= batch_size {
                                        insert_multiple_claim_local_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                    let model: ::entity::claim_local_state::Model = ::entity::claim_local_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
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
                insert_multiple_claim_local_state(&global_app_state, &on_conflict, &mut messages)
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

async fn insert_multiple_claim_local_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_local_state::ActiveModel>,
) {
    let insert = ::entity::claim_local_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting ClaimLocalState: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_claim_member_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimMemberState>>,
    batch_size: usize,
    time_limit: Duration,
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
                claim_member_state::Column::Region,
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
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = vec![];
                                let mut currently_known_claim_member_state = ::entity::claim_member_state::Entity::find()
                                    .filter(::entity::claim_member_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::claim_member_state::Model = ::entity::claim_member_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_claim_member_state.entry(model.entity_id) {
                                        Entry::Occupied(entry) => {
                                            let existing_model = entry.get();
                                            if &model != existing_model {
                                                local_messages.push(model.into_active_model());
                                            }
                                            global_app_state
                                                    .add_claim_member(existing_model.clone());
                                            entry.remove();
                                        }
                                        Entry::Vacant(_entry) => {
                                            local_messages.push(model.into_active_model());
                                        }
                                    }
                                    if local_messages.len() >= batch_size {
                                       insert_multiple_claim_member_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_claim_member_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_claim_member_state.into_iter().map(|(id, value)| {
                                    global_app_state.remove_claim_member(value.clone());

                                    id
                                }).collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::claim_member_state::Entity::delete_many().filter(::entity::claim_member_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(ClaimMemberState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ClaimMemberState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::claim_member_state::Model = ::entity::claim_member_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::claim_member_state::Model = ::entity::claim_member_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::claim_member_state::Model = ::entity::claim_member_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
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
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!("Processing {} messages in batch", messages.len());
                insert_multiple_claim_member_state(&global_app_state, &on_conflict, &mut messages)
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

async fn insert_multiple_claim_member_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_member_state::ActiveModel>,
) {
    let insert = ::entity::claim_member_state::Entity::insert_many(
        messages
            .iter()
            .map(|value| {
                global_app_state.add_claim_member(value.clone().try_into_model().unwrap());
                value.clone()
            })
            .collect::<Vec<_>>(),
    )
    .on_conflict(on_conflict.clone())
    .exec(&global_app_state.conn)
    .await;

    if insert.is_err() {
        tracing::error!("Error inserting ClaimMemberState: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_claim_tech_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechState>>,
    batch_size: usize,
    time_limit: Duration,
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
                                let mut currently_known_claim_tech_state = ::entity::claim_tech_state::Entity::find()
                                    .filter(::entity::claim_tech_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::claim_tech_state::Model = ::entity::claim_tech_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_claim_tech_state.entry(model.entity_id) {
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
                                       insert_multiple_claim_tech_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_claim_tech_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_claim_tech_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::claim_tech_state::Entity::delete_many().filter(::entity::claim_tech_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(ClaimTechState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ClaimTechState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::claim_tech_state::Model = ::entity::claim_tech_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::claim_tech_state::Model = ::entity::claim_tech_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::claim_tech_state::Model = ::entity::claim_tech_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
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

                insert_multiple_claim_tech_state(&global_app_state, &on_conflict, &mut messages)
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

async fn insert_multiple_claim_tech_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_tech_state::ActiveModel>,
) {
    let insert = ::entity::claim_tech_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting ClaimTechState: {}", insert.unwrap_err())
    }

    messages.clear();
}

pub(crate) fn start_worker_claim_tech_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechDesc>>,
    batch_size: usize,
    time_limit: Duration,
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
                                let mut currently_known_claim_tech_desc = ::entity::claim_tech_desc::Entity::find()
                                    .all(&global_app_state.conn)
                                    .await
                                    .map_or(vec![], |aa| aa)
                                    .into_iter()
                                    .map(|value| (value.id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::claim_tech_desc::Model = value.into();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_claim_tech_desc.entry(model.id) {
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
                                       insert_multiple_claim_tech_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_claim_tech_desc(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_claim_tech_desc.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::claim_tech_desc::Entity::delete_many().filter(::entity::claim_tech_desc::Column::Id.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(ClaimTechDesc = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ClaimTechDesc");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();

                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();
                                messages.push(model.into_active_model());
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

                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_claim_tech_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_tech_desc::ActiveModel>,
) {
    let insert = ::entity::claim_tech_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting ClaimTechDesc: {}", insert.unwrap_err())
    }

    messages.clear();
}
