mod claims;
mod items;
mod player_state;
mod cargo_desc;
mod locations;

use axum::{http::StatusCode, Router, routing::{get, get_service}};
use service::sea_orm::{Database, DatabaseConnection};

use migration::{Migrator, MigratorTrait};
use serde::{Deserialize, Serialize};
use std::env;
use sea_orm::{ActiveModelTrait, DeriveColumn, EntityTrait, EnumIter, IntoActiveModel, PaginatorTrait, QuerySelect};
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
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
        let (player_state, items, cargo_description, claim_description) = tokio::join!(
            player_state::import_player_state(&connImport),
            items::import_items(&connImport),
            cargo_desc::import_cargo_description(&connImport),
            claims::import_claim_description_state(&connImport),
        );

        if let Ok(player_state) = player_state {
            println!("PlayerState imported");
        } else {
            println!("PlayerState import failed: {:?}", player_state);
        }

        if let Ok(items) = items {
            println!("Items imported");
        } else {
            println!("Items import failed: {:?}", items);
        }

        if let Ok(cargo_description) = cargo_description {
            println!("CargoDescription imported");
        } else {
            println!("CargoDescription import failed: {:?}", cargo_description);
        }

        if let Ok(claim_description) = claim_description {
            println!("ClaimDescription imported");
        } else {
            println!("ClaimDescription import failed: {:?}", claim_description);
        }
    });

    let state = AppState { conn };

    let app = Router::new()
        .route("/players", get(player_state::list_players))
        .route("/locations", get(locations::list_locations))
        .route("/items", get(items::list_items))
        .route("/claims", get(claims::list_claims))
        .route("/claims/:id", get(claims::find_claim_descriptions))
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

pub fn main() {
    let result = start();

    if let Some(err) = result.err() {
        println!("Error: {err}");
    }
}