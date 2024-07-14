use axum::extract::{Query, State};
use tower_cookies::Cookies;
use axum::Json;
use serde_json::{json, Value};
use axum::http::StatusCode;
use sea_orm::{DatabaseConnection, EntityTrait,  QuerySelect};
use sea_orm::{ActiveModelTrait, DeriveColumn, EnumIter, IntoActiveModel, PaginatorTrait};
use service::Query as QueryCore;
use std::fs::File;
use crate::{AppState, Params};
use entity::{player_state};

pub async fn list_players(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
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

pub(crate) async fn import_player_state(
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut player_state_file = File::open("/home/resubaka/code/crafting-list/storage/State/PlayerState.json").unwrap();



    let player_state: Value = serde_json::from_reader(&player_state_file).unwrap();

    let player_state: Vec<player_state::Model> = serde_json::from_value(player_state.get(0).unwrap().get("rows").unwrap().clone()).unwrap();

    let count = player_state.len();

    let db_count = player_state::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    for player_state in player_state {
        let _ = player_state.into_active_model().insert(conn).await;
    }

    Ok(())
}