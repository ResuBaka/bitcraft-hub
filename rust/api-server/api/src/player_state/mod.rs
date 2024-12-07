use crate::websocket::Table;
use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::player_username_state::Model;
use entity::{player_state, player_username_state};
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::IntoActiveModel;
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/players", get(list_players))
        .route("/players/:id", get(find_player_by_id))
        .route("/api/bitcraft/players", get(list_players))
        .route("/api/bitcraft/players/:id", get(find_player_by_id))
}

pub async fn list_players(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (player, player_usernames, num_pages) =
        QueryCore::find_players(&state.conn, page, posts_per_page, search)
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
                entity_id: player.entity_id,
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

    Ok(Json(json!({
        "players": merged_player,
        "perPage": posts_per_page,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}

pub async fn find_player_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
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

    let deployables = QueryCore::find_vault_deployable_by_player_with_desc(&state.conn, id as i64)
        .await
        .unwrap_or_else(|error| {
            error!("find_player_by_id -> Error: {:?}", error);
            vec![]
        });

    Ok(Json(json!({
        "entity_id": player.entity_id,
        "time_played": player.time_played,
        "session_start_timestamp": player.session_start_timestamp,
        "time_signed_in": player.time_signed_in,
        "sign_in_timestamp": player.sign_in_timestamp,
        "last_shard_claim": player.last_shard_claim,
        "signed_in": player.signed_in,
        "teleport_location": player.teleport_location,
        "username": player_username,
        "deployables": deployables
    })))
}

#[allow(dead_code)]
pub(crate) async fn load_player_state_from_file(
    storage_path: &PathBuf,
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
        .filter(|player_state| {
            match player_states_from_db_map.get(&player_state.entity_id) {
                Some(player_state_from_db) => {
                    if player_state_from_db != *player_state {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|player_state| player_state.clone().into_active_model())
        .collect::<Vec<player_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
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
    player_state::Entity::delete_many()
        .filter(player_state::Column::EntityId.is_in(known_player_username_state_ids))
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
                    if player_username_state_from_db != *player_username_state {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|player_username_state| player_username_state.clone().into_active_model())
        .collect::<Vec<player_username_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
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
    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<player_username_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let on_conflict = sea_query::OnConflict::column(player_username_state::Column::EntityId)
        .update_columns([player_username_state::Column::Username])
        .to_owned();

    let mut known_player_username_state_ids = get_known_player_username_state_ids(p0).await?;
    for row in p1.inserts.iter() {
        match serde_json::from_str::<player_username_state::Model>(row.text.as_ref()) {
            Ok(player_username_state) => {
                if known_player_username_state_ids.contains(&player_username_state.entity_id) {
                    known_player_username_state_ids.remove(&player_username_state.entity_id);
                }
                buffer_before_insert.push(player_username_state);
                if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                    db_insert_player_username_states(p0, &mut buffer_before_insert, &on_conflict)
                        .await?;
                }
                buffer_before_insert.clear();
            }
            Err(_) => {}
        }
    }

    if buffer_before_insert.len() > 0 {
        db_insert_player_username_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    if known_player_username_state_ids.len() > 0 {
        delete_player_username_state(p0, known_player_username_state_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update_player_username_state(
    p0: &DatabaseConnection,
    tables: &Vec<Table>,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(player_username_state::Column::EntityId)
        .update_columns([player_username_state::Column::Username])
        .to_owned();

    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<player_username_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(1000));

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<player_username_state::Model>(row.text.as_ref()) {
                Ok(player_username_state) => {
                    found_in_inserts.insert(player_username_state.entity_id);
                    buffer_before_insert.push(player_username_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(1000) {
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

    if buffer_before_insert.len() > 0 {
        db_insert_player_username_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    let mut players_username_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<player_username_state::Model>(row.text.as_ref()) {
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

    if players_username_to_delete.len() > 0 {
        delete_player_username_state(p0, players_username_to_delete).await?;
    }

    Ok(())
}

pub(crate) async fn handle_initial_subscription_player_state(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<player_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

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
    for row in p1.inserts.iter() {
        match serde_json::from_str::<player_state::Model>(row.text.as_ref()) {
            Ok(player_state) => {
                if known_player_state_ids.contains(&player_state.entity_id) {
                    known_player_state_ids.remove(&player_state.entity_id);
                }
                buffer_before_insert.push(player_state);
                if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                    info!("PlayerState insert");
                    db_insert_player_states(p0, &mut buffer_before_insert, &on_conflict).await?;
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

    if buffer_before_insert.len() > 0 {
        info!("PlayerState insert");
        db_insert_player_states(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    if known_player_state_ids.len() > 0 {
        delete_player_state(p0, known_player_state_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update_player_state(
    p0: &DatabaseConnection,
    tables: &Vec<Table>,
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

    let chunk_size = Some(5000);
    let mut buffer_before_insert = HashMap::new();

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<player_state::Model>(row.text.as_ref()) {
                Ok(player_state) => {
                    found_in_inserts.insert(player_state.entity_id);
                    buffer_before_insert.insert(player_state.entity_id, player_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(1000) {
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

    if buffer_before_insert.len() > 0 {
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
            match serde_json::from_str::<player_state::Model>(row.text.as_ref()) {
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

    if players_to_delete.len() > 0 {
        delete_player_state(p0, players_to_delete).await?;
    }

    Ok(())
}
