mod buildings;
mod cargo_desc;
mod claim_tech_state;
mod claims;
mod config;
mod inventory;
mod items;
mod items_and_cargo;
mod leaderboard;
mod locations;
mod player_state;
mod recipes;
mod skill_descriptions;
mod vehicle_state;

use axum::{
    http::StatusCode,
    middleware,
    routing::{get, get_service},
    Router,
};
use reqwest_websocket::{Message, RequestBuilderExt};
use service::sea_orm::{Database, DatabaseConnection};
use tower_http::cors::{Any, CorsLayer};

use axum::extract::{MatchedPath, Request};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::IntoResponse;
use base64::Engine;
use futures::{SinkExt, TryStreamExt};
use log::{error, info};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use migration::{Migrator, MigratorTrait};
use reqwest::header::{HeaderMap, SEC_WEBSOCKET_PROTOCOL};
use reqwest::Client;
use sea_orm::ConnectOptions;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    let config = config::Config::new();

    let mut connection_options = ConnectOptions::new(config.database.url.clone());
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

    let path_to_storage_tmp = config.storage_path.clone();
    import_data(connection_options, path_to_storage_tmp);

    let state = AppState {
        conn,
        storage_path: PathBuf::from(config.storage_path.clone()),
    };

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
        .route("/locations", get(locations::list_locations))
        .route("/items", get(items::list_items))
        .merge(player_state::get_routes())
        .merge(claims::get_routes())
        .merge(buildings::get_routes())
        .merge(inventory::get_routes())
        .merge(recipes::get_routes())
        .merge(items_and_cargo::get_routes())
        .merge(leaderboard::get_routes())
        .nest("/desc", desc_router)
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
                .allow_origin(
                    config
                        .origins
                        .origin
                        .iter()
                        .map(|origin| origin.parse::<HeaderValue>().unwrap())
                        .collect::<Vec<HeaderValue>>(),
                )
                .allow_methods(Any),
        )
        .route_layer(middleware::from_fn(track_metrics))
        .with_state(state);

    let websocket_url = config.weboosocket_url();
    let websocket_password = config.spacetimedb.password.clone();
    let websocket_username = config.spacetimedb.username.clone();
    let database_name = config.spacetimedb.database.clone();

    if config.live_updates {
        tokio::spawn(async move {
            let mut headers = HeaderMap::new();
            headers.insert(
                "Authorization",
                format!(
                    "Basic {}",
                    base64::prelude::BASE64_STANDARD
                        .encode(format!("{}:{}", websocket_username, websocket_password))
                )
                .parse()
                .unwrap(),
            );
            headers.insert(
                SEC_WEBSOCKET_PROTOCOL,
                "v1.text.spacetimedb".parse().unwrap(),
            );
            headers.insert(
                "Sec-WebSocket-Key",
                "dGhlIHNhbXBsZSBub25jZQ==".parse().unwrap(),
            );

            let response = Client::default()
                .get(format!(
                    "{}/{}/{}",
                    websocket_url, "database/subscribe", database_name
                ))
                .headers(headers)
                .upgrade()
                .protocols(vec!["v1.text.spacetimedb"])
                .send()
                .await
                .unwrap();
            let mut websocket = response.into_websocket().await.unwrap();

            websocket
                .send(Message::Text(
                    serde_json::json!({
                        "subscribe": {
                            "query_strings": ["SELECT * FROM ExperienceState"],
                        },
                    })
                    .to_string(),
                ))
                .await
                .unwrap();

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            let _ = tokio::spawn(async move {
                let mut evenets = Vec::with_capacity(1000);

                loop {
                    let count = rx.recv_many(&mut evenets, 1000).await;
                    evenets.clear();
                    info!("Received {count} events");
                }
            });

            while let Some(message) = websocket.try_next().await.unwrap() {
                if let Message::Text(text) = message {
                    let message: Result<WebSocketMessage, serde_json::Error> =
                        serde_json::from_str(&text);

                    if message.is_err() {
                        info!("Text: {:?}", text);
                        info!("Error: {:?}", message.err());
                        continue;
                    }

                    let message = message.unwrap();

                    match &message {
                        WebSocketMessage::TransactionUpdate(transaction_update) => {
                            tx.send(message.clone()).unwrap();
                            info!("Received transaction update: {transaction_update:?}");
                        }
                        WebSocketMessage::SubscriptionUpdate(subscription_update) => {
                            tx.send(message.clone()).unwrap();
                            info!("Received subscription update: {subscription_update:?}");
                        }
                        WebSocketMessage::IdentityToken(identity_token) => {
                            info!("Received identity token: {identity_token:?}");
                        }
                    }
                }
            }
        });
    }

    let server_url = config.server_url();
    let listener = tokio::net::TcpListener::bind(&server_url).await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

fn import_data(connection_options: ConnectOptions, path_to_storage_tmp: String) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut importer = Vec::new();
                let path_to_storage_path_buf = PathBuf::from(path_to_storage_tmp);
                info!("Starting importing data");
                let conn = Database::connect(connection_options)
                    .await
                    .expect("Database connection failed");

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                let _ = tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_player_state = conn_import.clone();
                    let conn_import_import_items = conn_import.clone();

                    let (player_state, items) = tokio::join!(
                        player_state::import_player_state(
                            &conn_import_import_player_state,
                            &path_to_storage_path_buf
                        ),
                        items::import_items(&conn_import_import_items, &path_to_storage_path_buf),
                    );

                    if let Ok(_player_state) = player_state {
                        info!("PlayerState imported");
                    } else {
                        error!("PlayerState import failed: {:?}", player_state);
                    }

                    if let Ok(_items) = items {
                        info!("Items imported");
                    } else {
                        error!("Items import failed: {:?}", items);
                    }
                })
                .await;

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_cargo_description = conn_import.clone();
                    let conn_import_import_claim_description = conn_import.clone();

                    let (cargo_description, claim_description) = tokio::join!(
                        cargo_desc::import_cargo_description(
                            &conn_import_import_cargo_description,
                            &path_to_storage_path_buf
                        ),
                        claims::import_claim_description_state(
                            &conn_import_import_claim_description,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_cargo_description) = cargo_description {
                        info!("CargoDescription imported");
                    } else {
                        error!("CargoDescription import failed: {:?}", cargo_description);
                    }

                    if let Ok(_claim_description) = claim_description {
                        info!("ClaimDescription imported");
                    } else {
                        error!("ClaimDescription import failed: {:?}", claim_description);
                    }
                }));

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_leaderboard = conn_import.clone();
                    let conn_import_import_skill_descriptions = conn_import.clone();

                    let (leaderboard, skill_descriptions) = tokio::join!(
                        leaderboard::import_experience_state(
                            &conn_import_import_leaderboard,
                            &path_to_storage_path_buf
                        ),
                        skill_descriptions::import_skill_descriptions(
                            &conn_import_import_skill_descriptions,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_leaderboard) = leaderboard {
                        info!("Leaderboard imported");
                    } else {
                        error!("Leaderboard import failed: {:?}", leaderboard);
                    }
                    if let Ok(_skill_descriptions) = skill_descriptions {
                        info!("SkillDescriptions imported");
                    } else {
                        error!("SkillDescriptions import failed: {:?}", skill_descriptions);
                    }
                }));

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_recipes = conn_import.clone();
                    let conn_import_import_building_state = conn_import.clone();

                    let (recipes, building_state) = tokio::join!(
                        recipes::import_recipes(
                            &conn_import_import_recipes,
                            &path_to_storage_path_buf
                        ),
                        buildings::import_building_state(
                            &conn_import_import_building_state,
                            &path_to_storage_path_buf
                        ),
                    );

                    if let Ok(_recipes) = recipes {
                        info!("Recipes imported");
                    } else {
                        error!("Recipes import failed: {:?}", recipes);
                    }

                    if let Ok(_building_state) = building_state {
                        info!("BuildingState imported");
                    } else {
                        error!("BuildingState import failed: {:?}", building_state);
                    }
                }));

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_building_desc = conn_import.clone();

                    let building_descs =
                        buildings::load_building_desc_from_file(&path_to_storage_path_buf)
                            .await
                            .unwrap();

                    let (building_desc,) = tokio::join!(buildings::import_building_desc(
                        &conn_import_import_building_desc,
                        &building_descs,
                        None
                    ),);

                    if let Ok(_building_desc) = building_desc {
                        info!("BuildingDesc imported");
                    } else {
                        error!("BuildingDesc import failed: {:?}", building_desc);
                    };
                }));

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
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
                        info!("ClaimDescriptionState imported");
                    } else {
                        error!(
                            "ClaimDescriptionState import failed: {:?}",
                            claim_description_state
                        );
                    }

                    if let Ok(_claim_description_desc) = claim_description_desc {
                        info!("ClaimDescriptionDesc imported");
                    } else {
                        error!(
                            "ClaimDescriptionDesc import failed: {:?}",
                            claim_description_desc
                        );
                    }
                }));

                let conn_import = conn.clone();
                let path_to_storage_path_tmp = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_tmp.clone();
                    let conn_import_import_inventory = conn_import.clone();

                    let vehicle_state = vehicle_state::import_vehicle_state(
                        &conn_import_import_inventory,
                        &path_to_storage_path_buf,
                    )
                    .await;

                    if let Ok(_vehicle_state) = vehicle_state {
                        info!("VehicleState imported");
                    } else {
                        error!("VehicleState import failed: {:?}", vehicle_state);
                    }
                }));

                let path_to_storage_path_buf = path_to_storage_path_buf.clone();
                importer.push(tokio::spawn(async move {
                    let path_to_storage_path_buf = path_to_storage_path_buf.clone();
                    let inventory =
                        inventory::import_inventory(&conn, &path_to_storage_path_buf).await;

                    if let Ok(_inventory) = inventory {
                        info!("Inventory imported");
                    } else {
                        error!("Inventory import failed: {:?}", inventory);
                    }
                }));

                info!("Importing data finished");

                futures::future::join_all(importer).await;

                // let control_c = tokio::signal::ctrl_c();
                // let _ = control_c.await;
            });
    });
}

#[derive(Serialize, Deserialize, Clone)]
enum WebSocketMessage {
    IdentityToken(IdentityToken),
    TransactionUpdate(TransactionUpdate),
    SubscriptionUpdate(SubscriptionUpdate),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct IdentityToken {
    identity: String,
    token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TransactionUpdate {
    event: Event,
    subscription_update: SubscriptionUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Event {
    timestamp: u64,
    status: String,
    caller_identity: String,
    function_call: FunctionCall,
    energy_quanta_used: u64,
    message: String,
    caller_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FunctionCall {
    reducer: String,
    args: String,
    request_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SubscriptionUpdate {
    table_updates: Vec<TableUpdate>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TableUpdate {
    table_id: i64,
    table_name: String,
    table_row_operations: Vec<TableRowOperation>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TableRowOperation {
    row: Value,
    op: String,
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
        error!("Error: {err}");
    }
}
