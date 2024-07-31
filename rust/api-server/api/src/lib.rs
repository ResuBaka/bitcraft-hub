mod buildings;
mod cargo_desc;
mod claim_tech_state;
mod claims;
mod inventory;
mod items;
mod itemsAndCargo;
mod leaderboard;
mod locations;
mod player_state;
mod recipes;
mod skill_descriptions;
mod vehicle_state;

use tower_http::cors::{Any, CorsLayer};

use axum::{
    http::StatusCode,
    middleware,
    routing::{get, get_service},
    Router,
};
use service::sea_orm::{Database, DatabaseConnection};

use axum::extract::{MatchedPath, Request};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::IntoResponse;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use migration::{Migrator, MigratorTrait};
use sea_orm::ConnectOptions;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};
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
    let path_to_storage = env::var("STORAGE_PATH").expect("PATH_TO_STORAGE is not set in .env file");
    let allowed_origin = env::var("ALLOWED_ORIGIN").unwrap_or("http://localhost:3000".to_string());
    let server_url = format!("{host}:{port}");

    let mut connection_options = ConnectOptions::new(db_url);
    connection_options
        .max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .sqlx_logging(env::var("SQLX_LOG").is_ok());

    let conn = Database::connect(connection_options.clone())
        .await
        .expect("Database connection failed");
    Migrator::up(&conn, None).await.unwrap();

    let connection_options = connection_options.clone();

    let path_to_storage_tmp = path_to_storage.clone();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut path_to_storage_path_buf = PathBuf::from(path_to_storage_tmp);
                println!("Starting importing data");
                let conn = Database::connect(connection_options)
                    .await
                    .expect("Database connection failed");

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_player_state = conn_import.clone();
                    let conn_import_import_items = conn_import.clone();

                    let (player_state, items) = tokio::join!(
                        player_state::import_player_state(&conn_import_import_player_state, &path_to_storage_path_buf),
                        items::import_items(&conn_import_import_items, &path_to_storage_path_buf),
                    );

                    if let Ok(_player_state) = player_state {
                        println!("PlayerState imported");
                    } else {
                        println!("PlayerState import failed: {:?}", player_state);
                    }

                    if let Ok(_items) = items {
                        println!("Items imported");
                    } else {
                        println!("Items import failed: {:?}", items);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_cargo_description = conn_import.clone();
                    let conn_import_import_claim_description = conn_import.clone();

                    let (cargo_description, claim_description) = tokio::join!(
                        cargo_desc::import_cargo_description(&conn_import_import_cargo_description, &path_to_storage_path_buf),
                        claims::import_claim_description_state(
                            &conn_import_import_claim_description,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_cargo_description) = cargo_description {
                        println!("CargoDescription imported");
                    } else {
                        println!("CargoDescription import failed: {:?}", cargo_description);
                    }

                    if let Ok(_claim_description) = claim_description {
                        println!("ClaimDescription imported");
                    } else {
                        println!("ClaimDescription import failed: {:?}", claim_description);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_leaderboard = conn_import.clone();
                    let conn_import_import_skill_descriptions = conn_import.clone();

                    let (leaderboard, skill_descriptions) = tokio::join!(
                        leaderboard::import_experience_state(&conn_import_import_leaderboard, &path_to_storage_path_buf),
                        skill_descriptions::import_skill_descriptions(
                            &conn_import_import_skill_descriptions,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_leaderboard) = leaderboard {
                        println!("Leaderboard imported");
                    } else {
                        println!("Leaderboard import failed: {:?}", leaderboard);
                    }
                    if let Ok(_skill_descriptions) = skill_descriptions {
                        println!("SkillDescriptions imported");
                    } else {
                        println!("SkillDescriptions import failed: {:?}", skill_descriptions);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_recipes = conn_import.clone();
                    let conn_import_import_building_state = conn_import.clone();

                    let (recipes, building_state) = tokio::join!(
                        recipes::import_recipes(&conn_import_import_recipes, &path_to_storage_path_buf),
                        buildings::import_building_state(&conn_import_import_building_state, &path_to_storage_path_buf),
                    );

                    if let Ok(_recipes) = recipes {
                        println!("Recipes imported");
                    } else {
                        println!("Recipes import failed: {:?}", recipes);
                    }

                    if let Ok(_building_state) = building_state {
                        println!("BuildingState imported");
                    } else {
                        println!("BuildingState import failed: {:?}", building_state);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_building_desc = conn_import.clone();

                    let (building_desc,) = tokio::join!(buildings::import_building_desc(
                        &conn_import_import_building_desc,
                        &path_to_storage_path_buf
                    ),);

                    if let Ok(_building_desc) = building_desc {
                        println!("BuildingDesc imported");
                    } else {
                        println!("BuildingDesc import failed: {:?}", building_desc);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_inventory = conn_import.clone();

                    let (claim_description_state, claim_description_desc) = tokio::join!(
                        claim_tech_state::import_claim_description_state(
                            &conn_import_import_inventory,
                            &path_to_storage_path_buf
                        ),
                        claim_tech_state::import_claim_description_desc(
                            &conn_import_import_inventory,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_claim_description_state) = claim_description_state {
                        println!("ClaimDescriptionState imported");
                    } else {
                        println!(
                            "ClaimDescriptionState import failed: {:?}",
                            claim_description_state
                        );
                    }

                    if let Ok(_claim_description_desc) = claim_description_desc {
                        println!("ClaimDescriptionDesc imported");
                    } else {
                        println!(
                            "ClaimDescriptionDesc import failed: {:?}",
                            claim_description_desc
                        );
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_inventory = conn_import.clone();

                    let vehicle_state =
                        vehicle_state::import_vehicle_state(&conn_import_import_inventory, &path_to_storage_path_buf).await;

                    if let Ok(_vehicle_state) = vehicle_state {
                        println!("VehicleState imported");
                    } else {
                        println!("VehicleState import failed: {:?}", vehicle_state);
                    }
                })
                .await;

                let path_to_storage_path_buf = path_to_storage_path_buf.clone();
                tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_buf.clone();
                    let inventory = inventory::import_inventory(&conn, &path_to_storage_path_buf).await;

                    if let Ok(_inventory) = inventory {
                        println!("Inventory imported");
                    } else {
                        println!("Inventory import failed: {:?}", inventory);
                    }
                })
                .await;

                println!("Importing data finished");
            });
    });

    let state = AppState { conn, storage_path: PathBuf::from(path_to_storage) };

    let desc_router = Router::new()
        .route(
            "/buildings/:id",
            axum_codec::routing::get(buildings::find_claim_description).into(),
        )
        .route(
            "/buildings",
            axum_codec::routing::get(buildings::find_building_descriptions).into(),
        );

    let app = Router::new()
        .route(
            "/api/bitcraft/desc/buildings",
            axum_codec::routing::get(buildings::find_building_descriptions).into(),
        )
        .route("/players", get(player_state::list_players))
        .route("/players/:id", get(player_state::find_player_by_id))
        .route("/api/bitcraft/players", get(player_state::list_players))
        .route(
            "/api/bitcraft/players/:id",
            get(player_state::find_player_by_id),
        )
        .route("/locations", get(locations::list_locations))
        .route("/items", get(items::list_items))
        .route("/claims", get(claims::list_claims))
        .route("/api/bitcraft/claims", get(claims::list_claims))
        .route("/api/bitcraft/claims/:id", get(claims::get_claim))
        .route("/claims/:id", get(claims::find_claim_descriptions))
        .route(
            "/buildings",
            axum_codec::routing::get(buildings::find_building_states).into(),
        )
        .route(
            "/api/bitcraft/buildings",
            axum_codec::routing::get(buildings::find_building_states).into(),
        )
        .route(
            "/buildings/:id",
            axum_codec::routing::get(buildings::find_building_state).into(),
        )
        .route(
            "/api/bitcraft/buildings/:id",
            axum_codec::routing::get(buildings::find_building_state).into(),
        )
        .route(
            "/inventorys/changes/:id",
            axum_codec::routing::get(inventory::read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/changes/:id",
            axum_codec::routing::get(inventory::read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/owner_entity_id/:id",
            axum_codec::routing::get(inventory::find_inventory_by_owner_entity_id).into(),
        )
        .route(
            "/inventory/:id",
            axum_codec::routing::get(inventory::find_inventory_by_id).into(),
        )
        .route(
            "/recipes/needed_in_crafting/:id",
            axum_codec::routing::get(recipes::get_needed_in_crafting).into(),
        )
        .route(
            "/recipes/produced_in_crafting/:id",
            axum_codec::routing::get(recipes::get_produced_in_crafting).into(),
        )
        .route(
            "/recipes/needed_to_craft/:id",
            axum_codec::routing::get(recipes::get_needed_to_craft).into(),
        )
        .route(
            "/api/bitcraft/recipes/needed_in_crafting/:id",
            axum_codec::routing::get(recipes::get_needed_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/produced_in_crafting/:id",
            axum_codec::routing::get(recipes::get_produced_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/needed_to_craft/:id",
            axum_codec::routing::get(recipes::get_needed_to_craft).into(),
        )
        .route(
            "/api/bitcraft/itemsAndCargo",
            axum_codec::routing::get(itemsAndCargo::list_items_and_cargo).into(),
        )
        .nest("/desc", desc_router)
        .route("/leaderboard", get(leaderboard::get_top_100))
        .route(
            "/experience/:player_id",
            get(leaderboard::player_leaderboard),
        )
        .route(
            "/api/bitcraft/experience/:player_id",
            get(leaderboard::player_leaderboard),
        )
        .route(
            "/api/bitcraft/leaderboard/claims/:claim_id",
            get(leaderboard::get_claim_leaderboard),
        )
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
        .layer(
            CorsLayer::new()
                .allow_origin(allowed_origin.parse::<HeaderValue>().unwrap())
                .allow_methods(Any),
        )
        .route_layer(middleware::from_fn(track_metrics))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&server_url).await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    conn: DatabaseConnection,
    storage_path: PathBuf,
}

#[derive(Deserialize)]
struct Params {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
}

fn setup_metrics_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install_recorder()
        .unwrap()
}

async fn track_metrics(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}

pub fn main() {
    let result = start();

    if let Some(err) = result.err() {
        println!("Error: {err}");
    }
}
