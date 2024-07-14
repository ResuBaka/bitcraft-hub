use axum::{extract::{Form, Path, Query, State}, http::StatusCode, response::Html, routing::{get, get_service, post}, Router, Json};
use service::{
    sea_orm::{Database, DatabaseConnection},
    Mutation as MutationCore, Query as QueryCore,
};
use entity::{player_state, location, item, cargo_description};
use migration::{Migrator, MigratorTrait};
use serde::{Deserialize, Serialize};
use std::env;
use sea_orm::{ActiveModelTrait, DeriveColumn, EntityTrait, EnumIter, IntoActiveModel, PaginatorTrait, QuerySelect};
use std::fs::File;
use serde_json::{json, Value};
use tower_cookies::{CookieManagerLayer, Cookies};
use tower_http::services::ServeDir;
use entity::prelude::PlayerState;

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");
    let host = env::var("HOST").expect("HOST is not set in .env file");
    let port = env::var("PORT").expect("PORT is not set in .env file");
    let server_url = format!("{host}:{port}");

    let conn = Database::connect(db_url)
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();

    let connImport = conn.clone();
    tokio::spawn(async move {
        import_player_state(&connImport).await.unwrap();
        import_items(&connImport).await.unwrap();
        import_cargo_description(&connImport).await.unwrap();
    });

    let state = AppState { conn };

    let app = Router::new()
        .route("/", get(list_posts))
        .route("/l", get(list_locations))
        .route("/items", get(list_items))
        .nest_service(
            "/static",
            get_service(ServeDir::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/static"
            )))
                .handle_error(|error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {error}"),
                    )
                }),
        )
        .layer(CookieManagerLayer::new())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&server_url).await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    conn: DatabaseConnection,
}

#[derive(Deserialize)]
struct Params {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
}

async fn list_posts(
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
        "perPage": num_pages.number_of_pages,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}

async fn list_items(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (items, tags, tiers) = tokio::join!(
        QueryCore::find_items(&state.conn, page, posts_per_page, search),
         QueryCore::find_unique_item_tags(&state.conn),
         QueryCore::find_unique_item_tiers(&state.conn),
    );

    let (items, num_pages) = items.expect("Cannot find items");
    let tags = tags.expect("Cannot find tags");
    let tiers = tiers.expect("Cannot find tiers");

    Ok(Json(json!({
        "items": items,
        "tiers": tiers,
        "tags": tags,
        "perPage": num_pages.number_of_pages,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}


async fn list_locations(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);

    let (posts, num_pages) = QueryCore::find_locations(&state.conn, page, posts_per_page)
        .await
        .expect("Cannot find posts in page");

    Ok(Json(json!({
        "posts": posts,
        "num_pages": num_pages.number_of_pages,
        "num_items": num_pages.number_of_items,
    })))
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum Counter {
    Count
}

async fn import_player_state(
    conn: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
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

async fn import_items(
    conn: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/Desc/ItemDesc.json").unwrap();
    let item: Value = serde_json::from_reader(&item_file).unwrap();
    let item: Vec<item::Model> = serde_json::from_value(item.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = item.len();
    let db_count = item::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    for item in item {
        let _ = item.into_active_model().insert(conn).await;
    }

    Ok(())
}

async fn import_cargo_description(
    conn: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/Desc/CargoDesc.json").unwrap();
    let cargo_description: Value = serde_json::from_reader(&item_file).unwrap();
    let cargo_descriptions: Vec<cargo_description::Model> = serde_json::from_value(cargo_description.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = cargo_descriptions.len();
    let db_count = item::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    for cargo_description in cargo_descriptions {
        let _ = cargo_description.into_active_model().insert(conn).await;
    }

    Ok(())
}

pub fn main() {
    let result = start();

    if let Some(err) = result.err() {
        println!("Error: {err}");
    }
}