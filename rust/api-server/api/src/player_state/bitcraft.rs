use crate::AppState;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages, record_worker_received};
use game_module::module_bindings::{PlayerState, PlayerUsernameState};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_player_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::player_state::Column::EntityId)
            .update_columns([
                ::entity::player_state::Column::TimePlayed,
                ::entity::player_state::Column::SessionStartTimestamp,
                ::entity::player_state::Column::TimeSignedIn,
                ::entity::player_state::Column::SignInTimestamp,
                ::entity::player_state::Column::SignedIn,
                ::entity::player_state::Column::TeleportLocation,
                ::entity::player_state::Column::TravelerTasksExpiration,
                ::entity::player_state::Column::Region,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut ids = vec![];
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("player_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_player_state = ::entity::player_state::Entity::find()
                                    .filter(::entity::player_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(
                                            error = error.to_string(),
                                            "Error while query whole player_state state"
                                        );
                                        vec![]
                                    })
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                let mut online = 0;
                                let mut offline = 0;

                                for mut model in data.into_iter().map(|value| {
                                    let model: ::entity::player_state::Model = ::entity::player_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    if model.signed_in {
                                        online += 1;
                                    } else {
                                        offline += 1;
                                    }

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_player_state.entry(model.entity_id) {
                                        Entry::Occupied(entry) => {
                                            let existing_model = entry.get();
                                            if &model != existing_model {
                                                if model.sign_in_timestamp == 0 {
                                                    model.sign_in_timestamp = existing_model.sign_in_timestamp;
                                                }
                                                global_app_state.player_state.insert(model.entity_id, model.clone());
                                                global_app_state.ranking_system.time_played.update(model.entity_id, model.time_played as i64);
                                                global_app_state.ranking_system.time_signed_in.update(model.entity_id, model.time_signed_in as i64);
                                                local_messages.push(model.into_active_model());
                                            }
                                            entry.remove();
                                        }
                                        Entry::Vacant(_entry) => {
                                            global_app_state.player_state.insert(model.entity_id, model.clone());
                                            global_app_state.ranking_system.time_played.update(model.entity_id, model.time_played as i64);
                                            global_app_state.ranking_system.time_signed_in.update(model.entity_id, model.time_signed_in as i64);
                                            local_messages.push(model.into_active_model());
                                        }
                                    }
                                    if local_messages.len() >= batch_size {
                                       insert_multiple_player_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_player_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                metrics::gauge!("players_current_state", &[
                                    ("online", "false".to_string()),
                                    ("region", database_name.to_string())
                                ]).set(offline);

                                metrics::gauge!("players_current_state", &[
                                    ("online", "true".to_string()),
                                    ("region", database_name.to_string())
                                ]).set(online);

                                for chunk_ids in currently_known_player_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::player_state::Entity::delete_many()
                                        .filter(::entity::player_state::Column::EntityId.is_in(chunk_ids.clone()))
                                        .filter(::entity::player_state::Column::Region.eq(database_name.to_string()))
                                        .exec(&global_app_state.conn)
                                        .await
                                    {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(PlayerState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete PlayerState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let mut model: ::entity::player_state::Model = ::entity::player_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                global_app_state.ranking_system.time_played.update(model.entity_id, model.time_played as i64);
                                global_app_state.ranking_system.time_signed_in.update(model.entity_id, model.time_signed_in as i64);

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).increment(1);

                                if ids.contains(&model.entity_id) {
                                    if let Some(index) = messages.iter().position(|value: &::entity::player_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        if model.sign_in_timestamp == 0 {
                                            model.sign_in_timestamp = messages[index].sign_in_timestamp.clone().unwrap();
                                        }
                                        messages.remove(index);
                                    }
                                }

                                global_app_state.player_state.insert(model.entity_id, model.clone());
                                ids.push(model.entity_id);
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, old, .. } => {
                                let mut model: ::entity::player_state::Model = ::entity::player_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                global_app_state.ranking_system.time_played.update(model.entity_id, model.time_played as i64);
                                let rank = global_app_state.ranking_system.time_played.get_rank(model.time_played as i64);
                                if let Some(rank) = rank {
                                    let _ = global_app_state.tx.send(WebSocketMessages::TimePlayed {
                                        user_id: model.entity_id,
                                        time: model.time_played as u64,
                                        rank: rank as u64,
                                    });
                                }

                                global_app_state.ranking_system.time_signed_in.update(model.entity_id, model.time_signed_in as i64);
                                let rank = global_app_state.ranking_system.time_signed_in.get_rank(model.time_played as i64);
                                if let Some(rank) = rank {
                                    let _ = global_app_state.tx.send(WebSocketMessages::TimeSignedIn {
                                        user_id: model.entity_id,
                                        time: model.time_played as u64,
                                        rank: rank as u64,
                                    });
                                }

                                if model.signed_in != old.signed_in {
                                    metrics::gauge!("players_current_state", &[
                                        ("online", model.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).increment(1);
                                    metrics::gauge!("players_current_state", &[
                                        ("online", old.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).decrement(1);
                                }

                                if ids.contains(&model.entity_id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        if model.sign_in_timestamp == 0 {
                                            model.sign_in_timestamp = messages[index].sign_in_timestamp.clone().unwrap();
                                        }
                                        messages.remove(index);
                                    }
                                }

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }

                                if model.sign_in_timestamp == 0 {
                                    model.sign_in_timestamp = old.sign_in_timestamp;
                                }

                                ids.push(model.entity_id);

                                global_app_state.player_state.insert(model.entity_id, model.clone());
                                let _ = global_app_state.tx.send(WebSocketMessages::PlayerState(model.clone()));
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, reducer_name, .. } => {
                                let model: ::entity::player_state::Model = ::entity::player_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                if let Some(reducer_name) = &reducer_name {
                                    match reducer_name {
                                        &"transfer_player_delayed" => {
                                            continue
                                        }
                                        _ => {}
                                    }
                                }

                                // global_app_state.player_state.remove(&model.entity_id);
                                // global_app_state.ranking_system.time_played.remove(model.entity_id);
                                // global_app_state.ranking_system.time_signed_in.remove(model.entity_id);

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).decrement(1);

                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
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

                insert_multiple_player_state(&global_app_state, &on_conflict, &mut messages).await;
                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("PlayerState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::player_state::Entity::delete_many()
                        .filter(::entity::player_state::Column::EntityId.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            PlayerState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete PlayerState"
                        );
                    }
                }
                messages_delete.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_player_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::player_state::ActiveModel>,
) {
    let insert = ::entity::player_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting PlayerState: {}", err)
    }

    messages.clear();
}

pub(crate) fn start_worker_player_username_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
                .update_columns([
                    ::entity::player_username_state::Column::Username,
                    ::entity::player_username_state::Column::Region,
                ])
                .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut ids = vec![];
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("player_username_state", 1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                let mut currently_known_player_username_state = ::entity::player_username_state::Entity::find()
                                    .filter(::entity::player_username_state::Column::Region.eq(database_name.to_string()))
                                    .all(&global_app_state.conn)
                                    .await
                                    .unwrap_or_else(|error| {
                                        tracing::error!(
                                            error = error.to_string(),
                                            "Error while query whole player_username_state state"
                                        );
                                        vec![]
                                    })
                                    .into_iter()
                                    .map(|value| (value.entity_id, value))
                                    .collect::<HashMap<_, _>>();

                                for model in data.into_iter().map(|value| {
                                    let model: ::entity::player_username_state::Model = ::entity::player_username_state::ModelBuilder::new(value).with_region(database_name.to_string()).build();

                                    model
                                }) {
                                    use std::collections::hash_map::Entry;
                                    match currently_known_player_username_state.entry(model.entity_id) {
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
                                       insert_multiple_player_username_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }
                                };
                                if !local_messages.is_empty() {
                                    insert_multiple_player_username_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                }

                                for chunk_ids in currently_known_player_username_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                    let chunk_ids = chunk_ids.to_vec();
                                    if let Err(error) = ::entity::player_username_state::Entity::delete_many()
                                        .filter(::entity::player_username_state::Column::EntityId.is_in(chunk_ids.clone()))
                                        .filter(::entity::player_username_state::Column::Region.eq(database_name.to_string()))
                                        .exec(&global_app_state.conn)
                                        .await
                                    {
                                        let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                        tracing::error!(PlayerUsernameState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete PlayerUsernameState");
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::player_username_state::Model = ::entity::player_username_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                ids.push(model.entity_id);
                                messages.push(model.into_active_model());

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::player_username_state::Model = ::entity::player_username_state::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                ids.push(model.entity_id);
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, reducer_name,  .. } => {
                                let model: ::entity::player_username_state::Model = ::entity::player_username_state::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                if let Some(reducer_name) = &reducer_name {
                                    match reducer_name {
                                        &"transfer_player_delayed" => {
                                            continue
                                        }
                                        _ => {}
                                    }
                                }

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
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
                insert_multiple_player_username_state(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;

                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("PlayerUsernameState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::player_username_state::Entity::delete_many()
                        .filter(
                            ::entity::player_username_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            PlayerUsernameState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete PlayerUsernameState"
                        );
                    }
                }
                messages_delete.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

async fn insert_multiple_player_username_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::player_username_state::ActiveModel>,
) {
    let insert = ::entity::player_username_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting PlayerUsernameState: {}", err)
    }

    messages.clear();
}
