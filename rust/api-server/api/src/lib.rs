mod buildings;
mod cargo_desc;
mod claim_tech_desc;
mod claim_tech_state;
mod claims;
mod collectible_desc;
mod config;
mod deployable_state;
mod inventory;
mod items;
mod items_and_cargo;
mod leaderboard;
mod locations;
mod player_state;
mod recipes;
mod reducer_event_handler;
mod skill_descriptions;
mod trading_orders;
mod vault_state;
mod websocket;

use axum::{
    http::StatusCode,
    middleware,
    routing::{get, get_service},
    Router,
};
use service::sea_orm::{Database, DatabaseConnection};
use std::collections::{HashMap, HashSet};
use tower_http::cors::{Any, CorsLayer};

use crate::config::Config;
use crate::websocket::WebSocketMessages;
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    MatchedPath, Request, State,
};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::IntoResponse;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use log::{error, info};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use migration::{Migrator, MigratorTrait};
use reqwest::header::HeaderMap;
use reqwest::Client;
use sea_orm::ConnectOptions;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tower_cookies::CookieManagerLayer;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn start() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    dotenvy::dotenv().ok();
    let config = Config::new();

    tracing_subscriber::fmt::init();

    let prometheus = setup_metrics_recorder();

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
    Migrator::up(&conn, None).await?;

    if env::var("DOWNLOAD_ALL_TABLES").is_ok() {
        let client = create_default_client(config.clone());

        download_all_tables(
            &client,
            &config.spacetimedb.domain.clone(),
            &config.spacetimedb.protocol.clone(),
            &config.spacetimedb.database.clone(),
            &config.storage_path.clone().into(),
        )
        .await;
    }

    if config.import_enabled {
        import_data(config.clone());
    }

    let state = Arc::new(AppState {
        conn,
        storage_path: PathBuf::from(config.storage_path.clone()),
        user_map: Arc::new(RwLock::new(HashMap::new())),
    });

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(boradcast_message(state.clone(), rx));

    let app = create_app(&config, state, prometheus);

    let websocket_url = config.weboosocket_url();
    let websocket_password = config.spacetimedb.password.clone();
    let websocket_username = config.spacetimedb.username.clone();
    let database_name = config.spacetimedb.database.clone();

    if config.live_updates_ws {
        let tmp_config = config.clone();
        websocket::start_websocket_bitcraft_logic(
            websocket_url,
            websocket_password,
            websocket_username,
            database_name,
            tmp_config,
            tx,
        );
    }

    let server_url = config.server_url();
    let listener = tokio::net::TcpListener::bind(&server_url).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

#[allow(dead_code)]
struct ServerInstance {
    // cached_resources
    connected_clients: HashMap<String, (UnboundedSender<WebSocketMessages>, Vec<String>)>,
    topics_listen_to: HashMap<String, Vec<String>>,
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending chat messages).
async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let id = nanoid::nanoid!();

    let (tx, mut rx) = tokio::sync::broadcast::channel::<WebSocketMessages>(20);

    state
        .user_map
        .write()
        .await
        .insert(id.clone(), (tx.clone(), HashMap::new()));

    // // Username gets set in the receive loop, if it's valid.
    // let mut username = String::new();
    // // Loop until a text message is found.
    // while let Some(Ok(message)) = receiver.next().await {
    //     if let Message::Text(name) = message {
    //         // If username that is sent by client is not taken, fill username string.
    //
    //         // If not empty we want to quit the loop else we want to quit function.
    //         if !username.is_empty() {
    //             break;
    //         } else {
    //             // Only send our client that username is taken.
    //             let _ = sender
    //                 .send(Message::Text(String::from("Username already taken.")))
    //                 .await;
    //
    //             return;
    //         }
    //     }
    // }

    // Now send the "joined" message to all subscribers.
    let msg = format!("{id} joined.");
    let _ = tx.send(WebSocketMessages::Message(msg));

    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if let Err(error) = sender
                .send(Message::Text(serde_json::to_string(&msg).unwrap()))
                .await
            {
                tracing::error!("Error sending message to client: {error}");
                break;
            }
        }
    });

    let inner_id = id.clone();
    let inner_state = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            match serde_json::from_str::<WebSocketMessages>(&text) {
                Ok(message) => match message {
                    WebSocketMessages::Subscribe { topics } => {
                        let mut current_topics = inner_state
                            .user_map
                            .read()
                            .await
                            .get(&inner_id)
                            .unwrap()
                            .1
                            .clone();
                        let mut new_topics: HashMap<String, HashSet<i64>> = HashMap::new();
                        for topic in topics {
                            let (topic, id) = topic.split_once(".").unwrap();
                            let id = id.parse::<i64>().unwrap();

                            if let Some(current_topic) = current_topics.get_mut(&topic.to_string())
                            {
                                if !current_topic.contains(&id) {
                                    if let Some(new_topic) = new_topics.get_mut(&topic.to_string())
                                    {
                                        new_topic.insert(id);
                                    } else {
                                        new_topics.insert(topic.to_string(), HashSet::from([id]));
                                    }
                                }
                            } else {
                                if let Some(new_topic) = new_topics.get_mut(&topic.to_string()) {
                                    new_topic.insert(id);
                                } else {
                                    new_topics.insert(topic.to_string(), HashSet::from([id]));
                                }
                            }
                        }

                        for (topic, users) in new_topics {
                            if inner_state
                                .user_map
                                .read()
                                .await
                                .get(&inner_id)
                                .unwrap()
                                .1
                                .contains_key(&topic)
                            {
                                inner_state
                                    .user_map
                                    .write()
                                    .await
                                    .get_mut(&inner_id)
                                    .unwrap()
                                    .1
                                    .get_mut(&topic)
                                    .unwrap()
                                    .extend(users);
                            } else {
                                inner_state
                                    .user_map
                                    .write()
                                    .await
                                    .get_mut(&inner_id)
                                    .unwrap()
                                    .1
                                    .insert(topic, users);
                            }
                        }
                    }
                    WebSocketMessages::Unsubscribe { topic } => {
                        let (topic, id) = topic.split_once(".").unwrap();
                        let id = id.parse::<i64>().unwrap();

                        if let Some(current_topic) = inner_state
                            .user_map
                            .write()
                            .await
                            .get_mut(&inner_id)
                            .unwrap()
                            .1
                            .get_mut(&topic.to_string())
                        {
                            current_topic.remove(&id);
                        }

                        // inner_state
                        //     .user_map
                        //     .write()
                        //     .await
                        //     .get_mut(&inner_id)
                        //     .unwrap()
                        //     .1
                        //     .retain(|x| x != &topic);
                    }
                    WebSocketMessages::ListSubscribedTopics => {
                        let topics = inner_state
                            .user_map
                            .read()
                            .await
                            .get(&inner_id)
                            .unwrap()
                            .1
                            .clone();

                        let topics = topics
                            .into_iter()
                            .map(|(topic, users)| {
                                users
                                    .into_iter()
                                    .map(|user| format!("{}.{:?}", topic, user))
                                    .collect::<Vec<String>>()
                            })
                            .flatten()
                            .collect::<Vec<String>>();

                        let _ = tx.send(WebSocketMessages::SubscribedTopics(topics));
                    }
                    _ => {}
                },
                Err(error) => {
                    tracing::error!("Error handling websocket message from client: {error}");
                }
            }
        }
    });

    // If any one of the tasks run to completion, we abort the other.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    // Remove username from map so new clients can take it again.
    state.user_map.write().await.remove(&id);
}

async fn boradcast_message(state: Arc<AppState>, mut rx: UnboundedReceiver<WebSocketMessages>) {
    while let Some(message) = rx.recv().await {
        let clients = state.user_map.read().await;
        let connection_to_send_to = clients
            .iter()
            .filter(|(_, topics)| {
                let topic_name = match &message {
                    WebSocketMessages::Experience { user_id, .. } => {
                        ("experience".to_string(), user_id)
                    }
                    WebSocketMessages::Level { user_id, .. } => ("level".to_string(), user_id),
                    WebSocketMessages::PlayerState(player_state) => {
                        ("player_state".to_string(), &player_state.entity_id)
                    }
                    WebSocketMessages::ClaimDescriptionState(claim_description_state) => {
                        ("claim".to_string(), &claim_description_state.entity_id)
                    }
                    WebSocketMessages::Subscribe { .. } => return false,
                    WebSocketMessages::Message(_) => return false,
                    WebSocketMessages::Unsubscribe { .. } => return false,
                    WebSocketMessages::ListSubscribedTopics { .. } => return false,
                    WebSocketMessages::SubscribedTopics { .. } => return false,
                };

                if let Some(topics) = topics.1.get(&topic_name.0) {
                    return topics.contains(&topic_name.1);
                }

                false
            })
            .collect::<Vec<_>>();

        for (_user_id, (tx, _)) in connection_to_send_to {
            tx.send(message.clone()).unwrap();
        }
    }
}

// #[derive(Debug)]
// enum DisconnectReason {
//     Reconnect,
//     Timeout,
//     Unknown,
//     Disconnect,
// }
//
// async fn handle_disconnect_reconnect(state: Arc<AppState>, mut rx: UnboundedReceiver<DisconnectReason>) {
//     while let Some(message) = rx.recv().await {
//         let clients = state.user_map.read().await;
//         let connection_to_send_to = clients.iter().filter(|(_, topics)| {
//             let topic_name = match message {
//                 WebSocketMessages::Experience { user_id,.. } => { format!("experience.{user_id}") },
//                 WebSocketMessages::Subscribe { .. } => return false,
//                 WebSocketMessages::Message(_) => return false,
//                 WebSocketMessages::Unsubscribe{ .. } => return false,
//                 WebSocketMessages::ListSubscribedTopics{ .. } => return false,
//                 WebSocketMessages::SubscribedTopics{ .. } => return false,
//             }.to_string();
//
//             topics.1.contains(&topic_name)
//         }).collect::<Vec<_>>();
//
//         for (user_id, (tx, _)) in connection_to_send_to {
//             tx.send(message.clone()).unwrap();
//         }
//     }
// }

pub(crate) type AppRouter = Router<Arc<AppState>>;

fn create_app(config: &Config, state: Arc<AppState>, prometheus: PrometheusHandle) -> Router {
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
        .route("/websocket", get(websocket_handler))
        .route("/locations", get(locations::list_locations))
        .route("/items", get(items::list_items))
        .merge(player_state::get_routes())
        .merge(claims::get_routes())
        .merge(buildings::get_routes())
        .merge(inventory::get_routes())
        .merge(recipes::get_routes())
        .merge(items_and_cargo::get_routes())
        .merge(leaderboard::get_routes())
        .merge(trading_orders::get_routes())
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
        .route("/metrics", get(|| async move { prometheus.render() }))
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
        .layer(CompressionLayer::new())
        .route_layer(middleware::from_fn(track_metrics))
        .with_state(state);
    app
}

fn create_default_client(config: Config) -> Client {
    let mut default_header = HeaderMap::new();
    default_header.insert(
        "Authorization",
        format!(
            "Basic {}",
            base64::prelude::BASE64_STANDARD.encode(format!(
                "{}:{}",
                config.spacetimedb.username, config.spacetimedb.password
            ))
        )
        .parse()
        .unwrap(),
    );
    default_header.insert(
        "User-Agent",
        format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION"))
            .parse()
            .unwrap(),
    );

    Client::builder()
        .timeout(Duration::from_secs(60))
        .gzip(true)
        .deflate(true)
        .brotli(true)
        .pool_idle_timeout(Duration::from_secs(20))
        .default_headers(default_header)
        .build()
        .unwrap()
}

async fn create_importer_default_db_connection(config: Config) -> DatabaseConnection {
    let mut connection_options = ConnectOptions::new(config.database.url.clone());
    connection_options
        .max_connections(20)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .connect_lazy(true)
        .max_lifetime(Duration::from_secs(60))
        .sqlx_logging(env::var("SQLX_LOG").is_ok());

    Database::connect(connection_options)
        .await
        .expect("Database connection failed")
}

fn import_data(config: Config) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut tasks = vec![];

                //
                // let temp_config = config.clone();
                // tasks.push(tokio::spawn(async move {
                //     let config = temp_config.clone();
                //     if config.live_updates {
                //         loop {
                //             let conn = create_importer_default_db_connection(config.clone()).await;
                //             let client = create_default_client(config.clone());
                //
                //             let now = Instant::now();
                //             let now_in = now.add(Duration::from_secs(60));
                //
                //             import_recipes(config.clone(), conn, client);
                //
                //             let now = Instant::now();
                //             let wait_time = now_in.duration_since(now);
                //
                //             if wait_time.as_secs() > 0 {
                //                 tokio::time::sleep(wait_time).await;
                //             }
                //         }
                //     } else {
                //         let conn = create_importer_default_db_connection(config.clone()).await;
                //         let client = create_default_client(config.clone());
                //
                //         import_recipes(config.clone(), conn, client);
                //     }
                // }));
                // let temp_config = config.clone();
                // tasks.push(tokio::spawn(async move {
                //     let config = temp_config.clone();
                //     if config.live_updates {
                //         loop {
                //             let conn = create_importer_default_db_connection(config.clone()).await;
                //             let client = create_default_client(config.clone());
                //
                //             let now = Instant::now();
                //             let now_in = now.add(Duration::from_secs(60));
                //
                //             import_claim_tech_desc(config.clone(), conn, client);
                //
                //             let now = Instant::now();
                //             let wait_time = now_in.duration_since(now);
                //
                //             if wait_time.as_secs() > 0 {
                //                 tokio::time::sleep(wait_time).await;
                //             }
                //         }
                //     } else {
                //         let conn = create_importer_default_db_connection(config.clone()).await;
                //         let client = create_default_client(config.clone());
                //
                //         import_claim_tech_desc(config.clone(), conn, client);
                //     }
                // }));
                //
                // let temp_config = config.clone();
                // tasks.push(tokio::spawn(async move {
                //     let config = temp_config.clone();
                //     if config.live_updates {
                //         loop {
                //             let conn = create_importer_default_db_connection(config.clone()).await;
                //             let client = create_default_client(config.clone());
                //
                //             let now = Instant::now();
                //             let now_in = now.add(Duration::from_secs(60));
                //
                //             import_skill_descs(config.clone(), conn, client);
                //
                //             let now = Instant::now();
                //             let wait_time = now_in.duration_since(now);
                //
                //             if wait_time.as_secs() > 0 {
                //                 tokio::time::sleep(wait_time).await;
                //             }
                //         }
                //     } else {
                //         let conn = create_importer_default_db_connection(config.clone()).await;
                //         let client = create_default_client(config.clone());
                //
                //         import_skill_descs(config.clone(), conn, client);
                //     }
                // }));
                //
                //

                //
                // let temp_config = config.clone();
                // tasks.push(tokio::spawn(async move {
                //     let config = temp_config.clone();
                //     if config.live_updates {
                //         loop {
                //             let conn = create_importer_default_db_connection(config.clone()).await;
                //             let client = create_default_client(config.clone());
                //
                //             let now = Instant::now();
                //             let now_in = now.add(Duration::from_secs(60));
                //
                //             import_trade_order_state(config.clone(), conn, client);
                //
                //             let now = Instant::now();
                //             let wait_time = now_in.duration_since(now);
                //
                //             if wait_time.as_secs() > 0 {
                //                 tokio::time::sleep(wait_time).await;
                //             }
                //         }
                //     } else {
                //         let conn = create_importer_default_db_connection(config.clone()).await;
                //         let client = create_default_client(config.clone());
                //
                //         import_trade_order_state(config.clone(), conn, client);
                //     }
                // }));
                //

                if config.enabled_importer.contains(&"skill_desc".to_string())
                    || config.enabled_importer.len() == 0
                {
                    let temp_config = config.clone();
                    tasks.push(tokio::spawn(skill_descriptions::import_job_skill_desc(
                        temp_config,
                    )));
                }

                if config
                    .enabled_importer
                    .contains(&"claim_tech_desc".to_string())
                    || config.enabled_importer.len() == 0
                {
                    let temp_config = config.clone();
                    tasks.push(tokio::spawn(claim_tech_desc::import_job_claim_tech_desc(
                        temp_config,
                    )));
                }

                if config.enabled_importer.contains(&"items".to_string())
                    || config.enabled_importer.len() == 0
                {
                    let temp_config = config.clone();
                    tasks.push(tokio::spawn(items::import_job_item_desc(temp_config)));
                }

                if config.enabled_importer.contains(&"cargo_desc".to_string())
                    || config.enabled_importer.len() == 0
                {
                    let temp_config = config.clone();
                    tasks.push(tokio::spawn(cargo_desc::import_job_cargo_desc(temp_config)));
                }

                if config
                    .enabled_importer
                    .contains(&"building_desc".to_string())
                    || config.enabled_importer.len() == 0
                {
                    let temp_config = config.clone();
                    tasks.push(tokio::spawn(buildings::import_job_building_desc(
                        temp_config,
                    )));
                }

                futures::future::join_all(tasks).await;
            });
    });
}

#[allow(dead_code)]
fn import_recipes(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = recipes::load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("Recipes imported");
                } else {
                    error!("Recipes import failed: {:?}", vehicle_state);
                }
            });
    });
}

#[allow(dead_code)]
fn import_skill_descs(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = recipes::load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("SkillDescriptions imported");
                } else {
                    error!("SkillDescriptions import failed: {:?}", vehicle_state);
                }
            });
    });
}

#[allow(dead_code)]
fn import_trade_order_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let trade_order_state = trading_orders::load_trade_order(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                    &config,
                )
                .await;

                if let Ok(_) = trade_order_state {
                    info!("TradeOrderState imported");
                } else {
                    error!("TradeOrderState import failed: {:?}", trade_order_state);
                }
            });
    });
}

#[derive(Clone)]
struct AppState {
    conn: DatabaseConnection,
    storage_path: PathBuf,
    user_map: Arc<
        RwLock<
            HashMap<
                String,
                (
                    tokio::sync::broadcast::Sender<crate::websocket::WebSocketMessages>,
                    HashMap<String, HashSet<i64>>,
                ),
            >,
        >,
    >,
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

    const EXPONENTIAL_SECONDS_INITIAL_SUBSCRIPTION: &[f64] = &[
        0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 100.0, 500.0, 1000.0,
    ];

    const EXPONENTIAL_SECONDS_TRANSACTION_UPDATE: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full(
                "bitraft_event_handler_initial_subscription_duration_seconds".to_string(),
            ),
            EXPONENTIAL_SECONDS_INITIAL_SUBSCRIPTION,
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full("bitraft_event_handler_transaction_update_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS_TRANSACTION_UPDATE,
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

pub async fn download_all_tables(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
    storage_path: &PathBuf,
) {
    let desc_tables = vec![
        "AchievementDesc",
        "AlertDesc",
        "BiomeDesc",
        "BuffDesc",
        "BuffTypeDesc",
        "BuildingClaimDesc",
        "BuildingDesc",
        "BuildingFunctionTypeMappingDesc",
        "BuildingPortalDesc",
        "BuildingRepairsDesc",
        "BuildingSpawnDesc",
        "BuildingTypeDesc",
        "CargoDesc",
        "CharacterStatDesc",
        "ChestRarityDesc",
        "ClaimDescriptionState",
        "ClaimTechDesc",
        "ClimbRequirementDesc",
        "ClothingDesc",
        "CollectibleDesc",
        "CombatActionDesc",
        "ConstructionRecipeDesc",
        "CraftingRecipeDesc",
        "DeconstructionRecipeDesc",
        "DeployableDesc",
        "DimensionDescriptionState",
        "ElevatorDesc",
        "EmoteDesc",
        "EmpireColorDesc",
        "EmpireNotificationDesc",
        "EmpireRankDesc",
        "EmpireSuppliesDesc",
        "EmpireTerritoryDesc",
        "EnemyAiParamsDesc",
        "EnemyDesc",
        "EnvironmentDebuffDesc",
        "EquipmentDesc",
        "ExtractionRecipeDesc",
        "FoodDesc",
        "GateDesc",
        "InteriorInstanceDesc",
        "InteriorNetworkDesc",
        "InteriorPortalConnectionsDesc",
        "InteriorShapeDesc",
        "InteriorSpawnDesc",
        "ItemConversionRecipeDesc",
        "ItemDesc",
        "ItemListDesc",
        "KnowledgeScrollDesc",
        "KnowledgeScrollTypeDesc",
        "LootChestDesc",
        "LootRarityDesc",
        "LootTableDesc",
        "NpcDesc",
        "OnboardingRewardDesc",
        "ParametersDesc",
        "PathfindingDesc",
        "PavingTileDesc",
        "PlayerActionDesc",
        "PrivateParametersDesc",
        "ResourceClumpDesc",
        "ResourceDesc",
        "ResourceGrowthRecipeDesc",
        "ResourcePlacementRecipeDesc",
        "SecondaryKnowledgeDesc",
        "SingleResourceToClumpDesc",
        "SkillDesc",
        "TargetingMatrixDesc",
        "TeleportItemDesc",
        "TerraformRecipeDesc",
        "ToolDesc",
        "ToolTypeDesc",
        "TravelerTradeOrderDesc",
        "WallDesc",
        "WeaponDesc",
        "WeaponTypeDesc",
    ];

    let state_tables = vec![
        "AIDebugState",
        "ActionState",
        "ActiveBuffState",
        "AdminRestorePlayerStateTimer",
        "AlertState",
        "AttachedHerdsState",
        "AttackOutcomeState",
        "AutoClaimState",
        "BarterStallState",
        "BuildingState",
        "CargoState",
        "CharacterStatsState",
        "ChatMessageState",
        "ClaimDescriptionState",
        "ClaimRecruitmentState",
        "ClaimTechState",
        "ClaimTileState",
        "CombatState",
        "DeployableCollectibleState",
        "DeployableState",
        "DimensionDescriptionState",
        "DimensionNetworkState",
        "EmpireChunkState",
        "EmpireExpansionState",
        "EmpireFoundryState",
        "EmpireLogState",
        "EmpireNodeSiegeState",
        "EmpireNodeState",
        "EmpireNotificationState",
        "EmpirePlayerDataState",
        "EmpirePlayerLogState",
        "EmpireRankState",
        "EmpireSettlementState",
        "EmpireSiegeEngineState",
        "EmpireState",
        "EnemyMobMonitorState",
        "EnemyState",
        "EquipmentState",
        "ExperienceState",
        "ExplorationChunksState",
        "FootprintTileState",
        "GlobalSearchState",
        "GrowthState",
        "HealthState",
        "HerdState",
        "InteriorCollapseTriggerState",
        "InventoryState",
        "ItemPileState",
        "KnowledgeAchievementState",
        "KnowledgeBattleActionState",
        "KnowledgeBuildingState",
        "KnowledgeCargoState",
        "KnowledgeConstructionState",
        "KnowledgeCraftState",
        "KnowledgeDeployableState",
        "KnowledgeEnemyState",
        "KnowledgeExtractState",
        "KnowledgeItemState",
        "KnowledgeLoreState",
        "KnowledgeNpcState",
        "KnowledgePavingState",
        "KnowledgeResourcePlacementState",
        "KnowledgeResourceState",
        "KnowledgeRuinsState",
        "KnowledgeSecondaryState",
        "KnowledgeVaultState",
        "LightSourceState",
        "LocationState",
        "LootChestState",
        "MobileEntityState",
        "MountingState",
        "MoveValidationStrikeCounterState",
        "NpcState",
        "OnboardingState",
        "PassiveCraftState",
        "PavedTileState",
        "PlayerActionState",
        "PlayerLowercaseUsernameState",
        "PlayerNoteState",
        "PlayerPrefsState",
        "PlayerState",
        "PlayerTimestampState",
        "PlayerUsernameState",
        "PlayerVoteState",
        "PortalState",
        "ProgressiveActionState",
        "ProjectSiteState",
        "RentState",
        "ResourceState",
        "SatiationState",
        "SignedInPlayerState",
        "SignedInUserState",
        "StaminaState",
        "StarvingPlayerState",
        "TargetState",
        "TargetableState",
        "TerraformProgressState",
        "TerrainChunkState",
        "ThreatState",
        "ToolbarState",
        "TradeOrderState",
        "TradeSessionState",
        "UnclaimedCollectiblesState",
        "UnclaimedShardsState",
        "UserModerationState",
        "UserSignInState",
        "UserState",
        "VaultState",
    ];

    let rest_tables = vec![
        "AdminBroadcast",
        "AttackImpactTimer",
        "AttackTimer",
        "AutoLogoutLoopTimer",
        "BuildingDecayLoopTimer",
        "BuildingDespawnTimer",
        "CargoDespawnTimer",
        "CargoSpawnTimer",
        "ChatCache",
        "ClaimTechUnlockTimer",
        "ClaimTileCost",
        "CollectStatsTimer",
        "Config",
        "DayNightLoopTimer",
        "DeployableDismountTimer",
        "DestroyDimensionNetworkTimer",
        "EmpireCraftSuppliesTimer",
        "EmpireDecayLoopTimer",
        "EmpireSiegeLoopTimer",
        "EndGracePeriodTimer",
        "EnemyDespawnTimer",
        "EnemyRegenLoopTimer",
        "EnvironmentDebuffLoopTimer",
        "ForceGenerateTypes",
        "Globals",
        "GlobalsAppeared",
        "GrowthLoopTimer",
        "HideDeployableTimer",
        "IdentityRole",
        "InteriorSetCollapsedTimer",
        "ItemPileDespawnTimer",
        "LocationCache",
        "LootChestDespawnTimer",
        "LootChestSpawnTimer",
        "NpcAiLoopTimer",
        "PassiveCraftTimer",
        "PlayerDeathTimer",
        "PlayerRegenLoopTimer",
        "PlayerRespawnAfterDeathTimer",
        "PlayerUseElevatorTimer",
        "PlayerVoteConcludeTimer",
        "RentCollectorLoopTimer",
        "RentEvictTimer",
        "ResetChunkIndexTimer",
        "ResetMobileEntityTimer",
        "ResourceCount",
        "ResourceSpawnTimer",
        "ResourcesLog",
        "ResourcesRegenLoopTimer",
        "RespawnResourceInChunkTimer",
        "ServerIdentity",
        "SingleResourceClumpInfo",
        "StagedStaticData",
        "StarvingLoopTimer",
        "TeleportPlayerTimer",
        "TradeSessionLoopTimer",
    ];

    for table in desc_tables {
        let desc_result = download_all_table(
            client,
            domain,
            protocol,
            database,
            table,
            storage_path,
            "desc",
        )
        .await;

        if let Err(error) = desc_result {
            error!("Error while downloading desc table: {error}");
        }
    }

    for table in state_tables {
        let state_result = download_all_table(
            client,
            domain,
            protocol,
            database,
            table,
            storage_path,
            "state",
        )
        .await;

        if let Err(error) = state_result {
            error!("Error while downloading state table: {error}");
        }
    }

    for table in rest_tables {
        let rest_result = download_all_table(
            client,
            domain,
            protocol,
            database,
            table,
            storage_path,
            "rest",
        )
        .await;

        if let Err(error) = rest_result {
            error!("Error while downloading rest table: {error}");
        }
    }
}

///
/// Donwload the table and save it to the storage path with the type as the folder before the name
pub async fn download_all_table(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
    table: &str,
    storage_path: &PathBuf,
    folder: &str,
) -> anyhow::Result<()> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body(format!("SELECT * FROM {table}"))
        .send()
        .await;

    let json = match response {
        Ok(response) => {
            if !response.status().is_success() {
                let error = response.text().await?;
                error!("Error: {error}");
                return Err(anyhow::anyhow!("Error: {error}"));
            }

            response.text().await?
        }
        Err(error) => {
            error!("Error: {error}");
            return Err(anyhow::anyhow!("Error: {error}"));
        }
    };

    let folder_to_create = storage_path.join(folder);
    if !folder_to_create.exists() {
        std::fs::create_dir_all(&folder_to_create)?;
    }
    let path = storage_path.join(format!("{folder}/{table}.json"));
    let mut file = File::create(&path)?;

    println!("Saving to {path:?}");

    file.write_all(json.as_bytes())?;

    Ok(())
}

pub fn main() {
    let result = start();

    if let Some(err) = result.err() {
        error!("Error: {err}");
    }
}
