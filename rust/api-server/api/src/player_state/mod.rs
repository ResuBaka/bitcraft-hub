use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::player_state;
use log::{debug, error, info};
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm::{IntoActiveModel, PaginatorTrait};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

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

    let (posts, num_pages) = QueryCore::find_players(&state.conn, page, posts_per_page, search)
        .await
        .expect("Cannot find posts in page");

    Ok(Json(json!({
        "players": posts,
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

    Ok(Json(json!(player)))
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
            player_state::Column::SerialId,
            player_state::Column::Username,
            player_state::Column::TimePlayed,
            player_state::Column::SessionStartTimestamp,
            player_state::Column::TimeSignedIn,
            player_state::Column::SignInTimestamp,
            player_state::Column::SignedIn,
            player_state::Column::UnmannedVehicleCoords,
            player_state::Column::DestinationMarker,
            player_state::Column::FavoriteCraftingRecipes,
            player_state::Column::TeleportLocation,
            player_state::Column::LightRadius,
            player_state::Column::AccessLevel,
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
