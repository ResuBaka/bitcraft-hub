mod buildings;
mod cargo_desc;
mod claim_tech_desc;
mod claim_tech_state;
mod claims;
mod collectible_desc;
mod config;
mod deployable_state;
mod download;
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

use crate::config::Config;
use crate::websocket::WebSocketMessages;
use axum::extract::{
    MatchedPath, Query, Request, State,
    ws::{Message, WebSocket, WebSocketUpgrade},
};
use axum::http::{HeaderValue, Version};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::{
    Router,
    http::StatusCode,
    middleware,
    routing::{get, get_service},
};
use clap::{Parser, Subcommand};
use futures::{SinkExt, StreamExt};
use log::error;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use migration::{Migrator, MigratorTrait};
use reqwest::Client;
use reqwest::header::HeaderMap;
use sea_orm::ConnectOptions;
use sea_orm::strum::Display;
use sea_orm_cli::MigrateSubcommands;
use serde::Deserialize;
use service::sea_orm::{Database, DatabaseConnection};
use spacetimedb_sdk::Identity;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Display;
use std::ops::{AddAssign, SubAssign};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tower_cookies::CookieManagerLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt;

async fn start(database_connection: DatabaseConnection, config: Config) -> anyhow::Result<()> {
    let prometheus = setup_metrics_recorder();

    Migrator::up(&database_connection, None).await?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState::new(
        database_connection.clone(),
        &config,
        tx.clone(),
    ));

    let server_url = config.server_url();

    let server_url = if server_url.is_err() {
        log::error!("Could not create socket {}", server_url.err().unwrap());
        exit(1)
    } else {
        server_url?
    };

    if config.live_updates_ws {
        if config.spacetimedb.databases.is_empty() {
            tracing::warn!("You need to set spacetimedb databases");
        } else {
            tracing::info!("Staring Bitcraft websocket connection");

            tokio::spawn(broadcast_message(state.clone(), rx));

            let tmp_config = config.clone();

            config.spacetimedb.databases.iter().for_each(|connection_state| {
                state.connection_state.insert(connection_state.clone(), false);
            });

            websocket::start_websocket_bitcraft_logic(tmp_config, state.clone());
        }
    }

    let app = create_app(&config, state.clone(), prometheus);

    tracing::info!("Starting server on http://{}", server_url);

    let listener = tokio::net::TcpListener::bind(&server_url).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn create_db_connection(config: &Config) -> DatabaseConnection {
    let mut connection_options = ConnectOptions::new(config.database.url.clone());
    connection_options
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .set_schema_search_path("public")
        .connect_timeout(Duration::from_secs(config.database.connect_timeout))
        .idle_timeout(Duration::from_secs(config.database.idle_timeout))
        .sqlx_logging(env::var("SQLX_LOG").is_ok());

    if let Some(max_lifetime) = config.database.max_lifetime {
        connection_options = connection_options
            .max_lifetime(Duration::from_secs(max_lifetime))
            .to_owned();
    }

    let mut connection = Database::connect(connection_options)
        .await
        .expect("Database connection failed");

    connection.set_metric_callback(|arg| {
        let latency = arg.elapsed.as_secs_f64();
        let first_parts = arg
            .statement
            .sql
            .split_whitespace()
            .take(4)
            .collect::<Vec<&str>>();

        let labels = if let Some(possible_query_type) = first_parts.first() {
            if possible_query_type.eq(&"SELECT") || possible_query_type.eq(&"select") {
                let method = "SELECT".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else if possible_query_type.eq(&"UPDATE") || possible_query_type.eq(&"update") {
                let method = "UPDATE".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else if possible_query_type.eq(&"INSERT") || possible_query_type.eq(&"insert") {
                let method = "INSERT".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else if possible_query_type.eq(&"ALTER") || possible_query_type.eq(&"alter") {
                let method = "ALTER".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else if possible_query_type.eq(&"CREATE") || possible_query_type.eq(&"create") {
                let method = "CREATE".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else if possible_query_type.eq(&"DELETE") || possible_query_type.eq(&"delete") {
                let method = "DELETE".to_string();
                [("method", method), ("failed", arg.failed.to_string())]
            } else {
                tracing::info!("Could not find type for {possible_query_type}");
                [
                    ("method", "UNKNOWN".to_string()),
                    ("failed", arg.failed.to_string()),
                ]
            }
        } else {
            tracing::info!("Could not find type for {}", arg.statement.sql);
            [
                ("method", "UNKNOWN".to_string()),
                ("failed", arg.failed.to_string()),
            ]
        };

        metrics::counter!("database_query_total", &labels).increment(1);
        metrics::histogram!("database_query_duration_seconds", &labels).record(latency);
    });

    connection
}

#[derive(Deserialize)]
struct QueryWebsocketOptions {
    encoding: Option<WebsocketEncoding>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    version: Version,
    State(state): State<Arc<AppState>>,
    Query(websocket_options): Query<QueryWebsocketOptions>,
) -> impl IntoResponse {
    tracing::debug!("Websocket upgraded with version: {version:?}");
    ws.on_upgrade(|socket| websocket(socket, state, websocket_options))
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
async fn websocket(
    stream: WebSocket,
    state: Arc<AppState>,
    websocket_options: QueryWebsocketOptions,
) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let id = nanoid::nanoid!();

    let (tx, mut rx) = tokio::sync::broadcast::channel::<WebSocketMessages>(20);

    state
        .clients_state
        .add_client(
            id.clone(),
            tx.clone(),
            websocket_options
                .encoding
                .map_or(WebsocketEncoding::Json, |value| value),
        )
        .await;

    // Now send the "joined" message to all subscribers.
    let msg = format!("{id} joined.");
    let _ = tx.send(WebSocketMessages::Message(msg));

    let internal_id = id.clone();
    let inner_state = state.clone();
    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let a = axum_codec::Codec(msg.clone());
            let encoding = inner_state
                .clients_state
                .get_encode_format_for_client(&internal_id)
                .await;

            if encoding.is_none() {
                tracing::warn!("Could not find encoding for {internal_id}")
            };

            let encoding = encoding.unwrap();

            let send_result = if encoding == WebsocketEncoding::Json {
                sender
                    .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                    .await
            } else if encoding == WebsocketEncoding::Toml {
                sender
                    .send(Message::Text(a.to_toml().unwrap().into()))
                    .await
            } else if encoding == WebsocketEncoding::Yaml {
                sender
                    .send(Message::Text(a.to_yaml().unwrap().into()))
                    .await
            } else if encoding == WebsocketEncoding::MessagePack {
                sender
                    .send(Message::Binary(a.to_msgpack().unwrap().into()))
                    .await
            } else {
                tracing::warn!("Unsupported encoding {encoding}");
                continue;
            };

            // In any websocket error, break loop.
            if let Err(error) = send_result {
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
                        for topic in topics {
                            let (topic, id) = topic.split_once(".").unwrap();
                            let id = id.parse::<i64>().unwrap();

                            inner_state
                                .clients_state
                                .add_topic_to_client(&inner_id, &topic.to_string(), id)
                                .await;
                        }
                    }
                    WebSocketMessages::Unsubscribe { topic } => {
                        let (topic, id) = topic.split_once(".").unwrap();
                        let id = id.parse::<i64>().unwrap();

                        inner_state
                            .clients_state
                            .remove_topic_from_client(&inner_id, &topic.to_string(), id)
                            .await;
                    }
                    WebSocketMessages::ListSubscribedTopics => {
                        let topics = inner_state
                            .clients_state
                            .get_topics_for_client(&inner_id)
                            .await;

                        if topics.is_none() {
                            continue;
                        }

                        let topics = topics.unwrap();
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
    state.clients_state.remove_client(&id).await;
}

async fn broadcast_message(state: Arc<AppState>, mut rx: UnboundedReceiver<WebSocketMessages>) {
    while let Some(message) = rx.recv().await {
        if message.topics().is_none() {
            continue;
        }

        let topics = message.topics().unwrap();
        let mut senders = vec![];

        for (topic_name, topic_id) in topics {
            senders.extend(
                state
                    .clients_state
                    .clients_listen_to_topic(&topic_name, topic_id)
                    .await,
            );
        }

        for sender in senders {
            sender.send(message.clone()).unwrap();
        }
    }
}

pub(crate) type AppRouter = Router<Arc<AppState>>;

fn create_app(config: &Config, state: Arc<AppState>, prometheus: PrometheusHandle) -> Router {
    let desc_router = Router::new()
        .route(
            "/buildings/{id}",
            axum_codec::routing::get(buildings::find_claim_description).into(),
        )
        .route(
            "/buildings",
            axum_codec::routing::get(buildings::find_building_descriptions).into(),
        );

    Router::new()
        .route("/websocket", any(websocket_handler))
        // .route(
        //     "/locations",
        //     axum_codec::routing::get(locations::list_locations).into(),
        // )
        .route("/items", axum_codec::routing::get(items::list_items).into())
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
        .with_state(state)
}

fn create_default_client(config: Config) -> Client {
    let mut default_header = HeaderMap::new();
    default_header.insert(
        "Authorization",
        format!("Bearer {}", config.spacetimedb.password)
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

    let mut connection = Database::connect(connection_options)
        .await
        .expect("Database connection failed");

    connection.set_metric_callback(|arg| {
        tracing::warn!(
            "Query {}, Elapsed {}, Failed {}",
            arg.statement.sql,
            arg.elapsed.as_millis(),
            arg.failed
        );
    });

    connection
}

#[derive(Clone)]
struct AppState {
    conn: DatabaseConnection,
    tx: UnboundedSender<WebSocketMessages>,
    connection_state: Arc<dashmap::DashMap<String, bool>>,
    storage_path: PathBuf,
    clients_state: Arc<ClientsState>,
    mobile_entity_state: Arc<dashmap::DashMap<u64, entity::mobile_entity_state::Model>>,
    claim_member_state:
        Arc<dashmap::DashMap<u64, dashmap::DashMap<i64, entity::claim_member_state::Model>>>,
    player_to_claim_id_cache: Arc<dashmap::DashMap<u64, dashmap::DashSet<u64>>>,
    claim_local_state: Arc<dashmap::DashMap<u64, entity::claim_local_state::Model>>,
    claim_tile_state: Arc<dashmap::DashMap<u64, entity::claim_tile_state::Model>>,
    player_action_state: Arc<dashmap::DashMap<u64, entity::player_action_state::Model>>,
    crafting_recipe_desc: Arc<dashmap::DashMap<i32, entity::crafting_recipe::Model>>,
    claim_tech_desc: Arc<dashmap::DashMap<i32, entity::claim_tech_desc::Model>>,
    item_tags: Arc<dashmap::DashSet<String>>,
    item_tiers: Arc<dashmap::DashSet<i64>>,
    item_desc: Arc<dashmap::DashMap<i32, entity::item_desc::Model>>,
    item_list_desc: Arc<dashmap::DashMap<i32, entity::item_list_desc::Model>>,
    skill_desc: Arc<dashmap::DashMap<i64, entity::skill_desc::Model>>,
    cargo_desc: Arc<dashmap::DashMap<i32, entity::cargo_desc::Model>>,
    cargo_tags: Arc<dashmap::DashSet<String>>,
    building_desc: Arc<dashmap::DashMap<i64, entity::building_desc::Model>>,
    building_nickname_state: Arc<dashmap::DashMap<i64, entity::building_nickname_state::Model>>,
    cargo_tiers: Arc<dashmap::DashSet<i64>>,
    action_state: Arc<dashmap::DashMap<u64, dashmap::DashMap<u64, entity::action_state::Model>>>,
    location_state: Arc<dashmap::DashMap<i64, entity::location::Model>>,
    inventory_state: Arc<dashmap::DashMap<i64, ::entity::inventory::Model>>,
    connected_user_map: Arc<dashmap::DashMap<String, i64>>,
    user_state: Arc<dashmap::DashMap<Identity, u64>>,
}

impl AppState {
    fn new(
        conn: DatabaseConnection,
        config: &Config,
        tx: UnboundedSender<WebSocketMessages>,
    ) -> Self {
        Self {
            conn,
            tx,
            connection_state: Arc::new(dashmap::DashMap::new()),
            storage_path: PathBuf::from(config.storage_path.clone()),
            clients_state: Arc::new(ClientsState::new()),
            mobile_entity_state: Arc::new(dashmap::DashMap::new()),
            claim_member_state: Arc::new(dashmap::DashMap::new()),
            player_to_claim_id_cache: Arc::new(dashmap::DashMap::new()),
            claim_local_state: Arc::new(dashmap::DashMap::new()),
            claim_tile_state: Arc::new(dashmap::DashMap::new()),
            player_action_state: Arc::new(dashmap::DashMap::new()),
            crafting_recipe_desc: Arc::new(dashmap::DashMap::new()),
            claim_tech_desc: Arc::new(dashmap::DashMap::new()),
            item_tags: Arc::new(dashmap::DashSet::new()),
            item_tiers: Arc::new(dashmap::DashSet::new()),
            item_desc: Arc::new(dashmap::DashMap::new()),
            item_list_desc: Arc::new(dashmap::DashMap::new()),
            skill_desc: Arc::new(dashmap::DashMap::new()),
            cargo_tags: Arc::new(dashmap::DashSet::new()),
            cargo_tiers: Arc::new(dashmap::DashSet::new()),
            cargo_desc: Arc::new(dashmap::DashMap::new()),
            building_desc: Arc::new(dashmap::DashMap::new()),
            building_nickname_state: Arc::new(dashmap::DashMap::new()),
            action_state: Arc::new(dashmap::DashMap::new()),
            location_state: Arc::new(dashmap::DashMap::new()),
            inventory_state: Arc::new(dashmap::DashMap::new()),
            connected_user_map: Arc::new(dashmap::DashMap::new()),
            user_state: Arc::new(dashmap::DashMap::new()),
        }
    }

    fn add_claim_member(&self, claim_member_state: entity::claim_member_state::Model) {
        let claim_entity_id = claim_member_state.claim_entity_id;
        let entity_id = claim_member_state.entity_id;

        let cms = self.claim_member_state.get(&(claim_entity_id as u64));

        if let Some(cms) = cms {
            cms.insert(claim_member_state.entity_id,claim_member_state);
        } else {
            let dashset = dashmap::DashMap::new();
            dashset.insert(claim_member_state.entity_id, claim_member_state);

            self.claim_member_state
                .insert(claim_entity_id as u64, dashset);
        }

        if let Some(claim_state_to_member_set) =
            self.player_to_claim_id_cache.get_mut(&(entity_id as u64))
        {
            claim_state_to_member_set.insert(claim_entity_id as u64);
        } else {
            let claim_state_to_member_set = dashmap::DashSet::new();
            claim_state_to_member_set.insert(claim_entity_id as u64);

            self.player_to_claim_id_cache
                .insert(entity_id as u64, claim_state_to_member_set);
        };
    }

    fn remove_claim_member(&self, claim_member_state: entity::claim_member_state::Model) {
        let claim_entity_id = claim_member_state.claim_entity_id;
        let entity_id = claim_member_state.entity_id;

        self.claim_member_state.iter().for_each(|cms| {
            cms.remove(&claim_member_state.entity_id);
        });

        if let Some(claim_state_to_member_set) =
            self.player_to_claim_id_cache.get_mut(&(entity_id as u64))
        {
            claim_state_to_member_set.remove(&(claim_entity_id as u64));
        };
    }
}

#[derive(Deserialize, Display, Copy, Clone, Eq, PartialEq)]
enum WebsocketEncoding {
    Json,
    Toml,
    Yaml,
    MessagePack,
}

type WebsocketClient = (
    tokio::sync::broadcast::Sender<crate::websocket::WebSocketMessages>,
    HashMap<String, HashSet<i64>>,
    WebsocketEncoding,
);

struct ClientsState {
    clients: Arc<RwLock<HashMap<String, WebsocketClient>>>,
    topics_listen_to: Arc<RwLock<HashMap<String, HashMap<i64, u64>>>>,
}

impl ClientsState {
    fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            topics_listen_to: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) async fn add_client(
        &self,
        id: String,
        tx: tokio::sync::broadcast::Sender<crate::websocket::WebSocketMessages>,
        encoding: WebsocketEncoding,
    ) {
        self.clients
            .write()
            .await
            .insert(id, (tx, HashMap::new(), encoding));
        metrics::gauge!("websocket_clients_connected_total").increment(1);
    }

    pub(crate) async fn remove_client(&self, id: &String) {
        let current_topics = self.clients.read().await.get(id).unwrap().1.clone();
        for (topic, topic_ids) in current_topics {
            for topic_id in topic_ids {
                self.remove_topic_from_client(id, &topic, topic_id).await;
            }
        }

        self.clients.write().await.remove(id);
        metrics::gauge!("websocket_clients_connected_total").decrement(1);
    }

    pub(crate) async fn get_encode_format_for_client(
        &self,
        id: &String,
    ) -> Option<WebsocketEncoding> {
        self.clients.read().await.get(id).map(|client| client.2)
    }

    pub(crate) async fn get_topics_for_client(&self, id: &String) -> Option<Vec<String>> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(id) {
            return Some(
                client
                    .1
                    .iter()
                    .flat_map(|(topic, ids)| {
                        ids.iter()
                            .map(|id| format!("{}.{:?}", topic, id))
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<String>>(),
            );
        }

        None
    }

    pub(crate) async fn add_topic_to_client(&self, id: &String, topic: &String, topic_id: i64) {
        if self
            .client_listen_to_topics(id, vec![(topic.clone(), topic_id)])
            .await
        {
            return;
        }

        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(id) {
            if let Some(topics) = client.1.get_mut(topic) {
                topics.insert(topic_id);
            } else {
                client.1.insert(topic.clone(), HashSet::from([topic_id]));
            }
        }

        let mut topics_listen_to = self.topics_listen_to.write().await;
        if let Some(found_topic) = topics_listen_to.get_mut(topic) {
            if let Some(current_topic) = found_topic.get_mut(&topic_id) {
                current_topic.add_assign(1);
            } else {
                found_topic.insert(topic_id, 1);
            }
        } else {
            topics_listen_to.insert(topic.clone(), HashMap::from([(topic_id, 1)]));
        }
    }

    pub(crate) async fn remove_topic_from_client(
        &self,
        id: &String,
        topic: &String,
        topic_id: i64,
    ) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(id) {
            if let Some(topics) = client.1.get_mut(topic) {
                topics.remove(&topic_id);
            }
        }

        let mut topics_listen_to = self.topics_listen_to.write().await;
        if let Some(found_topic) = topics_listen_to.get_mut(topic) {
            if let Some(current_topic) = found_topic.get_mut(&topic_id) {
                if current_topic > &mut 0u64 {
                    current_topic.sub_assign(1);
                }
            }
        }
    }

    pub(crate) async fn clients_listen_to_topic(
        &self,
        topic: &String,
        topic_id: i64,
    ) -> Vec<tokio::sync::broadcast::Sender<crate::websocket::WebSocketMessages>> {
        let mut senders = vec![];
        let clients = self.clients.read().await;

        for (_, (tx, topics, _)) in clients.iter() {
            if let Some(found_topic) = topics.get(topic) {
                if found_topic.contains(&topic_id) {
                    senders.push(tx.clone());
                }
            }
        }

        senders
    }

    pub(crate) async fn client_listen_to_topics(
        &self,
        id: &String,
        topics: Vec<(String, i64)>,
    ) -> bool {
        if let Some(client) = self.clients.read().await.get(id) {
            for (topic, id) in topics {
                if let Some(found_topic) = client.1.get(&topic) {
                    if found_topic.contains(&id) {
                        return true;
                    }
                }
            }
        }

        false
    }

    #[allow(dead_code)]
    pub(crate) async fn listeners_for_topic(&self, topics: Vec<(String, i64)>) -> bool {
        let topics_listen_to = self.topics_listen_to.read().await;
        for (topic, id) in topics {
            if let Some(found_topic) = topics_listen_to.get(&topic) {
                if let Some(current_topic) = found_topic.get(&id) {
                    if current_topic > &0u64 {
                        return true;
                    }
                }
            }
        }

        false
    }
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
            Matcher::Full("database_query_duration_seconds".to_string()),
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

#[derive(Parser, Debug)]
#[command(version, author)]
pub struct Cli {
    #[arg(global = true, short = 'u', long, help = "Database URL")]
    database_url: Option<String>,

    #[arg(global = true, short = 'c', long, help = "Config path")]
    config_path: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, PartialEq, Eq, Debug)]
pub enum Commands {
    #[command(about = "Migration related commands", display_order = 20)]
    Migrate {
        #[arg(
            global = true,
            short = 'd',
            long,
            env = "MIGRATION_DIR",
            help = "Migration script directory.
If your migrations are in their own crate,
you can provide the root of that crate.
If your migrations are in a submodule of your app,
you should provide the directory of that submodule.",
            default_value = "./migration"
        )]
        migration_dir: String,

        #[command(subcommand)]
        command: Option<sea_orm_cli::MigrateSubcommands>,
    },
    Serve {
        #[arg(long, short = 'p', help = "Port to listen on")]
        port: Option<u16>,

        #[arg(long, short = 'H', help = "Host to listen on")]
        host: Option<String>,

        #[arg(long, help = "Storage path")]
        storage_path: Option<String>,

        #[arg(long, help = "Live updates")]
        live_updates_ws: Option<bool>,
    },
    Download {
        #[arg(long, help = "Use remote schema to get tables", default_value_t = true)]
        remote_schema: bool,

        #[command(subcommand)]
        command: crate::download::DownloadSubcommand,
    },
    PrintConfig {
        #[arg(long, help = "Format to print the config", default_value = "json", value_parser = ["yml","yaml","json","toml"])]
        format: String,

        #[arg(long, help = "Show only default config", default_value_t = false)]
        show_default: bool,
    },
}

pub async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut cli_config_parameters = crate::config::CliConfigParameters::default();

    if let Some(db_url) = cli.database_url {
        cli_config_parameters.database_url = Some(db_url);
    }

    if let Some(config_path) = cli.config_path {
        cli_config_parameters.config_path = Some(config_path);
    }

    match &cli.command {
        Commands::Migrate { .. } => {}
        Commands::Download { .. } => {}
        Commands::PrintConfig { .. } => {}
        Commands::Serve {
            port,
            host,
            storage_path,
            live_updates_ws,
        } => {
            cli_config_parameters.host = host.clone();
            cli_config_parameters.port = *port;
            cli_config_parameters.storage_path = storage_path.clone();
            cli_config_parameters.live_updates_ws = *live_updates_ws;
        }
    }

    dotenvy::dotenv().ok();
    let config: Config = Config::new(Some(cli_config_parameters))?;

    setup_tracing(&config);

    match cli.command {
        Commands::Migrate {
            command,
            migration_dir,
        } => {
            let database_connection = create_db_connection(&config).await;

            match command {
                Some(MigrateSubcommands::Fresh) => Migrator::fresh(&database_connection)
                    .await
                    .unwrap_or_else(handle_error),
                Some(MigrateSubcommands::Refresh) => Migrator::refresh(&database_connection)
                    .await
                    .unwrap_or_else(handle_error),
                Some(MigrateSubcommands::Reset) => Migrator::reset(&database_connection)
                    .await
                    .unwrap_or_else(handle_error),
                Some(MigrateSubcommands::Status) => Migrator::status(&database_connection)
                    .await
                    .unwrap_or_else(handle_error),
                Some(MigrateSubcommands::Up { num }) => Migrator::up(&database_connection, num)
                    .await
                    .unwrap_or_else(handle_error),
                Some(MigrateSubcommands::Down { num }) => {
                    Migrator::down(&database_connection, Some(num))
                        .await
                        .unwrap_or_else(handle_error)
                }
                Some(MigrateSubcommands::Init) => {
                    sea_orm_cli::run_migrate_init(&migration_dir).unwrap_or_else(handle_error)
                }
                Some(MigrateSubcommands::Generate {
                    migration_name,
                    universal_time: _,
                    local_time,
                }) => {
                    sea_orm_cli::run_migrate_generate(&migration_dir, &migration_name, !local_time)
                        .unwrap_or_else(handle_error)
                }
                _ => Migrator::up(&database_connection, None)
                    .await
                    .unwrap_or_else(handle_error),
            };
        }
        Commands::Serve { .. } => {
            let database_connection = create_db_connection(&config).await;

            let result = start(database_connection, config).await;

            if let Some(err) = result.err() {
                error!("Error: {err}");
            }
        }
        Commands::Download {
            command,
            remote_schema,
        } => {
            let client = create_default_client(config.clone());

            crate::download::download_all_tables(
                command,
                &client,
                Path::new(&config.storage_path.clone()),
                &config,
                remote_schema,
            )
            .await;
        }
        Commands::PrintConfig {
            format,
            show_default,
        } => {
            let config = if show_default {
                Config::default()
            } else {
                config
            };

            match format.as_str() {
                "yml" => {
                    println!("{:}", serde_yml::to_string(&config)?);
                }
                "yaml" => {
                    println!("{:}", serde_yml::to_string(&config)?);
                }
                "json" => {
                    println!("{:}", serde_json::to_string_pretty(&config)?);
                }
                "toml" => {
                    println!("{:}", toml::to_string_pretty(&config)?)
                }
                _ => {
                    error!("Unknown format: {format}");
                    exit(1);
                }
            }
        }
    };

    Ok(())
}

fn setup_tracing(cfg: &Config) {
    let filter_directive = std::env::var("RUST_LOG").unwrap_or_else(|e| {
        if let std::env::VarError::NotUnicode(_) = e {
            eprintln!("RUST_LOG is not unicode");
        };

        const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");
        format!("{}={},axum={},spacetimedb_sdk={},", CRATE_NAME, cfg.log_level, cfg.log_level, cfg.log_level)
    });

    match cfg.log_type {
        config::LogType::Default => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::Registry::default()
                .with(stdout_layer)
                .with(tracing_subscriber::EnvFilter::new(filter_directive))
                .init()
        }
        config::LogType::Json => {
            let fmt = tracing_subscriber::fmt::format()
                .json()
                .flatten_event(true)
                .with_file(true)
                .with_line_number(true);
            let json_fields = tracing_subscriber::fmt::format::JsonFields::new();

            let stdout_layer = tracing_subscriber::fmt::layer()
                .event_format(fmt)
                .fmt_fields(json_fields);

            tracing_subscriber::Registry::default()
                .with(stdout_layer)
                .with(tracing_subscriber::EnvFilter::new(filter_directive))
                .init()
        }
        config::LogType::Pretty => {
            let fmt = tracing_subscriber::fmt::format()
                .pretty()
                .with_file(true)
                .with_line_number(true);
            let json_fields = tracing_subscriber::fmt::format::JsonFields::new();

            let stdout_layer = tracing_subscriber::fmt::layer()
                .event_format(fmt)
                .fmt_fields(json_fields);

            tracing_subscriber::Registry::default()
                .with(stdout_layer)
                .with(tracing_subscriber::EnvFilter::new(filter_directive))
                .init()
        }
    };
}

fn handle_error<E>(error: E)
where
    E: Display,
{
    eprintln!("{error}");
    exit(1);
}
