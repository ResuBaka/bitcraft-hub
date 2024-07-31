use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use entity::player_state;
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm::{IntoActiveModel, PaginatorTrait};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::fs::File;
use std::path::PathBuf;

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

pub(crate) async fn import_player_state(conn: &DatabaseConnection, storage_path: &PathBuf) -> anyhow::Result<()> {
    let player_state_file =
        File::open(storage_path.join("State/PlayerState.json")).unwrap();

    let player_state: Value = serde_json::from_reader(&player_state_file).unwrap();

    let player_state: Vec<player_state::Model> =
        serde_json::from_value(player_state.get(0).unwrap().get("rows").unwrap().clone()).unwrap();

    let count = player_state.len();

    let db_count = player_state::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let player_state: Vec<player_state::ActiveModel> = player_state
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for player_state in player_state.chunks(2000) {
        let _ = player_state::Entity::insert_many(player_state.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
