#![warn(clippy::await_holding_invalid_type)]

mod auction_listing_state;
mod buildings;
mod cargo_desc;
mod claim_tech_desc;
mod claim_tech_state;
mod claims;
mod collectible_desc;
mod config;
mod crafting;
mod crafting_recipe_desc;
mod deployable_state;
mod extraction_recipe_desc;
mod houses;
mod inventory;
mod item_list_desc;
mod items;
mod items_and_cargo;
#[cfg(feature = "bench")]
pub mod leaderboard;
#[cfg(not(feature = "bench"))]
mod leaderboard;
mod location_state;
mod locations;
mod mobile_entity_state;
mod npc_desc;
mod player_state;
mod recipes;
mod resource_desc;
mod shutdown;
#[cfg(test)]
mod shutdown_tests;
mod skill_descriptions;
mod startup_memory;
mod state_loading;
mod trading_orders;
mod traveler_task_desc;
mod traveler_task_state;
mod traveler_tasks;
mod user_state;
mod vault_state;
mod websocket;

use crate::config::{Config, TechTierResearchMap};
use crate::leaderboard::{
    EXCLUDED_USERS_FROM_LEADERBOARD, Leaderboard, RankingSystem, experience_to_level,
};
use crate::websocket::{InboundWebSocketMessages, OutboundWebSocketMessages};
use axum::extract::{
    MatchedPath, Query, Request, State,
    ws::{Message, WebSocket, WebSocketUpgrade},
};
use axum::http::{HeaderValue, Version};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::{Router, http::StatusCode, middleware, routing::get};
use clap::{Parser, Subcommand};
use entity::{experience_state, player_username_state};
use futures::{SinkExt, StreamExt, TryStreamExt};
use kanal::AsyncSender;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use metrics_process::Collector;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, ConnectOptions, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use sea_orm_cli::MigrateSubcommands;
use serde::Deserialize;
use service::sea_orm::{Database, DatabaseConnection};
use spacetimedb_sdk::Identity;
use spacetimedb_sdk::unstable::CLIENT_METRICS;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::{Display, Write};
use std::ops::{AddAssign, SubAssign};
use std::process::exit;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tower_cookies::CookieManagerLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing::error;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt;

async fn start(database_connection: DatabaseConnection, config: Config) -> anyhow::Result<()> {
    let shutdown = shutdown::Shutdown::default();
    let stop = shutdown.stop.clone();
    let after = config.shutdown.after_seconds.map(Duration::from_secs);
    let trigger = tokio::spawn(async move {
        let reason = shutdown::wait_for_trigger(after).await;
        tracing::info!(reason, "Graceful shutdown requested");
        stop.cancel();
    });
    let result = serve_with_shutdown(database_connection, config, shutdown.clone()).await;
    shutdown.stop.cancel();
    trigger.abort();
    let _ = trigger.await;
    result
}

async fn serve_with_shutdown(
    database_connection: DatabaseConnection,
    config: Config,
    shutdown: shutdown::Shutdown,
) -> anyhow::Result<()> {
    tracing::info!("Starting up server");
    let prometheus = setup_metrics_recorder();

    tokio::select! { biased;
        _ = shutdown.stop.cancelled() => return Ok(()),
        result = Migrator::up(&database_connection, None) => result?,
    }

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<OutboundWebSocketMessages>();

    let mut state = AppState::with_leaderboard_config(
        database_connection.clone(),
        tx.clone(),
        config.tech_tier_research_map.clone(),
        config.leaderboard.clone(),
    );
    state.shutdown = shutdown.clone();

    tracing::info!("App state initialized");
    tokio::select! { biased;
        _ = shutdown.stop.cancelled() => return Ok(()),
        result = state.fill_state_from_db(None) => result?,
    }
    tracing::info!("App state ready");

    let server_url = config.server_url()?;
    // Bind before spawning workers: bind failure must not leave sync running.
    let listener = tokio::net::TcpListener::bind(&server_url).await?;
    let mut broadcaster = None;

    if config.live_updates_ws {
        if config.spacetimedb.databases.is_empty() {
            tracing::warn!("You need to set spacetimedb databases");
            drop(rx);
        } else {
            tracing::info!("Staring Bitcraft websocket connection");

            broadcaster = Some(tokio::spawn(broadcast_message(state.clone(), rx)));

            let tmp_config = config.clone();

            config
                .spacetimedb
                .databases
                .iter()
                .for_each(|connection_state| {
                    state
                        .connection_state
                        .insert(connection_state.clone(), false);
                });

            websocket::start_websocket_bitcraft_logic(tmp_config, state.clone());
        }
    } else {
        drop(rx);
    }
    drop(tx);

    let app = create_app(&config, state.clone(), prometheus);

    tracing::info!("Server started on http://{}", server_url);

    let stop = shutdown.stop.clone();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
    });
    let mut server_result = None;
    tokio::select! {
        _ = shutdown.stop.cancelled() => {},
        result = &mut server => { server_result = Some(result); shutdown.stop.cancel(); },
    }
    tracing::info!(
        grace_seconds = config.shutdown.grace_seconds,
        "Stopping connections and draining accepted work"
    );
    let memory = startup_memory::StartupMemory::new();
    memory.snapshot("shutdown_requested");
    let drain = async {
        let http = async {
            if server_result.is_none() {
                server_result = Some((&mut server).await);
            }
        };
        tokio::join!(http, shutdown.drain());
        state.ranking_system.shutdown().await;
        if let Some(task) = &mut broadcaster {
            let _ = task.await;
        }
        memory.snapshot("shutdown_drained_state_retained");
        // Pool closure is part of the same grace budget, not an unbounded wait
        // after it (for example if a detached request still holds a connection).
        database_connection.clone().close().await
    };
    let drained =
        tokio::time::timeout(Duration::from_secs(config.shutdown.grace_seconds), drain).await;
    if drained.is_err() {
        tracing::error!(
            "Shutdown grace period expired; cancelling unfinished work (queued writes may be lost)"
        );
        server.abort();
        if let Some(task) = &broadcaster {
            task.abort();
        }
        shutdown.force_stop().await;
        if server_result.is_none() {
            let _ = (&mut server).await;
        }
        if let Some(task) = &mut broadcaster {
            if !task.is_finished() {
                let _ = task.await;
            }
        }
        // No further ranking writes can be accepted after tracked tasks stop.
        state.ranking_system.shutdown().await;
        return Err(anyhow::anyhow!("shutdown grace period expired"));
    }
    drained.unwrap()?;
    drop(state);
    drop(database_connection);
    memory.snapshot("shutdown_state_dropped");
    if let Some(result) = server_result {
        result??;
    }
    tracing::info!("Graceful shutdown complete");
    Ok(())
}

pub(crate) async fn create_db_connection(config: &Config) -> anyhow::Result<DatabaseConnection> {
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
        let labels = database_query_labels(&arg.statement.sql, arg.failed);

        metrics::counter!("database_query_total", labels.iter()).increment(1);
        metrics::histogram!("database_query_duration_seconds", labels.iter()).record(latency);
    });

    connection.ping().await?;

    Ok(connection)
}

fn database_query_labels(sql: &str, failed: bool) -> [metrics::Label; 3] {
    // Preserve the existing upper/lowercase-only method classification. Table
    // extraction below has always accepted mixed-case keywords independently.
    let method = match sql.split_whitespace().next() {
        Some("SELECT" | "select") => "SELECT",
        Some("UPDATE" | "update") => "UPDATE",
        Some("INSERT" | "insert") => "INSERT",
        Some("ALTER" | "alter") => "ALTER",
        Some("CREATE" | "create") => "CREATE",
        Some("DELETE" | "delete") => "DELETE",
        Some(possible_query_type) => {
            tracing::info!("Could not find type for {possible_query_type}");
            "UNKNOWN"
        }
        None => {
            tracing::info!("Could not find type for {sql}");
            "UNKNOWN"
        }
    };
    [
        metrics::Label::from_static_parts("method", method),
        metrics::Label::from_static_parts("failed", if failed { "true" } else { "false" }),
        match extract_table_name(sql) {
            // The registry must own this runtime label after the callback exits.
            Some(table) => metrics::Label::new("table", table.to_owned()),
            None => metrics::Label::from_static_parts("table", "UNKNOWN"),
        },
    ]
}

fn extract_table_name(sql: &str) -> Option<&str> {
    let mut tokens = sql.split_whitespace().peekable();
    let mut from = None;
    let mut into = None;
    let mut update = None;
    let mut table = None;
    // One pass without materializing tokens. FROM wins even if INTO/UPDATE/TABLE
    // occurred earlier; each keyword's first following token wins, as before.
    while let Some(token) = tokens.next() {
        if token.eq_ignore_ascii_case("from") {
            from = tokens.peek().copied();
            break;
        }
        // Once a higher-priority fallback exists, only look for keywords that
        // could supersede it. Large INSERT value lists then only check FROM.
        if into.is_some() {
            continue;
        }
        let candidate = if token.eq_ignore_ascii_case("into") {
            &mut into
        } else if update.is_some() {
            continue;
        } else if token.eq_ignore_ascii_case("update") {
            &mut update
        } else if table.is_none() && token.eq_ignore_ascii_case("table") {
            &mut table
        } else {
            continue;
        };
        if candidate.is_none() {
            *candidate = tokens.peek().copied();
        }
    }

    from.or(into).or(update).or(table).map(|token| {
        let token = token
            .trim_matches(|c: char| c == ';' || c == ',' || c == ')' || c == '(')
            .trim_matches(|c: char| c == '"' || c == '`' || c == '[' || c == ']');
        token.split('(').next().unwrap_or(token)
    })
}

#[cfg(test)]
mod database_metric_tests {
    use super::{database_query_labels, extract_table_name};

    // Keep the former implementation as an independent compatibility oracle:
    // these are metric-label heuristics, not an SQL parser to fix in this patch.
    fn legacy_table_name(sql: &str) -> Option<String> {
        let tokens: Vec<&str> = sql.split_whitespace().collect();
        let find_next = |keyword: &str| {
            tokens
                .iter()
                .position(|token| token.eq_ignore_ascii_case(keyword))
                .and_then(|index| tokens.get(index + 1).copied())
        };
        find_next("from")
            .or_else(|| find_next("into"))
            .or_else(|| find_next("update"))
            .or_else(|| find_next("table"))
            .map(|token| {
                let token = token
                    .trim_matches(|c: char| c == ';' || c == ',' || c == ')' || c == '(')
                    .trim_matches(|c: char| c == '"' || c == '`' || c == '[' || c == ']');
                token.split('(').next().unwrap_or(token).to_owned()
            })
    }

    #[test]
    fn table_extraction_preserves_priority_first_matches_and_trimming() {
        for (sql, expected) in [
            (
                "SELECT * FROM \"inventory\" WHERE entity_id = $1",
                Some("inventory"),
            ),
            (
                "insert into inventory(entity_id) values ($1)",
                Some("inventory"),
            ),
            (
                "UPDATE `player_state` SET time_played = $1",
                Some("player_state"),
            ),
            ("ALTER TABLE [experience_state];", Some("experience_state")),
            (
                "CREATE TABLE \"public\".\"inventory\" (id bigint)",
                Some("public\".\"inventory"),
            ),
            (
                "TABLE low UPDATE middle INTO high FROM highest",
                Some("highest"),
            ),
            ("TABLE low UPDATE middle INTO high", Some("high")),
            ("FROM first FROM second", Some("first")),
            ("INTO first INTO second FROM", Some("first")),
            ("INTO UPDATE FROM", Some("UPDATE")),
            ("SELECT * FrOm (\"inventory\"),", Some("inventory")),
            (
                "\u{2003}SELECT\n*\tFROM\u{2003}\"inventory\"",
                Some("inventory"),
            ),
            ("FROM (", Some("")),
            ("SELECT 1", None),
            ("FROM", None),
            ("", None),
        ] {
            assert_eq!(extract_table_name(sql), expected, "SQL={sql}");
            assert_eq!(
                extract_table_name(sql).map(str::to_owned),
                legacy_table_name(sql),
                "SQL={sql}"
            );
        }
    }

    #[test]
    fn iterator_parser_matches_legacy_for_keyword_combinations() {
        let vocabulary = [
            "FROM",
            "from",
            "iNtO",
            "UPDATE",
            "TaBlE",
            "\"quoted\"",
            "x(y)",
            ";",
            "FROM,",
            "(",
        ];
        for length in 0..=4 {
            for mut number in 0..vocabulary.len().pow(length) {
                let mut sql = String::new();
                for _ in 0..length {
                    sql.push_str(vocabulary[number % vocabulary.len()]);
                    sql.push_str(" \t");
                    number /= vocabulary.len();
                }
                assert_eq!(
                    extract_table_name(&sql).map(str::to_owned),
                    legacy_table_name(&sql),
                    "SQL={sql:?}"
                );
            }
        }
    }

    #[test]
    fn extracted_table_borrows_the_original_sql_storage() {
        let sql = String::from("SELECT * FROM \"inventory\" WHERE entity_id = $1");
        let table = extract_table_name(&sql).unwrap();
        assert_eq!(
            table.as_ptr(),
            sql[sql.find("inventory").unwrap()..].as_ptr()
        );
    }

    #[test]
    fn metric_labels_preserve_methods_booleans_unknowns_and_ownership() {
        for method in ["SELECT", "UPDATE", "INSERT", "ALTER", "CREATE", "DELETE"] {
            for token in [method.to_owned(), method.to_lowercase()] {
                for failed in [false, true] {
                    let sql = format!("  {token} FROM \"inventory\"  ");
                    let labels = database_query_labels(&sql, failed);
                    assert_eq!(
                        labels
                            .iter()
                            .map(|label| (label.key(), label.value()))
                            .collect::<Vec<_>>(),
                        [
                            ("method", method),
                            ("failed", if failed { "true" } else { "false" }),
                            ("table", "inventory")
                        ]
                    );
                }
            }
        }
        for sql in ["", "  \t", "WITH x AS (SELECT 1)", "SeLeCt 1", "BEGIN"] {
            let labels = database_query_labels(sql, false);
            assert_eq!(labels[0].value(), "UNKNOWN");
            assert_eq!(labels[2].value(), "UNKNOWN");
        }
        let labels = {
            let sql = String::from("SeLeCt * FROM inventory");
            database_query_labels(&sql, true)
        };
        assert_eq!(labels[0].value(), "UNKNOWN");
        assert_eq!(labels[1].value(), "true");
        assert_eq!(
            labels[2].value(),
            "inventory",
            "runtime label outlives SQL buffer"
        );
    }
}

#[derive(Deserialize)]
struct QueryWebsocketOptions {
    encoding: Option<WebsocketEncoding>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    version: Version,
    State(state): State<AppState>,
    Query(websocket_options): Query<QueryWebsocketOptions>,
) -> impl IntoResponse {
    tracing::debug!("Websocket upgraded with version: {version:?}");
    let clients = state.shutdown.clients.clone();
    ws.on_upgrade(move |socket| clients.track_future(websocket(socket, state, websocket_options)))
}

#[allow(dead_code)]
struct ServerInstance {
    // cached_resources
    connected_clients: HashMap<String, (UnboundedSender<OutboundWebSocketMessages>, Vec<String>)>,
    topics_listen_to: HashMap<String, Vec<String>>,
}

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending chat messages).
async fn websocket(stream: WebSocket, state: AppState, websocket_options: QueryWebsocketOptions) {
    use sea_orm::{ColumnTrait, QueryFilter};
    if state.shutdown.stop.is_cancelled() {
        return;
    }
    let stop = state.shutdown.stop.clone();

    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let id = nanoid::nanoid!();

    // Reserve one slot for the message currently being written to the socket.
    let (tx, rx) =
        kanal::bounded_async::<OutboundWebSocketMessages>(WEBSOCKET_CLIENT_MAX_IN_FLIGHT - 1);

    state
        .clients_state
        .add_client(
            id.clone(),
            tx.clone(),
            websocket_options
                .encoding
                .unwrap_or(WebsocketEncoding::Json),
        )
        .await;

    // Now send the "joined" message to all subscribers.
    let msg = format!("{id} joined.");
    let _ = tx.send(OutboundWebSocketMessages::Message(msg)).await;

    let internal_id = id.clone();
    let inner_state = state.clone();
    let writer_stop = stop.clone();
    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    let mut send_task = tokio::spawn(async move {
        loop {
            let msg = tokio::select! { biased;
                _ = writer_stop.cancelled() => {
                    let _ = tokio::time::timeout(Duration::from_secs(1), sender.send(Message::Close(None))).await;
                    break;
                },
                msg = rx.recv() => match msg { Ok(msg) => msg, Err(_) => break },
            };
            let encoding = inner_state
                .clients_state
                .get_encode_format_for_client(&internal_id)
                .await;

            if encoding.is_none() {
                tracing::warn!("Could not find encoding for {internal_id}")
            };

            let encoding = encoding.unwrap();

            let message = if encoding == WebsocketEncoding::Json {
                Message::Text(serde_json::to_string(&msg).unwrap().into())
            } else if encoding == WebsocketEncoding::MessagePack {
                Message::Binary(axum_codec::Codec(&msg).to_msgpack().unwrap().into())
            } else {
                tracing::warn!("Unsupported encoding {encoding}");
                continue;
            };

            let send_result =
                tokio::time::timeout(WEBSOCKET_CLIENT_WRITE_TIMEOUT, sender.send(message)).await;

            match send_result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!("Error sending message to client: {error}");
                    break;
                }
                Err(_) => {
                    tracing::warn!(
                        client = internal_id,
                        "Disconnecting client after websocket write timeout"
                    );
                    break;
                }
            }
        }
    });

    let inner_id = id.clone();
    let inner_state = state.clone();
    let recv_state = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            match serde_json::from_str::<InboundWebSocketMessages>(&text) {
                Ok(message) => match message {
                    InboundWebSocketMessages::Subscribe { topics } => {
                        for topic in topics {
                            let possible_topic = topic.split_once(".");

                            if let Some((topic, id)) = possible_topic {
                                let id = id.parse::<i64>().unwrap();

                                inner_state
                                    .clients_state
                                    .add_topic_to_client(&inner_id, &topic.to_string(), Some(id))
                                    .await;

                                #[allow(clippy::single_match)]
                                match topic {
                                    "player_username" => {
                                        let player_username = player_username_state::Entity::find()
                                            .filter(player_username_state::Column::EntityId.eq(id))
                                            .one(&recv_state.conn)
                                            .await
                                            .expect("Cannot find player_username");

                                        if let Some(player_username) = player_username {
                                            let _ = inner_state.tx.send(
                                                OutboundWebSocketMessages::PlayerUsername {
                                                    username: player_username.username,
                                                    entity_id: id,
                                                },
                                            );
                                        } else {
                                            tracing::debug!(
                                                topic = topic,
                                                id = id,
                                                "Could not find player_username"
                                            );
                                        }
                                    }
                                    _ => {}
                                }
                            } else {
                                inner_state
                                    .clients_state
                                    .add_topic_to_client(&inner_id, &topic.to_string(), None)
                                    .await;
                            }
                        }
                    }
                    InboundWebSocketMessages::Unsubscribe { topic } => {
                        let possible_topic = topic.split_once(".");

                        if let Some((topic, id)) = possible_topic {
                            let id = id.parse::<i64>().unwrap();
                            inner_state
                                .clients_state
                                .remove_topic_from_client(&inner_id, &topic.to_string(), Some(id))
                                .await;
                        } else {
                            inner_state
                                .clients_state
                                .remove_topic_from_client(&inner_id, &topic.to_string(), None)
                                .await;
                        }
                    }
                    InboundWebSocketMessages::ListSubscribedTopics => {
                        let topics = inner_state
                            .clients_state
                            .get_topics_for_client(&inner_id)
                            .await;

                        if topics.is_none() {
                            continue;
                        }

                        let topics = topics.unwrap();
                        let _ = tx
                            .send(OutboundWebSocketMessages::SubscribedTopics(topics))
                            .await;
                    }
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
        _ = stop.cancelled() => {
            recv_task.abort();
            // Allow a close frame, but do not let a slow client hold shutdown.
            if tokio::time::timeout(Duration::from_secs(2), &mut send_task).await.is_err() {
                send_task.abort();
            }
        },
    };
    // Await aborted tasks so their socket halves and app-state clones are gone.
    if !send_task.is_finished() {
        let _ = send_task.await;
    }
    if !recv_task.is_finished() {
        let _ = recv_task.await;
    }

    // Remove username from map so new clients can take it again.
    state.clients_state.remove_client(&id).await;
}

async fn broadcast_message(state: AppState, mut rx: UnboundedReceiver<OutboundWebSocketMessages>) {
    let mut buffer = Vec::with_capacity(500);

    loop {
        buffer.shrink_to(500);
        let count = tokio::select! { biased;
            _ = state.shutdown.broadcast_stop.cancelled(), if !rx.is_closed() => { rx.close(); continue; },
            count = rx.recv_many(&mut buffer, 500) => count,
        };
        if count == 0 {
            break;
        }

        if !state.clients_state.clients.read().await.is_empty() {
            for message in buffer.drain(..) {
                if let Some(topics) = message.topics() {
                    state.clients_state.broadcast_topics(topics, &message).await;
                }
            }
        } else {
            buffer.clear();
        }
    }
}

// This bounds application-owned messages; OS/socket buffers are managed separately.
const WEBSOCKET_CLIENT_MAX_IN_FLIGHT: usize = 2_000;
const WEBSOCKET_CLIENT_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn broadcast_to_clients(
    senders: impl IntoIterator<Item = AsyncSender<OutboundWebSocketMessages>>,
    message: &OutboundWebSocketMessages,
) {
    for sender in senders {
        match sender.try_send(message.clone()) {
            Ok(true) => {}
            Ok(false) => {
                // A missing update would leave the client out of sync. Close instead,
                // dropping its backlog and waking its receive loop. A stalled socket
                // write is bounded by WEBSOCKET_CLIENT_WRITE_TIMEOUT.
                let _ = sender.close();
                metrics::counter!("websocket_slow_client_disconnects_total").increment(1);
                tracing::warn!("Disconnecting websocket client with a full outgoing queue");
            }
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod websocket_broadcast_tests {
    use super::*;

    #[tokio::test]
    async fn routing_preserves_overlap_order_global_topics_and_unsubscribe() {
        let clients = ClientsState::new();
        let (tx, rx) = kanal::bounded_async(10);
        let id = "overlap".to_string();
        clients
            .add_client(id.clone(), tx, WebsocketEncoding::Json)
            .await;
        for topic in ["experience:Mining", "experience"] {
            clients
                .add_topic_to_client(&id, &topic.to_string(), Some(7))
                .await;
        }
        for xp in [100, 200] {
            let message = OutboundWebSocketMessages::Experience {
                experience: xp,
                level: 1,
                rank: 2,
                skill_name: "Mining".into(),
                user_id: 7,
            };
            clients
                .broadcast_topics(message.topics().unwrap(), &message)
                .await;
        }
        for expected in [100, 100, 200, 200] {
            assert!(
                matches!(rx.try_recv().unwrap().unwrap(), OutboundWebSocketMessages::Experience { experience, .. } if experience == expected)
            );
        }
        clients
            .remove_topic_from_client(&id, &"experience".into(), Some(7))
            .await;
        let message = OutboundWebSocketMessages::Experience {
            experience: 300,
            level: 1,
            rank: 2,
            skill_name: "Mining".into(),
            user_id: 7,
        };
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        assert!(matches!(
            rx.try_recv().unwrap().unwrap(),
            OutboundWebSocketMessages::Experience {
                experience: 300,
                ..
            }
        ));
        assert!(rx.try_recv().unwrap().is_none());

        let order = entity::auction_listing_state::AuctionListingState {
            entity_id: 77,
            item_id: 9,
            owner_entity_id: 0,
            claim_entity_id: 0,
            item_type: 0,
            price_threshold: 0,
            quantity: 1,
            stored_coins: 0,
            region: 1,
            timestamp: entity::shared::timestamp::Timestamp {
                __timestamp_micros_since_unix_epoch__: chrono::DateTime::from_timestamp_micros(0)
                    .unwrap(),
            },
        };
        // A keyed subscription to this name must not panic or match None.
        clients
            .add_topic_to_client(&id, &"insert_sell_order".into(), Some(999))
            .await;
        let message = OutboundWebSocketMessages::InsertSellOrder(order);
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        assert!(rx.try_recv().unwrap().is_none());
        clients
            .add_topic_to_client(&id, &"insert_sell_order".into(), None)
            .await;
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        assert!(matches!(
            rx.try_recv().unwrap().unwrap(),
            OutboundWebSocketMessages::InsertSellOrder(_)
        ));
        assert!(rx.try_recv().unwrap().is_none());
        assert_eq!(
            axum_codec::Codec(&message).to_msgpack().unwrap(),
            axum_codec::Codec(message.clone()).to_msgpack().unwrap()
        );
    }

    #[tokio::test]
    async fn routed_slow_client_does_not_block_healthy_clients() {
        let clients = ClientsState::new();
        let (slow_tx, slow_rx) = kanal::bounded_async(1);
        let (healthy_tx, healthy_rx) = kanal::bounded_async(3);
        for (id, tx) in [("slow", slow_tx), ("healthy", healthy_tx)] {
            clients
                .add_client(id.into(), tx, WebsocketEncoding::Json)
                .await;
            clients
                .add_topic_to_client(&id.into(), &"total_experience".into(), Some(7))
                .await;
        }
        for xp in [1, 2] {
            let message = OutboundWebSocketMessages::TotalExperience {
                user_id: 7,
                experience: xp,
                experience_per_hour: 0,
                rank: 1,
            };
            clients
                .broadcast_topics(message.topics().unwrap(), &message)
                .await;
        }
        assert!(slow_rx.is_closed());
        assert_eq!(slow_rx.len(), 0);
        for xp in [1, 2] {
            assert!(
                matches!(healthy_rx.try_recv().unwrap().unwrap(), OutboundWebSocketMessages::TotalExperience { experience, .. } if experience == xp)
            );
        }
    }

    #[tokio::test]
    async fn full_client_is_closed_without_blocking_healthy_clients() {
        let (slow_tx, slow_rx) = kanal::bounded_async(WEBSOCKET_CLIENT_MAX_IN_FLIGHT - 1);
        let (healthy_tx, healthy_rx) = kanal::bounded_async(WEBSOCKET_CLIENT_MAX_IN_FLIGHT - 1);
        let message = OutboundWebSocketMessages::Message("update".to_string());

        // One socket write plus this queue is at most 2,000 in-flight messages.
        for _ in 0..WEBSOCKET_CLIENT_MAX_IN_FLIGHT - 1 {
            assert!(slow_tx.try_send(message.clone()).unwrap());
        }
        assert_eq!(slow_tx.len(), 1_999);

        broadcast_to_clients(vec![slow_tx.clone(), healthy_tx], &message);

        assert!(slow_rx.is_closed());
        assert_eq!(slow_tx.len(), 0, "disconnect must release the backlog");
        assert!(matches!(
            healthy_rx.recv().await.unwrap(),
            OutboundWebSocketMessages::Message(value) if value == "update"
        ));
    }

    #[tokio::test]
    async fn closed_client_does_not_prevent_delivery_and_order_is_preserved() {
        let (closed_tx, closed_rx) = kanal::bounded_async(1);
        closed_rx.close().unwrap();
        let (healthy_tx, healthy_rx) = kanal::bounded_async(2);

        for value in ["first", "second"] {
            broadcast_to_clients(
                vec![closed_tx.clone(), healthy_tx.clone()],
                &OutboundWebSocketMessages::Message(value.to_string()),
            );
        }

        for expected in ["first", "second"] {
            assert!(matches!(
                healthy_rx.recv().await.unwrap(),
                OutboundWebSocketMessages::Message(value) if value == expected
            ));
        }
        assert!(!healthy_rx.is_closed());
    }
}

pub(crate) type AppRouter = Router<AppState>;

fn create_app(config: &Config, state: AppState, prometheus: PrometheusHandle) -> Router {
    let desc_router = Router::new()
        .route(
            "/buildings/{id}",
            axum_codec::routing::get(buildings::find_claim_description).into(),
        )
        .route(
            "/buildings",
            axum_codec::routing::get(buildings::find_building_descriptions).into(),
        );

    let collector = Collector::default();
    // Call `describe()` method to register help string.
    collector.describe();

    Router::new()
        .route("/websocket", any(websocket_handler))
        // .route(
        //     "/locations",
        //     axum_codec::routing::get(locations::list_locations).into(),
        // )
        .route("/items", axum_codec::routing::get(items::list_items).into())
        .route(
            "/items/world",
            axum_codec::routing::get(items::list_world_items).into(),
        )
        .merge(player_state::get_routes())
        .merge(claims::get_routes())
        .merge(buildings::get_routes())
        .merge(inventory::get_routes())
        .merge(recipes::get_routes())
        .merge(items_and_cargo::get_routes())
        .merge(extraction_recipe_desc::get_routes())
        .merge(leaderboard::get_routes())
        .merge(trading_orders::get_routes())
        .merge(traveler_tasks::get_routes())
        .merge(auction_listing_state::get_routes())
        .merge(houses::get_routes())
        .merge(crafting::get_routes())
        .nest("/desc", desc_router)
        .route(
            "/metrics",
            get(|State(app_state): State<AppState>| async move {
                let encoder = prometheus::TextEncoder::new();

                metrics::gauge!("database_connection_pool_idle")
                    .set(app_state.conn.get_postgres_connection_pool().num_idle() as f64);
                metrics::gauge!("database_connection_pool_total")
                    .set(app_state.conn.get_postgres_connection_pool().size() as f64);

                record_app_state_metrics(&app_state);

                let metric_families = app_state.metrics_registry.gather();
                let prometheus_body = encoder.encode_to_string(&metric_families);
                let metrics_body = prometheus.render();
                collector.collect();

                let mut leaderboard_buckets = String::new();
                let _ = writeln!(
                    leaderboard_buckets,
                    "# HELP leaderboard_bucket_count Bucket distribution for leaderboards"
                );
                let _ = writeln!(
                    leaderboard_buckets,
                    "# TYPE leaderboard_bucket_count gauge"
                );

                let append_bucket_metrics = |output: &mut String,
                                             name: &str,
                                             board: &Leaderboard| {
                    for bucket in board.bucket_distribution() {
                        let _ = writeln!(
                            output,
                            "leaderboard_bucket_count{{leaderboard=\"{name}\",bucket=\"{}\",min_xp=\"{}\",max_xp=\"{}\"}} {}",
                            bucket.bucket,
                            bucket.min_xp,
                            bucket.max_xp,
                            bucket.count
                        );
                    }
                };

                append_bucket_metrics(
                    &mut leaderboard_buckets,
                    "global_experience",
                    &app_state.ranking_system.global_leaderboard,
                );
                append_bucket_metrics(
                    &mut leaderboard_buckets,
                    "level",
                    &app_state.ranking_system.level_leaderboard,
                );
                append_bucket_metrics(
                    &mut leaderboard_buckets,
                    "xp_per_hour",
                    &app_state.ranking_system.xp_per_hour,
                );
                append_bucket_metrics(
                    &mut leaderboard_buckets,
                    "time_played",
                    &app_state.ranking_system.time_played,
                );
                append_bucket_metrics(
                    &mut leaderboard_buckets,
                    "time_signed_in",
                    &app_state.ranking_system.time_signed_in,
                );

                for (skill_id, board) in app_state.ranking_system.skill_boards() {
                    append_bucket_metrics(
                        &mut leaderboard_buckets,
                        &format!("skill_{skill_id}"),
                        &board,
                    );
                }

                if let Err(err) = prometheus_body {
                    error!("Error: {:?}", err);

                    Err((StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error"))
                } else {
                    let prometheus_body = prometheus_body.unwrap();
                    Ok(format!(
                        "{metrics_body}\n{prometheus_body}\n{leaderboard_buckets}"
                    ))
                }
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
        .layer(CompressionLayer::new())
        .route_layer(middleware::from_fn(track_metrics))
        .with_state(state)
}

fn record_app_state_metrics(app_state: &AppState) {
    app_state.ranking_system.record_memory_metrics();
    metrics::gauge!("app_state_cache_size", &[("cache", "mobile_entity_state")])
        .set(app_state.mobile_entity_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "player_state")])
        .set(app_state.player_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "claim_member_state")])
        .set(app_state.claim_member_state.len() as f64);
    metrics::gauge!(
        "app_state_cache_size",
        &[("cache", "claim_member_state_total")]
    )
    .set(
        app_state
            .claim_member_state
            .iter()
            .map(|entry| entry.value().len())
            .sum::<usize>() as f64,
    );
    metrics::gauge!(
        "app_state_cache_size",
        &[("cache", "player_to_claim_id_cache")]
    )
    .set(app_state.player_to_claim_id_cache.len() as f64);
    metrics::gauge!(
        "app_state_cache_size",
        &[("cache", "player_to_claim_id_cache_total")]
    )
    .set(
        app_state
            .player_to_claim_id_cache
            .iter()
            .map(|entry| entry.value().len())
            .sum::<usize>() as f64,
    );
    metrics::gauge!("app_state_cache_size", &[("cache", "claim_local_state")])
        .set(app_state.claim_local_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "claim_state")])
        .set(app_state.claim_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "claim_tile_state")])
        .set(app_state.claim_tile_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "player_action_state")])
        .set(app_state.player_action_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "crafting_recipe_desc")])
        .set(app_state.crafting_recipe_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "claim_tech_desc")])
        .set(app_state.claim_tech_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "item_tags")])
        .set(app_state.item_tags.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "item_tiers")])
        .set(app_state.item_tiers.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "item_desc")])
        .set(app_state.item_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "item_list_desc")])
        .set(app_state.item_list_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "skill_desc")])
        .set(app_state.skill_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "cargo_desc")])
        .set(app_state.cargo_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "cargo_tags")])
        .set(app_state.cargo_tags.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "cargo_tiers")])
        .set(app_state.cargo_tiers.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "building_desc")])
        .set(app_state.building_desc.len() as f64);
    metrics::gauge!(
        "app_state_cache_size",
        &[("cache", "building_nickname_state")]
    )
    .set(app_state.building_nickname_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "action_state")])
        .set(app_state.action_state.len() as f64);
    let action_maps = app_state
        .action_state
        .iter()
        .map(|entry| Arc::clone(entry.value()))
        .collect::<Vec<_>>();
    metrics::gauge!("app_state_cache_size", &[("cache", "action_state_total")]).set(
        action_maps
            .iter()
            .map(|actions| actions.len())
            .sum::<usize>() as f64,
    );
    metrics::gauge!("app_state_cache_size", &[("cache", "location_state")])
        .set(app_state.location_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "connected_user_map")])
        .set(app_state.connected_user_map.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "traveler_task_desc")])
        .set(app_state.traveler_task_desc.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "user_state")])
        .set(app_state.user_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "trade_order_state")])
        .set(app_state.trade_order_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "deployable_state")])
        .set(app_state.deployable_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "buy_order_state")])
        .set(app_state.buy_order_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "sell_order_state")])
        .set(app_state.sell_order_state.len() as f64);
    metrics::gauge!("app_state_cache_size", &[("cache", "npc_desc")])
        .set(app_state.npc_desc.len() as f64);
}

#[derive(Clone)]
struct AppState {
    shutdown: shutdown::Shutdown,
    tech_tier_research_map: TechTierResearchMap,
    conn: DatabaseConnection,
    tx: UnboundedSender<OutboundWebSocketMessages>,
    connection_state: Arc<dashmap::DashMap<String, bool>>,
    clients_state: Arc<ClientsState>,
    mobile_entity_state: Arc<dashmap::DashMap<u64, entity::mobile_entity_state::Model>>,
    player_state: Arc<dashmap::DashMap<i64, entity::player_state::Model>>,
    claim_member_state: Arc<claims::member_cache::MembersByClaim>,
    player_to_claim_id_cache: Arc<claims::member_cache::ClaimsByPlayer>,
    claim_local_state: Arc<dashmap::DashMap<u64, entity::claim_local_state::Model>>,
    claim_state: Arc<dashmap::DashMap<i64, entity::claim_state::Model>>,
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
    action_state:
        Arc<dashmap::DashMap<u64, Arc<dashmap::DashMap<u64, entity::action_state::Model>>>>,
    location_state: Arc<dashmap::DashMap<i64, entity::location::Model>>,
    // inventory_state: Arc<dashmap::DashMap<i64, ::entity::inventory::Model>>,
    #[allow(dead_code)]
    connected_user_map: Arc<dashmap::DashMap<String, i64>>,
    traveler_task_desc: Arc<dashmap::DashMap<i32, entity::traveler_task_desc::Model>>,
    user_state: Arc<dashmap::DashMap<Identity, u64>>,
    trade_order_state: Arc<dashmap::DashMap<i64, entity::trade_order::Model>>,
    deployable_state: Arc<dashmap::DashMap<i64, entity::deployable_state::Model>>,
    buy_order_state: Arc<dashmap::DashMap<i64, entity::auction_listing_state::AuctionListingState>>,
    sell_order_state:
        Arc<dashmap::DashMap<i64, entity::auction_listing_state::AuctionListingState>>,
    npc_desc: Arc<dashmap::DashMap<i32, entity::npc_desc::Model>>,
    metrics_registry: prometheus::Registry,
    ranking_system: Arc<RankingSystem>,
}

impl AppState {
    #[cfg(test)]
    fn new(
        conn: DatabaseConnection,
        tx: UnboundedSender<OutboundWebSocketMessages>,
        tech_tier_research_map: TechTierResearchMap,
    ) -> Self {
        Self::with_leaderboard_config(conn, tx, tech_tier_research_map, Default::default())
    }

    fn with_leaderboard_config(
        conn: DatabaseConnection,
        tx: UnboundedSender<OutboundWebSocketMessages>,
        tech_tier_research_map: TechTierResearchMap,
        leaderboard_config: config::LeaderboardConfig,
    ) -> Self {
        let metrics_registry = prometheus::Registry::new();

        metrics_registry
            .register(Box::new(&*CLIENT_METRICS))
            .unwrap();

        Self {
            shutdown: shutdown::Shutdown::default(),
            tech_tier_research_map,
            conn,
            tx,
            metrics_registry,
            connection_state: Arc::new(dashmap::DashMap::new()),
            player_state: Arc::new(dashmap::DashMap::new()),
            clients_state: Arc::new(ClientsState::new()),
            mobile_entity_state: Arc::new(dashmap::DashMap::new()),
            claim_member_state: Arc::new(dashmap::DashMap::new()),
            player_to_claim_id_cache: Arc::new(dashmap::DashMap::new()),
            claim_local_state: Arc::new(dashmap::DashMap::new()),
            claim_tile_state: Arc::new(dashmap::DashMap::new()),
            claim_state: Arc::new(dashmap::DashMap::new()),
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
            // inventory_state: Arc::new(dashmap::DashMap::new()),
            connected_user_map: Arc::new(dashmap::DashMap::new()),
            traveler_task_desc: Arc::new(dashmap::DashMap::new()),
            trade_order_state: Arc::new(dashmap::DashMap::new()),
            buy_order_state: Arc::new(dashmap::DashMap::new()),
            sell_order_state: Arc::new(dashmap::DashMap::new()),
            user_state: Arc::new(dashmap::DashMap::new()),
            deployable_state: Arc::new(dashmap::DashMap::new()),
            npc_desc: Arc::new(dashmap::DashMap::new()),
            ranking_system: Arc::new(RankingSystem::with_bucket_config(leaderboard_config)),
        }
    }

    async fn fill_state_from_db(
        &self,
        memory: Option<&startup_memory::StartupMemory>,
    ) -> anyhow::Result<()> {
        if let Some(memory) = memory {
            memory.snapshot("fill_state_begin");
        }
        use state_loading::stream_into;
        // Count before any ranking ingestion. This is a capacity hint, not a
        // membership snapshot: experience-only/new players can still grow it.
        let player_count = ::entity::player_state::Entity::find()
            .count(&self.conn)
            .await?;
        self.ranking_system
            .reserve_player_capacity(usize::try_from(player_count)?);
        if let Some(memory) = memory {
            memory.snapshot("ranking_capacity_reserved");
            eprintln!("startup_ranking_capacity player_hint={player_count}");
        }
        tokio::try_join!(
            stream_into(::entity::item_desc::Entity::find(), &self.conn, |value| {
                self.item_desc.insert(value.id, value);
            }),
            stream_into(::entity::cargo_desc::Entity::find(), &self.conn, |value| {
                self.cargo_desc.insert(value.id, value);
            }),
            stream_into(
                ::entity::claim_local_state::Entity::find(),
                &self.conn,
                |value| {
                    self.claim_local_state.insert(value.entity_id as u64, value);
                }
            ),
            stream_into(::entity::claim_state::Entity::find(), &self.conn, |value| {
                self.claim_state.insert(value.entity_id, value);
            }),
            stream_into(
                ::entity::claim_member_state::Entity::find(),
                &self.conn,
                |value| self.add_claim_member(value)
            ),
            stream_into(
                ::entity::claim_tech_desc::Entity::find(),
                &self.conn,
                |value| {
                    self.claim_tech_desc.insert(value.id, value);
                }
            ),
            stream_into(::entity::skill_desc::Entity::find(), &self.conn, |value| {
                self.skill_desc.insert(value.id, value);
            }),
            stream_into(
                ::entity::building_desc::Entity::find(),
                &self.conn,
                |value| {
                    self.building_desc.insert(value.id, value);
                }
            ),
            stream_into(
                ::entity::building_nickname_state::Entity::find(),
                &self.conn,
                |value| {
                    self.building_nickname_state.insert(value.entity_id, value);
                }
            ),
            stream_into(
                ::entity::crafting_recipe::Entity::find(),
                &self.conn,
                |value| {
                    self.crafting_recipe_desc.insert(value.id, value);
                }
            ),
            stream_into(
                ::entity::item_list_desc::Entity::find(),
                &self.conn,
                |value| {
                    self.item_list_desc.insert(value.id, value);
                }
            ),
            stream_into(
                ::entity::player_state::Entity::find(),
                &self.conn,
                |value| {
                    self.ranking_system
                        .time_played
                        .update(value.entity_id, i64::from(value.time_played));
                    self.ranking_system
                        .time_signed_in
                        .update(value.entity_id, i64::from(value.time_signed_in));
                    self.player_state.insert(value.entity_id, value);
                }
            ),
        )?;

        if let Some(memory) = memory {
            memory.snapshot("parallel_cache_loads_complete");
            eprintln!(
                "startup_cache_counts items={} cargo={} claims={} skills={} buildings={} building_nicknames={} recipes={} item_lists={} players={}",
                self.item_desc.len(),
                self.cargo_desc.len(),
                self.claim_local_state.len(),
                self.skill_desc.len(),
                self.building_desc.len(),
                self.building_nickname_state.len(),
                self.crafting_recipe_desc.len(),
                self.item_list_desc.len(),
                self.player_state.len()
            );
        }

        let ranked_skills = self
            .skill_desc
            .iter()
            .filter(|skill| skill.skill_category != 0)
            .map(|skill| skill.id)
            .collect::<Vec<_>>();
        for skill_id in ranked_skills {
            self.ranking_system.ensure_skill_leaderboard(skill_id);
        }

        if let Some(memory) = memory {
            memory.snapshot("experience_query_begin");
        }
        let mut items = experience_state::Entity::find()
            .filter(
                experience_state::Column::EntityId
                    .is_not_in(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
            )
            .order_by_asc(experience_state::Column::EntityId)
            .order_by_asc(experience_state::Column::SkillId)
            .stream(&self.conn)
            .await?;

        if let Some(memory) = memory {
            memory.snapshot("experience_stream_opened");
        }

        let update_totals = |player_id, total_xp, total_level, profession_xp, adventure_xp| {
            self.ranking_system
                .global_leaderboard
                .update(player_id, total_xp);
            self.ranking_system
                .level_leaderboard
                .update(player_id, total_level);
            self.ranking_system
                .profession_leaderboard
                .update(player_id, profession_xp);
            self.ranking_system
                .adventure_leaderboard
                .update(player_id, adventure_xp);

            let xp_per_hour = self
                .player_state
                .get(&player_id)
                .filter(|player| player.time_signed_in >= 3600)
                .map_or(0, |player| {
                    total_xp / (i64::from(player.time_signed_in) / 3600)
                });
            self.ranking_system
                .xp_per_hour
                .update(player_id, xp_per_hour);
        };

        let mut current_player_id = None;
        let mut total_xp = 0i64;
        let mut total_level = 0i64;
        let mut profession_xp = 0i64;
        let mut adventure_xp = 0i64;

        let mut experience_rows = 0usize;
        while let Some(item) = items.try_next().await? {
            experience_rows += 1;
            let Some(skill_category) = self
                .skill_desc
                .get(&i64::from(item.skill_id))
                .map(|skill| skill.skill_category)
            else {
                tracing::error!(skill_id = item.skill_id, "This should never happen");
                continue;
            };
            if skill_category == 0 {
                continue;
            }

            if current_player_id.is_some_and(|player_id| player_id != item.entity_id) {
                update_totals(
                    current_player_id.unwrap(),
                    total_xp,
                    total_level,
                    profession_xp,
                    adventure_xp,
                );
                total_xp = 0;
                total_level = 0;
                profession_xp = 0;
                adventure_xp = 0;
            }
            current_player_id = Some(item.entity_id);

            let experience = i64::from(item.experience);
            total_xp.add_assign(experience);
            total_level.add_assign(i64::from(experience_to_level(experience)));
            match skill_category {
                1 => profession_xp.add_assign(experience),
                2 => adventure_xp.add_assign(experience),
                _ => {}
            }

            if let Some(leaderboard) = self
                .ranking_system
                .skill_leaderboard(i64::from(item.skill_id))
            {
                leaderboard.update(item.entity_id, experience);
            }
        }

        if let Some(player_id) = current_player_id {
            update_totals(
                player_id,
                total_xp,
                total_level,
                profession_xp,
                adventure_xp,
            );
        }
        drop(items);
        if let Some(memory) = memory {
            memory.snapshot("experience_stream_processed_and_dropped");
            eprintln!("startup_experience rows={experience_rows}");
        }
        Ok(())
    }

    fn add_claim_member(&self, claim_member_state: entity::claim_member_state::Model) {
        claims::member_cache::insert(
            &self.claim_member_state,
            &self.player_to_claim_id_cache,
            claim_member_state,
        );
    }

    fn remove_claim_member(&self, claim_member_state: entity::claim_member_state::Model) {
        claims::member_cache::remove(
            &self.claim_member_state,
            &self.player_to_claim_id_cache,
            &claim_member_state,
        );
    }
}

#[derive(Deserialize, Copy, Clone, Eq, PartialEq)]
enum WebsocketEncoding {
    Json,
    MessagePack,
}

impl Display for WebsocketEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebsocketEncoding::Json => {
                write!(f, "json")
            }
            WebsocketEncoding::MessagePack => {
                write!(f, "messagepack")
            }
        }
    }
}

type WebsocketClient = (
    AsyncSender<crate::websocket::OutboundWebSocketMessages>,
    HashMap<String, HashSet<i64>>,
    WebsocketEncoding,
    HashSet<String>,
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
        tx: AsyncSender<OutboundWebSocketMessages>,
        encoding: WebsocketEncoding,
    ) {
        self.clients
            .write()
            .await
            .insert(id, (tx, HashMap::new(), encoding, HashSet::new()));
        metrics::gauge!("websocket_clients_connected_total").increment(1);
    }

    pub(crate) async fn remove_client(&self, id: &String) {
        let current_topics = self.clients.read().await.get(id).unwrap().1.clone();
        for (topic, topic_ids) in current_topics {
            for topic_id in topic_ids {
                self.remove_topic_from_client(id, &topic, Some(topic_id))
                    .await;
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
                            .map(|id| format!("{topic}.{id:?}"))
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<String>>(),
            );
        }

        None
    }

    pub(crate) async fn add_topic_to_client(
        &self,
        id: &String,
        topic: &String,
        topic_id: Option<i64>,
    ) {
        if self
            .client_listen_to_topics(id, vec![(topic.clone(), topic_id)])
            .await
        {
            return;
        }

        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(id) {
            if let Some(topic_id) = topic_id {
                if let Some(topics) = client.1.get_mut(topic) {
                    topics.insert(topic_id);
                } else {
                    client.1.insert(topic.clone(), HashSet::from([topic_id]));
                }
            } else {
                client.3.insert(topic.clone());
            }
        }

        if topic_id.is_none() {
            return;
        }

        let mut topics_listen_to = self.topics_listen_to.write().await;
        if let Some(found_topic) = topics_listen_to.get_mut(topic) {
            if let Some(current_topic) = found_topic.get_mut(&topic_id.unwrap()) {
                current_topic.add_assign(1);
            } else {
                found_topic.insert(topic_id.unwrap(), 1);
            }
        } else {
            topics_listen_to.insert(topic.clone(), HashMap::from([(topic_id.unwrap(), 1)]));
        }
    }

    pub(crate) async fn remove_topic_from_client(
        &self,
        id: &String,
        topic: &String,
        topic_id: Option<i64>,
    ) {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(id) {
            if topic_id.is_none() && client.3.contains(topic) {
                client.3.remove(topic);
            } else if let Some(topics) = client.1.get_mut(topic) {
                topics.remove(&topic_id.unwrap());
            }
        }

        if topic_id.is_none() {
            return;
        }

        let mut topics_listen_to = self.topics_listen_to.write().await;
        if let Some(found_topic) = topics_listen_to.get_mut(topic)
            && let Some(current_topic) = found_topic.get_mut(&topic_id.unwrap())
            && current_topic > &mut 0u64
        {
            current_topic.sub_assign(1);
        }
    }

    async fn broadcast_topics(
        &self,
        topics: crate::websocket::BroadcastTopics,
        message: &OutboundWebSocketMessages,
    ) {
        // No awaits while holding this guard. Preserve topic order and existing
        // duplicate deliveries for clients subscribed to multiple matching topics.
        let clients = self.clients.read().await;
        for (topic, topic_id) in topics {
            let senders = clients
                .values()
                .filter_map(|(tx, subscriptions, _, global)| {
                    let matches = match topic_id {
                        None => global.contains(topic.as_ref()),
                        Some(id) => subscriptions
                            .get(topic.as_ref())
                            .is_some_and(|ids| ids.contains(&id)),
                    };
                    matches.then(|| tx.clone())
                });
            broadcast_to_clients(senders, message);
        }
    }

    pub(crate) async fn client_listen_to_topics(
        &self,
        id: &String,
        topics: Vec<(String, Option<i64>)>,
    ) -> bool {
        if let Some(client) = self.clients.read().await.get(id) {
            for (topic, id) in topics {
                if id.is_none() {
                    return client.3.contains(&topic);
                }

                if let Some(found_topic) = client.1.get(&topic) {
                    return found_topic.contains(&id.unwrap());
                }
            }
        }

        false
    }

    #[allow(dead_code)]
    pub(crate) async fn listeners_for_topic(&self, topics: Vec<(String, i64)>) -> bool {
        let topics_listen_to = self.topics_listen_to.read().await;
        for (topic, id) in topics {
            if let Some(found_topic) = topics_listen_to.get(&topic)
                && let Some(current_topic) = found_topic.get(&id)
                && current_topic > &0u64
            {
                return true;
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

    metrics::describe_counter!(
        "worker_queue_received_total",
        "This shows how many events a worker got"
    );

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
    #[command(
        about = "Measure database state initialization, then exit (no migrations or services)"
    )]
    ProfileInit {
        #[arg(
            long,
            default_value_t = 5,
            help = "Seconds to observe RSS after loading; not a ranking-publication barrier"
        )]
        settle_seconds: u64,
    },
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
        #[arg(
            long,
            help = "Request graceful shutdown after N seconds of serve startup (360 = six minutes)"
        )]
        shutdown_after_seconds: Option<u64>,
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
        Commands::ProfileInit { .. } => {}
        Commands::Migrate { .. } => {}
        Commands::PrintConfig { .. } => {}
        Commands::Serve {
            port,
            host,
            storage_path,
            live_updates_ws,
            shutdown_after_seconds,
        } => {
            cli_config_parameters.host = host.clone();
            cli_config_parameters.port = *port;
            cli_config_parameters.storage_path = storage_path.clone();
            cli_config_parameters.live_updates_ws = *live_updates_ws;
            cli_config_parameters.shutdown_after_seconds = *shutdown_after_seconds;
        }
    }

    dotenvy::dotenv().ok();
    let config: Config = Config::new(Some(cli_config_parameters))?;

    setup_tracing(&config);

    match cli.command {
        Commands::ProfileInit { settle_seconds } => {
            let memory = startup_memory::StartupMemory::new();
            memory.snapshot("before_database_connection");
            let database_connection = create_db_connection(&config).await?;
            memory.snapshot("database_connected");
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<OutboundWebSocketMessages>();
            let state = AppState::with_leaderboard_config(
                database_connection,
                tx,
                config.tech_tier_research_map.clone(),
                config.leaderboard.clone(),
            );
            memory.snapshot("empty_app_state");
            state.fill_state_from_db(Some(&memory)).await?;
            memory.snapshot("fill_state_returned");
            for _ in 0..settle_seconds {
                tokio::time::sleep(Duration::from_secs(1)).await;
                memory.snapshot("settling");
            }
            memory.snapshot("final_state_retained");
            state.ranking_system.print_startup_memory_stats();
            // Keep the caches alive through the final measurement, but do not
            // start HTTP, SpacetimeDB, broadcaster, or metrics background tasks.
            drop(state);
        }
        Commands::Migrate {
            command,
            migration_dir,
        } => {
            let database_connection = create_db_connection(&config).await?;

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
            let database_connection = create_db_connection(&config).await?;

            let result = start(database_connection, config).await;

            if let Some(err) = result.err() {
                error!("Error: {err}");
            }
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

        let log_level = &cfg.log_level;

        const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");
        println!("RUST_LOG={log_level} CRATE_NAME={CRATE_NAME}");

        format!("{CRATE_NAME}={log_level},axum={log_level},spacetimedb_sdk={log_level},tungstenite=warn",)
        // format!("{CRATE_NAME}={log_level},axum={log_level}",)
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
