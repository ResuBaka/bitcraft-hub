use crate::config::Config;
use crate::{
    buildings, claim_tech_state, claims, collectible_desc, deployable_state, inventory,
    leaderboard, player_state, vault_state,
};
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use axum::http::HeaderMap;
use base64::Engine;
use futures::{SinkExt, TryStreamExt};
use log::{debug, error, info};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub fn start_websocket_bitcraft_logic(
    websocket_url: String,
    websocket_password: String,
    websocket_username: String,
    database_name: String,
    tmp_config: Config,
) {
    tokio::spawn(async move {
        let config = tmp_config.clone();
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
        headers.insert(
            reqwest::header::USER_AGENT,
            format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION"))
                .parse()
                .unwrap(),
        );

        let response = Client::default()
            .get(format!(
                "{}/{}/{}",
                websocket_url, "database/subscribe", database_name
            ))
            .headers(headers)
            .upgrade()
            .web_socket_config(tungstenite::protocol::WebSocketConfig {
                max_frame_size: Some(1024 * 1024 * 150),
                max_message_size: Some(1024 * 1024 * 150),
                ..Default::default()
            })
            .protocols(vec!["v1.text.spacetimedb"])
            .send()
            .await
            .unwrap();
        let mut websocket = response.into_websocket().await.unwrap();

        let tables_to_subscribe = vec![
            "PlayerState",
            "PlayerUsernameState",
            "BuildingState",
            "VaultState",
            "ExperienceState",
            "InventoryState",
            "ClaimTechState",
            "ClaimDescriptionState",
            "DeployableState",
            "CollectibleDesc",
        ];

        let select_querys = tables_to_subscribe
            .iter()
            .map(|table_name| format!("SELECT * FROM {};", table_name))
            .collect::<Vec<String>>();

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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let tmp_config = config.clone();
        let _ = tokio::spawn(async move {
            let db = crate::create_importer_default_db_connection(tmp_config.clone()).await;
            loop {
                let mut evenets = Vec::with_capacity(1000);
                let mut tables: HashMap<String, Vec<Table>> = HashMap::new();
                let db = db.clone();

                let count = rx.recv_many(&mut evenets, 1000).await;

                for event in evenets.iter() {
                    match event {
                        WebSocketMessage::TransactionUpdate(transaction_update) => {
                            if transaction_update.status.committed.tables.len() == 0 {
                                continue;
                            }

                            for table in transaction_update.status.committed.tables.iter() {
                                if let Some(table_vec) =
                                    tables.get_mut(&table.table_name.as_ref().to_string())
                                {
                                    table_vec.push(table.clone());
                                } else {
                                    tables.insert(
                                        table.table_name.clone().as_ref().to_string(),
                                        vec![table.clone()],
                                    );
                                }
                            }
                        }
                        WebSocketMessage::InitialSubscription(subscription_update) => {
                            if subscription_update.database_update.tables.len() == 0 {
                                continue;
                            }

                            for table in subscription_update.database_update.tables.iter() {
                                if table.table_name.as_ref() == "PlayerUsernameState" {
                                    let result =
                                        player_state::handle_initial_subscription_player_username_state(&db, table)
                                            .await;

                                    if result.is_err() {
                                        error!(
                                            "PlayerUsernameState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }
                                if table.table_name.as_ref() == "PlayerState" {
                                    let result =
                                        player_state::handle_initial_subscription_player_state(
                                            &db, table,
                                        )
                                        .await;

                                    if result.is_err() {
                                        error!(
                                            "PlayerState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }
                                if table.table_name.as_ref() == "ExperienceState" {
                                    info!("ExperienceState initial subscription");
                                    let result =
                                        leaderboard::handle_initial_subscription(&db, table).await;

                                    if result.is_err() {
                                        error!(
                                            "ExperienceState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }
                                if table.table_name.as_ref() == "BuildingState" {
                                    let result =
                                        buildings::handle_initial_subscription(&db, table).await;

                                    if result.is_err() {
                                        error!(
                                            "BuildingState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }
                                if table.table_name.as_ref() == "InventoryState" {
                                    let result =
                                        inventory::handle_initial_subscription(&db, table).await;

                                    if result.is_err() {
                                        error!(
                                            "InventoryState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "ClaimTechState" {
                                    let result =
                                        claim_tech_state::handle_initial_subscription(&db, table)
                                            .await;

                                    if result.is_err() {
                                        error!(
                                            "ClaimTechState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "ClaimDescriptionState" {
                                    let result =
                                        claims::handle_initial_subscription(&db, table).await;

                                    if result.is_err() {
                                        error!(
                                                "ClaimDescriptionState initial subscription failed: {:?}",
                                                result.err()
                                            );
                                    }
                                }

                                if table.table_name.as_ref() == "DeployableState" {
                                    let result =
                                        deployable_state::handle_initial_subscription(&db, table)
                                            .await;

                                    if result.is_err() {
                                        error!(
                                            "DeployableState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "VaultState" {
                                    let result =
                                        vault_state::handle_initial_subscription(&db, table).await;

                                    if result.is_err() {
                                        error!(
                                            "DeployableState initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "CollectibleDesc" {
                                    let result =
                                        collectible_desc::handle_initial_subscription(&db, table)
                                            .await;

                                    if result.is_err() {
                                        error!(
                                            "CollectibleDesc initial subscription failed: {:?}",
                                            result.err()
                                        );
                                    }
                                }
                            }
                        }
                        WebSocketMessage::IdentityToken(identity_token) => {
                            println!("IdentityToken: {identity_token:?}");
                        }
                    }
                }

                for (table_name, table) in tables.iter() {
                    debug!("Received table: {table_name} -> {:?}", table.len());

                    if table_name == "PlayerUsernameState" {
                        let result = player_state::handle_transaction_update_player_username_state(
                            &db, table,
                        )
                        .await;

                        if result.is_err() {
                            error!(
                                "PlayerUsernameState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }
                    if table_name == "PlayerState" {
                        let result =
                            player_state::handle_transaction_update_player_state(&db, table).await;

                        if result.is_err() {
                            error!("PlayerState transaction update failed: {:?}", result.err());
                        }
                    }

                    if table_name == "ExperienceState" {
                        let result = leaderboard::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "ExperienceState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "InventoryState" {
                        let result = inventory::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "InventoryState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "BuildingState" {
                        let result = buildings::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "BuildingState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "ClaimTechState" {
                        let result = claim_tech_state::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "ClaimTechState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "ClaimDescriptionState" {
                        let result = claims::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "ClaimDescriptionState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "DeployableState" {
                        let result = deployable_state::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!(
                                "DeployableState transaction update failed: {:?}",
                                result.err()
                            );
                        }
                    }

                    if table_name == "VaultState" {
                        let result = vault_state::handle_transaction_update(&db, table).await;

                        if result.is_err() {
                            error!("VaultState transaction update failed: {:?}", result.err());
                        }
                    }
                }

                debug!("Received {count} events");
                evenets.clear();
                tokio::time::sleep(Duration::from_millis(100)).await;
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
                        debug!("Received transaction update: {transaction_update:?}");
                    }
                    WebSocketMessage::InitialSubscription(subscription_update) => {
                        tx.send(message.clone()).unwrap();
                        debug!("Received subscription update: {subscription_update:?}");
                    }
                    WebSocketMessage::IdentityToken(identity_token) => {
                        info!("Received identity token: {identity_token:?}");
                    }
                }
            } else {
                println!("No message {:?}", message);
            }
        }
    });
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
    pub(crate) total_host_execution_duration_micros: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct DatabaseUpdate {
    pub(crate) tables: Vec<Table>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct IdentityToken {
    pub(crate) identity: Identity,
    pub(crate) token: Box<str>,
    pub(crate) address: Address,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Identity {
    pub(crate) __identity_bytes: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Address {
    pub(crate) __address_bytes: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct TransactionUpdate {
    pub(crate) status: Status,
    pub(crate) timestamp: Timestamp,
    pub(crate) caller_identity: Identity,
    pub(crate) caller_address: Address,
    pub(crate) reducer_call: ReducerCall,
    pub(crate) energy_quanta_used: EnergyQuantaUsed,
    pub(crate) host_execution_duration_micros: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Status {
    #[serde(rename = "Committed")]
    pub(crate) committed: Committed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Committed {
    pub(crate) tables: Vec<Table>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Table {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) deletes: Vec<TableText>,
    pub(crate) inserts: Vec<TableText>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TableText {
    #[serde(rename = "Text")]
    pub(crate) text: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Timestamp {
    pub(crate) microseconds: u64,
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
