use crate::{create_default_client, create_importer_default_db_connection, AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::{player_state, player_username_state};
use log::{debug, error, info};
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm::{IntoActiveModel, PaginatorTrait};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use reqwest::Client;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;
use crate::config::Config;

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

    let (player, player_usernames, num_pages) = QueryCore::find_players(&state.conn, page, posts_per_page, search)
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
                last_shared_claim: player.last_shared_claim,
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
    
    Ok(Json(json!(player_state::PlayerStateMerged {
        entity_id: player.entity_id,
        time_played: player.time_played,
        session_start_timestamp: player.session_start_timestamp,
        time_signed_in: player.time_signed_in,
        sign_in_timestamp: player.sign_in_timestamp,
        last_shared_claim: player.last_shared_claim,
        signed_in: player.signed_in,
        teleport_location: player.teleport_location,
        username: player_username,
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

pub(crate) async fn load_player_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM PlayerState")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".to_string());
        }
    };

    Ok(json)
}

pub(crate) async fn load_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_player_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_player_states(&conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}

pub(crate) async fn import_player_states(
    conn: &DatabaseConnection,
    player_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<player_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(player_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(player_state::Column::EntityId)
        .update_columns([
            player_state::Column::TimePlayed,
            player_state::Column::SessionStartTimestamp,
            player_state::Column::TimeSignedIn,
            player_state::Column::SignInTimestamp,
            player_state::Column::SignedIn,
            player_state::Column::TeleportLocation,
            player_state::Column::LastSharedClaim,
        ])
        .to_owned();

    let mut player_states_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<player_state::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
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

            if player_states_from_db.len() != buffer_before_insert.len() {
                player_states_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|player_state| {
                            !player_states_from_db.iter().any(|player_state_from_db| {
                                player_state_from_db.entity_id == player_state.entity_id
                            })
                        })
                        .map(|player_state| player_state.entity_id),
                );
            }

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
                continue;
            } else {
                debug!("Inserting {} player_states", things_to_insert.len());
            }

            for player_state in &things_to_insert {
                let player_state_in = player_states_to_delete
                    .iter()
                    .position(|id| id == player_state.entity_id.as_ref());
                if player_state_in.is_some() {
                    player_states_to_delete.remove(player_state_in.unwrap());
                }
            }

            let _ = player_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
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
        } else {
            debug!("Inserting {} player_states", things_to_insert.len());
            player_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("player_state last batch imported");
    }
    info!(
        "Importing player_state finished in {}s",
        start.elapsed().as_secs()
    );

    if player_states_to_delete.len() > 0 {
        info!("player_state's to delete: {:?}", player_states_to_delete);
        player_state::Entity::delete_many()
            .filter(player_state::Column::EntityId.is_in(player_states_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

fn import_player_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = load_state_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                    .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("PlayerState imported");
                } else {
                    error!("PlayerState import failed: {:?}", vehicle_state);
                }
            });
    });
}

pub async fn import_job_player_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = create_importer_default_db_connection(config.clone()).await;
            let client = create_default_client(config.clone());

            let now = std::time::Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_player_state(config.clone(), conn, client);

            let now = std::time::Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = create_importer_default_db_connection(config.clone()).await;
        let client = create_default_client(config.clone());

        import_player_state(config.clone(), conn, client);
    }
}

pub(crate) async fn load_player_username_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM PlayerUsernameState")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".to_string());
        }
    };

    Ok(json)
}

pub(crate) async fn load_player_username_state(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_player_username_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_player_username_states(&conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}


pub(crate) async fn import_player_username_states(
    conn: &DatabaseConnection,
    player_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<player_username_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(player_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(player_username_state::Column::EntityId)
        .update_columns([
            player_username_state::Column::Username,
        ])
        .to_owned();

    let mut player_username_states_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<player_username_state::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
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

            if player_username_states_from_db.len() != buffer_before_insert.len() {
                player_username_states_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|player_username_state| {
                            !player_username_states_from_db.iter().any(|player_username_state_from_db| {
                                player_username_state_from_db.entity_id == player_username_state.entity_id
                            })
                        })
                        .map(|player_username_state| player_username_state.entity_id),
                );
            }

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
                continue;
            } else {
                debug!("Inserting {} player_username_states", things_to_insert.len());
            }

            for player_username_state in &things_to_insert {
                let player_username_state_in = player_username_states_to_delete
                    .iter()
                    .position(|id| id == player_username_state.entity_id.as_ref());
                if player_username_state_in.is_some() {
                    player_username_states_to_delete.remove(player_username_state_in.unwrap());
                }
            }

            let _ = player_username_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let player_states_from_db = player_username_state::Entity::find()
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

        let player_states_from_db_map = player_states_from_db
            .into_iter()
            .map(|player_username_state| (player_username_state.entity_id, player_username_state))
            .collect::<HashMap<i64, player_username_state::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|player_state| {
                match player_states_from_db_map.get(&player_state.entity_id) {
                    Some(player_username_state_from_db) => {
                        if player_username_state_from_db != *player_state {
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
        } else {
            debug!("Inserting {} player_username_state", things_to_insert.len());
            player_username_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("player_username_state last batch imported");
    }
    info!(
        "Importing player_username_state finished in {}s",
        start.elapsed().as_secs()
    );

    if player_username_states_to_delete.len() > 0 {
        info!("player_username_state's to delete: {:?}", player_username_states_to_delete);
        player_state::Entity::delete_many()
            .filter(player_state::Column::EntityId.is_in(player_username_states_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub fn import_player_username_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let player_username_state = load_player_username_state(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                    // &config,
                )
                    .await;

                if let Ok(_) = player_username_state {
                    info!("PlayerUsernameState imported");
                } else {
                    error!("PlayerUsernameState import failed: {:?}", player_username_state);
                }
            });
    });
}

pub async fn import_job_player_username_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = create_importer_default_db_connection(config.clone()).await;
            let client = create_default_client(config.clone());

            let now = std::time::Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_player_username_state(config.clone(), conn, client);

            let now = std::time::Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = create_importer_default_db_connection(config.clone()).await;
        let client = create_default_client(config.clone());

        import_player_username_state(config.clone(), conn, client);
    }
}