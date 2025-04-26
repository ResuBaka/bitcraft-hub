use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate, WebSocketMessages};
use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::player_username_state::Model;
use entity::{player_state, player_username_state};
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::IntoActiveModel;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, sea_query};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use tokio::sync::mpsc::UnboundedSender;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/players", axum_codec::routing::get(list_players).into())
        .route(
            "/players/{id}",
            axum_codec::routing::get(find_player_by_id).into(),
        )
        .route(
            "/api/bitcraft/players",
            axum_codec::routing::get(list_players).into(),
        )
        .route(
            "/api/bitcraft/players/{id}",
            axum_codec::routing::get(find_player_by_id).into(),
        )
}

#[derive(Deserialize)]
pub struct ListPlayersParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    online: Option<bool>,
}

pub async fn list_players(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<ListPlayersParams>,
) -> Result<axum_codec::Codec<PlayersResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;
    let online = params.online;

    let (player, player_usernames, num_pages) =
        QueryCore::find_players(&state.conn, page, posts_per_page, search, online)
            .await
            .expect("Cannot find player_state in page");

    let merged_player = player
        .into_iter()
        .map(|player| {
            let player_username = player_usernames
                .iter()
                .find(|player_username| player_username.entity_id == player.entity_id)
                .unwrap();

            player_state::PlayerStateMerged {
                entity_id: player.entity_id as u64,
                time_played: player.time_played,
                session_start_timestamp: player.session_start_timestamp,
                time_signed_in: player.time_signed_in,
                sign_in_timestamp: player.sign_in_timestamp,
                last_shard_claim: player.last_shard_claim,
                signed_in: player.signed_in,
                teleport_location: player.teleport_location,
                username: player_username.username.clone(),
            }
        })
        .collect::<Vec<player_state::PlayerStateMerged>>();

    Ok(axum_codec::Codec(PlayersResponse {
        players: merged_player,
        per_page: posts_per_page,
        total: num_pages.number_of_items,
        page,
    }))
}

#[derive(Serialize, Deserialize)]
pub struct PlayersResponse {
    pub players: Vec<player_state::PlayerStateMerged>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

pub async fn find_player_by_id(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<Value>, (StatusCode, &'static str)> {
    let player = player_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .expect("Cannot find player");

    if player.is_none() {
        return Err((StatusCode::NOT_FOUND, "Player not found"));
    }

    let player = player.unwrap();

    let player_username = player_username_state::Entity::find()
        .filter(player_username_state::Column::EntityId.eq(id))
        .one(&state.conn)
        .await
        .expect("Cannot find player_username");

    let player_username = match player_username {
        Some(player_username) => player_username.username,
        None => "".to_string(),
    };

    let deployables = QueryCore::find_vault_deployable_by_player_with_desc(&state.conn, id)
        .await
        .unwrap_or_else(|error| {
            error!("find_player_by_id -> Error: {:?}", error);
            vec![]
        });

    let player_location = state
        .mobile_entity_state
        .get(&(id as u64))
        .map(|player_location| player_location.clone());

    let chunk_index = player_location
        .as_ref()
        .map(|player_location| player_location.chunk_index);

    let claim_id = if chunk_index.is_some() {
        if let Some(claim_id) = state.claim_tile_state.get(&(chunk_index.unwrap())) {
            Some(claim_id.claim_id)
        } else {
            None
        }
    } else {
        None
    };

    let plyer_action_state2 = state
        .player_action_state
        .get(&(id as u64))
        .map(|player_action_state| player_action_state.value().clone());
    let player_action_state = state
        .player_action_state
        .get(&(id as u64))
        .map(|player_action_state| player_action_state.action_type.get_action_name());
    let current_action_state = state.action_state.get(&(id as u64));

    let claim_ids = state
        .claim_description_state
        .iter()
        .filter_map(|claim_description_state| {
            if claim_description_state
                .members
                .iter()
                .any(|member| member.player_entity_id == player.entity_id)
            {
                Some((
                    claim_description_state.entity_id as u64,
                    claim_description_state.name.clone(),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<(u64, String)>>();

    Ok(axum_codec::Codec(json!({
        "entity_id": player.entity_id,
        "time_played": player.time_played,
        "session_start_timestamp": player.session_start_timestamp,
        "time_signed_in": player.time_signed_in,
        "sign_in_timestamp": player.sign_in_timestamp,
        "last_shard_claim": player.last_shard_claim,
        "signed_in": player.signed_in,
        "teleport_location": player.teleport_location,
        "username": player_username,
        "deployables": deployables,
        "player_location": player_location,
        "claim_id": claim_id,
        "claim_ids": claim_ids,
        "player_action_state": player_action_state,
        "player_action_state2": plyer_action_state2,
        "current_action_state": current_action_state,
    })))
}

#[allow(dead_code)]
pub(crate) async fn load_player_state_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<player_state::Model>> {
    let player_state_file = File::open(storage_path.join("State/PlayerState.json"))?;
    let player_state: Value = serde_json::from_reader(&player_state_file)?;
    let player_state: Vec<player_state::Model> =
        serde_json::from_value(player_state.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(player_state)
}

async fn get_known_player_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_player_state_ids: Vec<i64> = player_state::Entity::find()
        .select_only()
        .column(player_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_player_state_ids = known_player_state_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_player_state_ids)
}

async fn db_insert_player_states(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<player_state::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let player_states_from_db = player_state::Entity::find()
        .filter(
            player_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|player_state| player_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let player_states_from_db_map = player_states_from_db
        .into_iter()
        .map(|player_state| (player_state.entity_id, player_state))
        .collect::<HashMap<i64, player_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(
            |player_state| match player_states_from_db_map.get(&player_state.entity_id) {
                Some(player_state_from_db) => player_state_from_db != *player_state,
                None => true,
            },
        )
        .map(|player_state| player_state.clone().into_active_model())
        .collect::<Vec<player_state::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} player_states", things_to_insert.len());
    }

    let _ = player_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_player_state(
    conn: &DatabaseConnection,
    known_player_state_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "player_state's ({}) to delete: {:?}",
        known_player_state_ids.len(),
        known_player_state_ids
    );
    player_state::Entity::delete_many()
        .filter(player_state::Column::EntityId.is_in(known_player_state_ids))
        .exec(conn)
        .await?;
    Ok(())
}

pub(crate) async fn get_known_player_username_state_ids(
    conn: &DatabaseConnection,
) -> anyhow::Result<HashSet<i64>> {
    let known_player_username_state_ids: Vec<i64> = player_username_state::Entity::find()
        .select_only()
        .column(player_username_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_player_username_state_ids = known_player_username_state_ids
        .into_iter()
        .collect::<HashSet<i64>>();

    Ok(known_player_username_state_ids)
}

async fn delete_player_username_state(
    conn: &DatabaseConnection,
    known_player_username_state_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "player_username_state's ({}) to delete: {:?}",
        known_player_username_state_ids.len(),
        known_player_username_state_ids
    );
    player_username_state::Entity::delete_many()
        .filter(player_username_state::Column::EntityId.is_in(known_player_username_state_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_player_username_states(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let player_username_states_from_db = player_username_state::Entity::find()
        .filter(
            player_username_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|player_username_state| player_username_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let player_username_states_from_db_map = player_username_states_from_db
        .into_iter()
        .map(|player_username_state| (player_username_state.entity_id, player_username_state))
        .collect::<HashMap<i64, player_username_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|player_username_state| {
            match player_username_states_from_db_map.get(&player_username_state.entity_id) {
                Some(player_username_state_from_db) => {
                    player_username_state_from_db != *player_username_state
                }
                None => true,
            }
        })
        .map(|player_username_state| player_username_state.clone().into_active_model())
        .collect::<Vec<player_username_state::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!(
            "Inserting {} player_username_states",
            things_to_insert.len()
        );
    }

    let _ = player_username_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();

    Ok(())
}

pub(crate) async fn handle_initial_subscription_player_username_state(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<player_username_state::Model> =
        Vec::with_capacity(chunk_size);

    let on_conflict = sea_query::OnConflict::column(player_username_state::Column::EntityId)
        .update_columns([player_username_state::Column::Username])
        .to_owned();

    let mut known_player_username_state_ids = get_known_player_username_state_ids(p0).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<player_username_state::Model>(row.as_ref()) {
                Ok(player_username_state) => {
                    if known_player_username_state_ids.contains(&player_username_state.entity_id) {
                        known_player_username_state_ids.remove(&player_username_state.entity_id);
                    }
                    buffer_before_insert.push(player_username_state);
                    if buffer_before_insert.len() == chunk_size {
                        db_insert_player_username_states(
                            p0,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert PlayerUsernameState Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        db_insert_player_username_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    if !known_player_username_state_ids.is_empty() {
        delete_player_username_state(p0, known_player_username_state_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update_player_username_state(
    p0: &DatabaseConnection,
    tables: &[TableWithOriginalEventTransactionUpdate],
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(player_username_state::Column::EntityId)
        .update_columns([player_username_state::Column::Username])
        .to_owned();

    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<player_username_state::Model> =
        Vec::with_capacity(chunk_size);

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<player_username_state::Model>(row.as_ref()) {
                Ok(player_username_state) => {
                    found_in_inserts.insert(player_username_state.entity_id);
                    buffer_before_insert.push(player_username_state);
                    if buffer_before_insert.len() == chunk_size {
                        db_insert_player_username_states(
                            p0,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert PlayerUsernameState Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        db_insert_player_username_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    let mut players_username_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<player_username_state::Model>(row.as_ref()) {
                Ok(player_username_state) => {
                    if !found_in_inserts.contains(&player_username_state.entity_id) {
                        players_username_to_delete.insert(player_username_state.entity_id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete PlayerUsernameState Error: {error}");
                }
            }
        }
    }

    if !players_username_to_delete.is_empty() {
        delete_player_username_state(p0, players_username_to_delete).await?;
    }

    Ok(())
}

pub(crate) async fn handle_initial_subscription_player_state(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<player_state::Model> = Vec::with_capacity(chunk_size);

    let on_conflict = sea_query::OnConflict::column(player_state::Column::EntityId)
        .update_columns([
            player_state::Column::TimePlayed,
            player_state::Column::SessionStartTimestamp,
            player_state::Column::TimeSignedIn,
            player_state::Column::SignInTimestamp,
            player_state::Column::SignedIn,
            player_state::Column::TeleportLocation,
            player_state::Column::LastShardClaim,
        ])
        .to_owned();

    let mut known_player_state_ids = get_known_player_state_ids(p0).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<player_state::Model>(row.as_ref()) {
                Ok(player_state) => {
                    if known_player_state_ids.contains(&player_state.entity_id) {
                        known_player_state_ids.remove(&player_state.entity_id);
                    }
                    buffer_before_insert.push(player_state);
                    if buffer_before_insert.len() == chunk_size {
                        info!("PlayerState insert");
                        db_insert_player_states(p0, &mut buffer_before_insert, &on_conflict)
                            .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert PlayerState Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        info!("PlayerState insert");
        db_insert_player_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    if !known_player_state_ids.is_empty() {
        delete_player_state(p0, known_player_state_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update_player_state(
    p0: &DatabaseConnection,
    tables: &[TableWithOriginalEventTransactionUpdate],
    sender: UnboundedSender<WebSocketMessages>,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(player_state::Column::EntityId)
        .update_columns([
            player_state::Column::TimePlayed,
            player_state::Column::SessionStartTimestamp,
            player_state::Column::TimeSignedIn,
            player_state::Column::SignInTimestamp,
            player_state::Column::SignedIn,
            player_state::Column::TeleportLocation,
            player_state::Column::LastShardClaim,
        ])
        .to_owned();

    let chunk_size = 5000;
    let mut buffer_before_insert = HashMap::new();

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<player_state::Model>(row.as_ref()) {
                Ok(player_state) => {
                    found_in_inserts.insert(player_state.entity_id);
                    sender
                        .send(WebSocketMessages::PlayerState(player_state.clone()))
                        .unwrap();
                    buffer_before_insert.insert(player_state.entity_id, player_state);

                    if buffer_before_insert.len() == chunk_size {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<player_state::Model>>();

                        db_insert_player_states(p0, &mut buffer_before_insert_vec, &on_conflict)
                            .await?;
                        buffer_before_insert.clear();
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert PlayerState Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<player_state::Model>>();

        db_insert_player_states(p0, &mut buffer_before_insert_vec, &on_conflict).await?;
        buffer_before_insert.clear();
    }

    let mut players_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<player_state::Model>(row.as_ref()) {
                Ok(player_state) => {
                    if !found_in_inserts.contains(&player_state.entity_id) {
                        players_to_delete.insert(player_state.entity_id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete PlayerState Error: {error}");
                }
            }
        }
    }

    if !players_to_delete.is_empty() {
        delete_player_state(p0, players_to_delete).await?;
    }

    Ok(())
}
