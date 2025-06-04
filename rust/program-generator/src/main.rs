mod config;

use config::*;
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Compression, DbContext, Error, Table, TableWithPrimaryKey, credentials};
use std::process::exit;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tokio::time::{Duration, sleep};
use serde_json::Value;
use time::OffsetDateTime;
use reqwest::ClientBuilder;
use reqwest_websocket::{Message, RequestBuilderExt, WebSocket};
use axum::http::HeaderMap;
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use log::{debug, error, warn};
use futures::{SinkExt, TryStreamExt};

use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt;

use std::fmt::Display;

use std::fs::File;
use std::io::prelude::*;



#[tokio::main]
async fn main()  {
    let cli_config_parameters = CliConfigParameters::default();
    let config: Config = Config::new(Some(cli_config_parameters)).unwrap();
    setup_tracing(&config);
    start_websocket_bitcraft_logic_old(config).await;
}


async fn create_websocket_connection(config: &Config) -> anyhow::Result<WebSocket> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", config.spacetimedb.password).parse()?,
    );
    headers.insert(SEC_WEBSOCKET_PROTOCOL, "v1.json.spacetimedb".parse()?);
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==".parse()?);
    headers.insert("Sec-WebSocket-Version", "13".parse()?);
    headers.insert(
        reqwest::header::USER_AGENT,
        format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION")).parse()?,
    );

    let response = ClientBuilder::default()
        .timeout(Duration::from_millis(5000))
        .connect_timeout(Duration::from_millis(2500))
        .build()?
        .get(format!(
            "{}/{}/{}/{}",
            config.weboosocket_url(),
            "database",
            "spacetime-control",
            "subscribe"
        ))
        .headers(headers)
        .upgrade()
        // .web_socket_config(tungstenite::protocol::WebSocketConfig::default()
        //    .max_frame_size(Some(1024 * 1024 * 1500))
        //    .max_message_size(Some(1024 * 1024 * 1500))
        // )
        .web_socket_config(tungstenite::protocol::WebSocketConfig {
            max_frame_size: Some(1024 * 1024 * 1500),
            max_message_size: Some(1024 * 1024 * 1500),
            ..Default::default()
        })
        .protocols(vec!["v1.json.spacetimedb"])
        .send()
        .await?;
    Ok(response.into_websocket().await?)
}

async fn start_websocket_bitcraft_logic_old(
    config: Config
) {
    
    let handle = tokio::spawn(async move {
        let reconnect_wait_time_sec = 5;
        let mut retry_count = 1_u32;
        let max_retry_count = 10;
        let backoff_factor = 2;

        let tables_to_subscribe = vec![
            "program",
        ];

        let select_querys = tables_to_subscribe
            .iter()
            .map(|table_name| format!("SELECT * FROM {};", table_name))
            .collect::<Vec<String>>();


        loop {
            let now = Instant::now();
            let websocket = create_websocket_connection(&config).await;

            if websocket.is_err()
                && websocket_retry_helper(
                reconnect_wait_time_sec,
                &mut retry_count,
                max_retry_count,
                backoff_factor,
                now,
                false,
            )
                .await
            {
                tracing::error!(
                    "Could not connect to bitcraft server with following error {websocket:?}"
                );
                break;
            } else if websocket.is_err() {
                tracing::error!(
                    "Could not connect to bitcraft server with following error {websocket:?}"
                );
                continue;
            }

            tracing::info!("Websocket connection established");

            let mut websocket = websocket.unwrap();

            websocket
                .send(Message::Text(
                    serde_json::json!({
                        "Subscribe": {
                            "query_strings": select_querys,
                            "request_id": 1,
                        },
                    })
                        .to_string(),
                ))
                .await
                .unwrap();

            tracing::info!("Websocket send Subscribe query");

            while let result = websocket.try_next().await {
                if result.is_err() {
                    let error = result.unwrap_err();
                    tracing::error!("WebSocket message could not be decoded {error:?}");
                    break;
                }

                if let Ok(Some(message)) = result {
                    if let Message::Text(text) = message {
                        let message: Result<WebSocketMessage, serde_json::Error> =
                            serde_json::from_str(&text);

                        if message.is_err() {
                            //info!("Text: {:?}", text);
                            error!("Error: {:?}, text: {text}", message.err());
                            continue;
                        }

                        let message = message.unwrap();

                        match &message {
                            WebSocketMessage::TransactionUpdate(transaction_update) => {
                            
                            }
                            WebSocketMessage::InitialSubscription(subscription_update) => {
                                let inserts = &subscription_update.database_update.tables[0].updates[0].inserts;
                                for insert in inserts {
                                    let value: Value = serde_json::from_str(insert).unwrap();
                                    warn!("{}",value.as_object().unwrap().get("hash").unwrap().to_string());
                                    if value.as_object().unwrap().get("hash").unwrap().to_string() == config.program_hash {
                                        let mut file = File::create("program.wasm").unwrap();
                                        let text = value.as_object().unwrap().get("bytes").unwrap().as_str().unwrap().to_string();
                                        file.write_all(&hex::decode(text.to_string()).unwrap()).unwrap();
                                    }
                                }
                            }
                            WebSocketMessage::IdentityToken(identity_token) => {
                                debug!("Received identity token: {identity_token:?}");
                            }
                        }
                    } else if let Message::Ping(_) = message {
                    } else {
                        warn!("Message: {:?}", message);
                    }
                }
            }

            if websocket_retry_helper(
                reconnect_wait_time_sec,
                &mut retry_count,
                max_retry_count,
                backoff_factor,
                now,
                true,
            )
                .await
            {
                break;
            }
        }
        
    });
    let _ = handle.await.unwrap();
}
fn setup_tracing(cfg: &Config) {
    let filter_directive = std::env::var("RUST_LOG").unwrap_or_else(|e| {
        if let std::env::VarError::NotUnicode(_) = e {
            eprintln!("RUST_LOG is not unicode");
        };

        const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");
        format!("{}={},axum={}", CRATE_NAME, cfg.log_level, cfg.log_level)
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

async fn websocket_retry_helper(
    reconnect_wait_time: u32,
    retry_count: &mut u32,
    max_retry_count: u32,
    backoff_factor: u32,
    now: Instant,
    was_connected: bool,
) -> bool {
    if now.elapsed() > Duration::from_secs(5) && was_connected {
        *retry_count = 1;
    }

    let wait_time = reconnect_wait_time * retry_count.pow(backoff_factor);

    tracing::debug!("Wait time {wait_time}");

    tokio::time::sleep(Duration::from_secs(wait_time as u64)).await;
    *retry_count += 1;
    if *retry_count > max_retry_count {
        return true;
    }

    tracing::info!("Reconnecting to websocket {retry_count} {max_retry_count}");
    false
}

enum SpacetimeUpdateMessages<T> {
    Insert { new: T },
    Update { old: T, new: T },
    Remove { delete: T },
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct InternalTransactionUpdate {
    pub(crate) user: Option<i64>,
    pub(crate) tables: Vec<TempTable>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) enum WebSocketMessage {
    IdentityToken(IdentityToken),
    TransactionUpdate(TransactionUpdate),
    InitialSubscription(InitialSubscription),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct InitialSubscription {
    pub(crate) database_update: DatabaseUpdate,
    pub(crate) request_id: u64,
    pub(crate) total_host_execution_duration: TotalHostExecutionDuration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct DatabaseUpdate {
    pub(crate) tables: Vec<TempTable>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct IdentityToken {
    pub(crate) identity: Identity,
    pub(crate) token: Box<str>,
    pub(crate) connection_id: ConnectionId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Identity {
    pub(crate) __identity__: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Address {
    pub(crate) __address__: u128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectionId {
    __connection_id__: u128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TransactionUpdate {
    pub(crate) status: Status,
    pub(crate) timestamp: Timestamp,
    pub(crate) caller_identity: Identity,
    pub(crate) caller_connection_id: ConnectionId,
    pub(crate) reducer_call: ReducerCall,
    pub(crate) energy_quanta_used: EnergyQuantaUsed,
    pub(crate) total_host_execution_duration: TotalHostExecutionDuration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TotalHostExecutionDuration {
    pub(crate) __time_duration_micros__: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Status {
    #[serde(rename = "Committed")]
    pub(crate) committed: Option<Committed>,
    #[serde(rename = "Failed")]
    pub(crate) failed: Option<Box<str>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Committed {
    pub(crate) tables: Vec<TempTable>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct QueryUpdate {
    pub(crate) deletes: Vec<Box<str>>,
    pub(crate) inserts: Vec<Box<str>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TempTable {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) num_rows: u64,
    pub(crate) updates: Vec<QueryUpdate>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TableWithOriginalEventTransactionUpdate {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) deletes: Vec<Box<str>>,
    pub(crate) inserts: Vec<Box<str>>,
    pub(crate) original_event: TransactionUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Timestamp {
    #[serde(with = "time::serde::timestamp::microseconds")]
    pub(crate) __timestamp_micros_since_unix_epoch__: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ReducerCall {
    pub(crate) reducer_name: Box<str>,
    pub(crate) reducer_id: u64,
    pub(crate) args: serde_json::Value,
    pub(crate) request_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct EnergyQuantaUsed {
    pub(crate) quanta: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Event {
    pub(crate) timestamp: u64,
    pub(crate) status: String,
    pub(crate) caller_identity: String,
    pub(crate) function_call: FunctionCall,
    pub(crate) energy_quanta_used: u64,
    pub(crate) message: String,
    pub(crate) caller_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct FunctionCall {
    pub(crate) reducer: String,
    pub(crate) args: String,
    pub(crate) request_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct TableRowOperation {
    pub(crate) row: Value,
    pub(crate) op: String,
}
