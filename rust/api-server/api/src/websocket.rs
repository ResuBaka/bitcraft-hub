use crate::config::Config;
use crate::{
    AppState, buildings, cargo_desc, claim_tech_state, claims, collectible_desc, deployable_state,
    inventory, items, leaderboard, player_state, skill_descriptions, vault_state,
};
use ::entity::raw_event_data::Model as RawEventData;
use ::entity::user_state;
use axum::http::HeaderMap;
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use base64::Engine;
#[allow(unused_imports)]
use entity::{raw_event_data, skill_desc};
use futures::{SinkExt, TryStreamExt};
use log::{debug, error, info};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt, WebSocket};
use sea_orm::{EntityTrait, IntoActiveModel, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tracing::warn;

struct WebSocketAppState {
    user_map: HashMap<String, i64>,
}

pub fn start_websocket_bitcraft_logic(
    config: Config,
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
) {
    tokio::spawn(async move {
        let reconnect_wait_time_sec = 5;
        let mut retry_count = 1_u32;
        let max_retry_count = 10;
        let backoff_factor = 2;

        let app_state = WebSocketAppState {
            user_map: HashMap::new(),
        };

        let tables_to_subscribe = vec![
            "user_state",
            "mobile_entity_state",
            "claim_tile_state",
            "combat_action_desc",
            "item_desc",
            "cargo_desc",
            "player_action_state",
            "crafting_recipe_desc",
            "action_state",
            "player_state",
            "player_username_state",
            "building_desc",
            "building_state",
            "vault_state",
            "experience_state",
            "inventory_state",
            "claim_tech_state",
            "claim_description_state",
            "deployable_state",
            "collectible_desc",
        ];

        let select_querys = tables_to_subscribe
            .iter()
            .map(|table_name| format!("SELECT * FROM {};", table_name))
            .collect::<Vec<String>>();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let tmp_config = config.clone();
        start_websocket_message_thread(broadcast_tx, global_app_state, app_state, rx, tmp_config);

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
                break;
            } else if websocket.is_err() {
                continue;
            }

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

            while let Ok(Some(message)) = websocket.try_next().await {
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
                            metrics::gauge!(
                                "websocket_message_inflight_gauge",
                                &[("type", "TransactionUpdate"),]
                            )
                            .increment(1);
                            tx.send(message.clone()).unwrap();
                            debug!("Received transaction update: {transaction_update:?}");
                        }
                        WebSocketMessage::InitialSubscription(subscription_update) => {
                            metrics::gauge!(
                                "websocket_message_inflight_gauge",
                                &[("type", "InitialSubscription"),]
                            )
                            .increment(1);
                            tx.send(message.clone()).unwrap();
                            debug!("Received subscription update: {subscription_update:?}");
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

fn start_websocket_message_thread(
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
    mut app_state: WebSocketAppState,
    mut rx: UnboundedReceiver<WebSocketMessage>,
    tmp_config: Config,
) {
    tokio::spawn(async move {
        let db = crate::create_importer_default_db_connection(tmp_config.clone()).await;

        let skill_id_to_skill_name = skill_desc::Entity::find()
            .select_only()
            .column(skill_desc::Column::Id)
            .column(skill_desc::Column::Name)
            .into_tuple::<(i64, String)>()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .collect::<HashMap<i64, String>>();

        loop {
            let mut evenets = Vec::with_capacity(1000);
            let mut tables: HashMap<String, Vec<TableWithOriginalEventTransactionUpdate>> =
                HashMap::new();
            let db = db.clone();

            let count = rx.recv_many(&mut evenets, 1000).await;
            let mut raw_events_data = vec![];

            for event in evenets.iter() {
                if let WebSocketMessage::TransactionUpdate(transaction_update) = event {
                    let mut compressor = async_compression::tokio::write::ZstdEncoder::new(vec![]);
                    let _ = compressor
                        .write_all(
                            serde_json::to_string(&transaction_update)
                                .unwrap()
                                .as_bytes(),
                        )
                        .await;
                    compressor.flush().await.unwrap();
                    compressor.shutdown().await.unwrap();

                    let user_id = transaction_update.caller_identity.__identity__.clone();

                    let user_id = app_state.user_map.get(&user_id.to_string());

                    raw_events_data.push(
                        RawEventData {
                            timestamp: transaction_update.timestamp.microseconds,
                            request_id: transaction_update.reducer_call.request_id as i64,
                            reducer_name: transaction_update
                                .reducer_call
                                .reducer_name
                                .clone()
                                .parse()
                                .unwrap(),
                            reducer_id: transaction_update.reducer_call.reducer_id as i64,
                            event_data: compressor.into_inner(),
                            user_id: user_id.cloned(),
                        }
                        .into_active_model(),
                    );
                }
            }

            //raw_event_data::Entity::insert_many(raw_events_data)
            //    .exec(&db)
            //    .await
            //    .unwrap();

            for event in evenets.iter() {
                match event {
                    WebSocketMessage::TransactionUpdate(transaction_update) => {
                        metrics::counter!(
                            "websocket.message.count",
                            &[("type", "TransactionUpdate"),]
                        )
                        .increment(1);

                        if transaction_update.status.committed.tables.is_empty() {
                            continue;
                        }

                        for table in transaction_update.status.committed.tables.iter() {
                            metrics::counter!(
                                "websocket_message_table_count",
                                &[
                                    ("type", "TransactionUpdate".to_string()),
                                    ("table", format!("{}", table.table_name)),
                                ]
                            )
                            .increment(1);

                            if let Some(table_vec) = tables.get_mut(&table.table_name.to_string()) {
                                //TODO this probebly has to be rewriten
                                table.updates.iter().for_each(|updates| {
                                    table_vec.push(TableWithOriginalEventTransactionUpdate {
                                        table_id: table.table_id,
                                        table_name: table.table_name.clone(),
                                        deletes: updates.deletes.clone(),
                                        inserts: updates.inserts.clone(),
                                        original_event: transaction_update.clone(),
                                    });
                                })
                            } else {
                                tables.insert(
                                    table.table_name.clone().as_ref().to_string(),
                                    table
                                        .updates
                                        .iter()
                                        .map(|updates| TableWithOriginalEventTransactionUpdate {
                                            table_id: table.table_id,
                                            table_name: table.table_name.clone(),
                                            deletes: updates.deletes.clone(),
                                            inserts: updates.inserts.clone(),
                                            original_event: transaction_update.clone(),
                                        })
                                        .collect::<Vec<TableWithOriginalEventTransactionUpdate>>(),
                                );
                            }
                        }

                        metrics::gauge!(
                            "websocket_message_inflight_gauge",
                            &[("type", "TransactionUpdate"),]
                        )
                        .decrement(1);
                    }
                    WebSocketMessage::InitialSubscription(subscription_update) => {
                        metrics::counter!(
                            "websocket.message.count",
                            &[("type", "InitialSubscription"),]
                        )
                        .increment(1);

                        if subscription_update.database_update.tables.is_empty() {
                            continue;
                        }

                        for table in subscription_update.database_update.tables.iter() {
                            metrics::counter!(
                                "websocket_message_table_count",
                                &[
                                    ("type", "InitialSubscription".to_string()),
                                    ("table", format!("{}", table.table_name)),
                                ]
                            )
                            .increment(1);

                            let start = std::time::Instant::now();

                            if table.table_name.as_ref() == "user_state" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let user_state: user_state::Model =
                                            match serde_json::from_str(row) {
                                                Ok(user_state) => user_state,
                                                Err(error) => {
                                                    error!(
                                                        "InitialSubscription Insert user_state Error: {:?} -> {:?}",
                                                        error, row
                                                    );
                                                    continue;
                                                }
                                            };

                                        app_state.user_map.insert(
                                            user_state.identity.__identity__,
                                            user_state.entity_id,
                                        );
                                    }
                                }
                            }
                            if table.table_name.as_ref() == "player_username_state" {
                                let result =
                                    player_state::handle_initial_subscription_player_username_state(&db, table)
                                        .await;

                                if result.is_err() {
                                    error!(
                                        "player_username_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }
                            if table.table_name.as_ref() == "player_state" {
                                let result =
                                    player_state::handle_initial_subscription_player_state(
                                        &db, table,
                                    )
                                    .await;

                                if result.is_err() {
                                    error!(
                                        "player_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }
                            if table.table_name.as_ref() == "experience_state" {
                                info!("experience_state initial subscription");
                                let result =
                                    leaderboard::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "experience_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }
                            if table.table_name.as_ref() == "building_state" {
                                let result =
                                    buildings::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "building_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }
                            if table.table_name.as_ref() == "building_desc" {
                                let result = buildings::handle_initial_subscription_desc(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "building_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }
                            if table.table_name.as_ref() == "inventory_state" {
                                let result =
                                    inventory::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "inventory_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "item_desc" {
                                let result = items::handle_initial_subscription_item_desc(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "item_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "cargo_desc" {
                                let result = cargo_desc::handle_initial_subscription_cargo_desc(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "cargo_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "claim_tech_state" {
                                let result = claim_tech_state::handle_initial_subscription(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "claim_tech_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "skill_desc" {
                                let result = skill_descriptions::handle_initial_subscription(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "skill_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "claim_tech_desc" {
                                let result = crate::claim_tech_desc::handle_initial_subscription(
                                    &global_app_state,
                                    table,
                                )
                                .await;

                                if result.is_err() {
                                    error!(
                                        "claim_tech_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "claim_description_state" {
                                let result = claims::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "claim_description_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "deployable_state" {
                                let result =
                                    deployable_state::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "deployable_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "vault_state" {
                                let result =
                                    vault_state::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "vault_state initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "collectible_desc" {
                                let result =
                                    collectible_desc::handle_initial_subscription(&db, table).await;

                                if result.is_err() {
                                    error!(
                                        "collectible_desc initial subscription failed: {:?}",
                                        result.err()
                                    );
                                }
                            }

                            if table.table_name.as_ref() == "mobile_entity_state" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let mobile_entity_state: entity::mobile_entity_state::Model =
                                            match serde_json::from_str(row) {
                                                Ok(mobile_entity_state) => mobile_entity_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}", error, row);
                                                    continue;
                                                }
                                            };

                                        global_app_state.mobile_entity_state.insert(
                                            mobile_entity_state.entity_id,
                                            mobile_entity_state.clone(),
                                        );

                                        broadcast_tx
                                            .send(WebSocketMessages::MobileEntityState(
                                                mobile_entity_state,
                                            ))
                                            .unwrap();
                                    }
                                }
                            }

                            if table.table_name.as_ref() == "claim_tile_state" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let claim_tile_state: entity::claim_tile_state::Model =
                                            match serde_json::from_str(row) {
                                                Ok(claim_tile_state) => claim_tile_state,
                                                Err(error) => {
                                                    error!(
                                                        "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
                                                        error, row
                                                    );
                                                    continue;
                                                }
                                            };

                                        global_app_state.claim_tile_state.insert(
                                            claim_tile_state.entity_id,
                                            claim_tile_state.clone(),
                                        );
                                    }
                                }
                            }

                            if table.table_name.as_ref() == "crafting_recipe_desc" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let crafting_recipe_desc: entity::crafting_recipe::Model =
                                            match serde_json::from_str(row) {
                                                Ok(crafting_recipe_desc) => crafting_recipe_desc,
                                                Err(error) => {
                                                    error!(
                                                        "InitialSubscription Insert crafting_recipe_desc Error: {:?} -> {:?}",
                                                        error, row
                                                    );
                                                    continue;
                                                }
                                            };

                                        global_app_state.crafting_recipe_desc.insert(
                                            crafting_recipe_desc.id,
                                            crafting_recipe_desc.clone(),
                                        );
                                    }
                                }
                            }

                            if table.table_name.as_ref() == "player_action_state" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let player_action_state: entity::player_action_state::Model =
                                            match serde_json::from_str(row) {
                                                Ok(player_action_state) => player_action_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert player_action_state Error: {:?} -> {:?}", error, row);
                                                    continue;
                                                }
                                            };

                                        global_app_state.player_action_state.insert(
                                            player_action_state.entity_id,
                                            player_action_state.clone(),
                                        );
                                    }
                                }
                            }

                            if table.table_name.as_ref() == "action_state" {
                                for update in table.updates.iter() {
                                    for row in update.inserts.iter() {
                                        let action_state: entity::action_state::Model =
                                            match serde_json::from_str(row) {
                                                Ok(action_state) => action_state,
                                                Err(error) => {
                                                    error!(
                                                        "InitialSubscription Insert action_state Error: {:?} -> {:?}",
                                                        error, row
                                                    );
                                                    continue;
                                                }
                                            };

                                        if let Some(action_states) = global_app_state
                                            .action_state
                                            .get_mut(&action_state.owner_entity_id)
                                        {
                                            action_states.insert(
                                                action_state.entity_id,
                                                action_state.clone(),
                                            );
                                        } else {
                                            let action_states = dashmap::DashMap::new();
                                            action_states.insert(
                                                action_state.entity_id,
                                                action_state.clone(),
                                            );
                                            global_app_state.action_state.insert(
                                                action_state.owner_entity_id,
                                                action_states,
                                            );
                                        }
                                    }
                                }
                            }

                            metrics::histogram!(
                                "bitraft_event_handler_initial_subscription_duration_seconds",
                                &[("table", table.table_name.as_ref().to_string())]
                            )
                            .record(start.elapsed().as_secs_f64());
                        }

                        metrics::gauge!(
                            "websocket_message_inflight_gauge",
                            &[("type", "InitialSubscription"),]
                        )
                        .decrement(1);
                    }
                    WebSocketMessage::IdentityToken(identity_token) => {
                        println!("IdentityToken: {identity_token:?}");
                    }
                }
            }

            for (table_name, table) in tables.iter() {
                debug!("Received table: {table_name} -> {:?}", table.len());
                let start = std::time::Instant::now();

                if table_name == "user_state" {
                    for row in table.iter() {
                        if row.inserts.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<user_state::Model>(&row.inserts[0]) {
                            Ok(user_state) => {
                                app_state
                                    .user_map
                                    .insert(user_state.identity.__identity__, user_state.entity_id);
                            }
                            Err(error) => {
                                error!("InitialSubscription Insert UserState Error: {:?}", error);
                            }
                        }
                    }
                }

                if table_name == "player_username_state" {
                    let result =
                        player_state::handle_transaction_update_player_username_state(&db, table)
                            .await;

                    if result.is_err() {
                        error!(
                            "player_username_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }
                if table_name == "player_state" {
                    let result = player_state::handle_transaction_update_player_state(
                        &db,
                        table,
                        broadcast_tx.clone(),
                    )
                    .await;

                    if result.is_err() {
                        error!("player_state transaction update failed: {:?}", result.err());
                    }
                }

                if table_name == "experience_state" {
                    let result = leaderboard::handle_transaction_update(
                        &db,
                        table,
                        &skill_id_to_skill_name,
                        broadcast_tx.clone(),
                    )
                    .await;

                    if result.is_err() {
                        error!(
                            "experience_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "inventory_state" {
                    let result = inventory::handle_transaction_update(&db, table).await;

                    if result.is_err() {
                        error!(
                            "inventory_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "building_state" {
                    let result = buildings::handle_transaction_update(&db, table).await;

                    if result.is_err() {
                        error!(
                            "building_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "building_desc" {
                    // let result = buildings::handle_transaction_update_desc(&global_app_state, table).await;
                    //
                    // if result.is_err() {
                    //     error!(
                    //         "building_desc transaction update failed: {:?}",
                    //         result.err()
                    //     );
                    // }
                }

                if table_name == "item_desc" {
                    let result =
                        items::handle_transaction_update_item_desc(&global_app_state, table).await;

                    if result.is_err() {
                        error!("item_desc transaction update failed: {:?}", result.err());
                    }
                }

                if table_name == "cargo_desc" {
                    let result =
                        cargo_desc::handle_transaction_update_cargo_desc(&global_app_state, table)
                            .await;

                    if result.is_err() {
                        error!("cargo_desc transaction update failed: {:?}", result.err());
                    }
                }

                if table_name == "claim_tech_state" {
                    let result =
                        claim_tech_state::handle_transaction_update(&global_app_state, table).await;

                    if result.is_err() {
                        error!(
                            "claim_tech_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "skill_desc" {
                    let result =
                        skill_descriptions::handle_transaction_update(&global_app_state, table)
                            .await;

                    if result.is_err() {
                        error!("skill_desc transaction update failed: {:?}", result.err());
                    }
                }

                if table_name == "claim_tech_desc" {
                    let result =
                        crate::claim_tech_desc::handle_transaction_update(&global_app_state, table)
                            .await;

                    if result.is_err() {
                        error!(
                            "claim_tech_desc transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "claim_description_state" {
                    let result =
                        claims::handle_transaction_update(&db, table, broadcast_tx.clone()).await;

                    if result.is_err() {
                        error!(
                            "claim_description_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "deployable_state" {
                    let result = deployable_state::handle_transaction_update(&db, table).await;

                    if result.is_err() {
                        error!(
                            "deployable_state transaction update failed: {:?}",
                            result.err()
                        );
                    }
                }

                if table_name == "vault_state" {
                    let result = vault_state::handle_transaction_update(&db, table).await;

                    if result.is_err() {
                        error!("vault_state transaction update failed: {:?}", result.err());
                    }
                }

                if table_name == "mobile_entity_state" {
                    for current_table in table.iter() {
                        let mut old_data = HashMap::new();

                        for row in current_table.deletes.iter() {
                            let mobile_entity_state: entity::mobile_entity_state::Model =
                                match serde_json::from_str(row) {
                                    Ok(mobile_entity_state) => mobile_entity_state,
                                    Err(error) => {
                                        error!(
                                            "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
                                            error, row
                                        );
                                        continue;
                                    }
                                };

                            old_data
                                .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());
                        }

                        for row in current_table.inserts.iter() {
                            let mobile_entity_state: entity::mobile_entity_state::Model =
                                match serde_json::from_str(row) {
                                    Ok(mobile_entity_state) => mobile_entity_state,
                                    Err(error) => {
                                        error!(
                                            "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
                                            error, row
                                        );
                                        continue;
                                    }
                                };

                            global_app_state
                                .mobile_entity_state
                                .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());

                            if !app_state.user_map.iter().any(|(_, user_id)| {
                                *user_id == mobile_entity_state.entity_id as i64
                            }) {
                                continue;
                            }

                            if let Some(old_data) = old_data.get(&mobile_entity_state.entity_id) {
                                let new_location_x = if mobile_entity_state.location_x == 0 {
                                    mobile_entity_state.location_x
                                } else {
                                    mobile_entity_state.location_x / 3 / 1000
                                };

                                let new_location_z = if mobile_entity_state.location_z == 0 {
                                    mobile_entity_state.location_z
                                } else {
                                    mobile_entity_state.location_z / 3 / 1000
                                };

                                let old_location_x = if old_data.location_x == 0 {
                                    old_data.location_x
                                } else {
                                    old_data.location_x / 3 / 1000
                                };

                                let old_location_z = if old_data.location_z == 0 {
                                    old_data.location_z
                                } else {
                                    old_data.location_z / 3 / 1000
                                };

                                let change_x = new_location_x - old_location_x;
                                let change_z = new_location_z - old_location_z;

                                if change_x == 0 && change_z == 0 {
                                    continue;
                                }

                                match (
                                    global_app_state
                                        .claim_tile_state
                                        .get(&mobile_entity_state.chunk_index),
                                    global_app_state.claim_tile_state.get(&old_data.chunk_index),
                                ) {
                                    (Some(new_chunk), Some(old_chunk)) => {
                                        let new_chunk = new_chunk.value();
                                        let old_chunk = old_chunk.value();

                                        if new_chunk.claim_id != old_chunk.claim_id {
                                            broadcast_tx
                                                .send(WebSocketMessages::MovedOutOfClaim {
                                                    user_id: mobile_entity_state.entity_id as i64,
                                                    chunk_index: old_data.chunk_index,
                                                    claim_id: old_chunk.claim_id,
                                                })
                                                .unwrap();

                                            broadcast_tx
                                                .send(WebSocketMessages::PlayerMovedOutOfClaim {
                                                    user_id: mobile_entity_state.entity_id as i64,
                                                    chunk_index: old_data.chunk_index,
                                                    claim_id: old_chunk.claim_id,
                                                })
                                                .unwrap();

                                            broadcast_tx
                                                .send(WebSocketMessages::MovedIntoClaim {
                                                    user_id: mobile_entity_state.entity_id as i64,
                                                    chunk_index: mobile_entity_state.chunk_index,
                                                    claim_id: new_chunk.claim_id,
                                                })
                                                .unwrap();

                                            broadcast_tx
                                                .send(WebSocketMessages::PlayerMovedIntoClaim {
                                                    user_id: mobile_entity_state.entity_id as i64,
                                                    chunk_index: mobile_entity_state.chunk_index,
                                                    claim_id: new_chunk.claim_id,
                                                })
                                                .unwrap();
                                        }
                                    }
                                    (Some(new_chunk), None) => {
                                        let new_chunk = new_chunk.value();
                                        broadcast_tx
                                            .send(WebSocketMessages::MovedIntoClaim {
                                                user_id: mobile_entity_state.entity_id as i64,
                                                chunk_index: mobile_entity_state.chunk_index,
                                                claim_id: new_chunk.claim_id,
                                            })
                                            .unwrap();
                                        broadcast_tx
                                            .send(WebSocketMessages::PlayerMovedIntoClaim {
                                                user_id: mobile_entity_state.entity_id as i64,
                                                chunk_index: mobile_entity_state.chunk_index,
                                                claim_id: new_chunk.claim_id,
                                            })
                                            .unwrap();
                                    }
                                    (_, Some(old_chunk)) => {
                                        let old_chunk = old_chunk.value();
                                        broadcast_tx
                                            .send(WebSocketMessages::MovedOutOfClaim {
                                                user_id: mobile_entity_state.entity_id as i64,
                                                chunk_index: old_data.chunk_index,
                                                claim_id: old_chunk.claim_id,
                                            })
                                            .unwrap();
                                        broadcast_tx
                                            .send(WebSocketMessages::PlayerMovedOutOfClaim {
                                                user_id: mobile_entity_state.entity_id as i64,
                                                chunk_index: old_data.chunk_index,
                                                claim_id: old_chunk.claim_id,
                                            })
                                            .unwrap();
                                    }
                                    (_, _) => {}
                                }

                                broadcast_tx
                                    .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
                                    .unwrap();
                            } else {
                                broadcast_tx
                                    .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
                                    .unwrap();
                            }
                        }
                    }
                }

                if table_name == "claim_tile_state" {
                    for current_table in table.iter() {
                        for row in current_table.inserts.iter() {
                            let claim_tile_state: entity::claim_tile_state::Model =
                                match serde_json::from_str(row) {
                                    Ok(claim_tile_state) => claim_tile_state,
                                    Err(error) => {
                                        error!(
                                            "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
                                            error, row
                                        );
                                        continue;
                                    }
                                };

                            global_app_state
                                .claim_tile_state
                                .insert(claim_tile_state.entity_id, claim_tile_state.clone());
                        }
                    }
                }

                if table_name == "action_state" {
                    for current_table in table.iter() {
                        for row in current_table.inserts.iter() {
                            let action_state: entity::action_state::Model =
                                match serde_json::from_str(row) {
                                    Ok(action_state) => action_state,
                                    Err(error) => {
                                        error!(
                                            "InitialSubscription Insert action_state Error: {:?} -> {:?}",
                                            error, row
                                        );
                                        continue;
                                    }
                                };

                            broadcast_tx
                                .send(WebSocketMessages::ActionState(action_state.clone()))
                                .unwrap();
                            if let Some(action_states) = global_app_state
                                .action_state
                                .get_mut(&action_state.owner_entity_id)
                            {
                                action_states.insert(action_state.entity_id, action_state.clone());
                            } else {
                                let action_states = dashmap::DashMap::new();
                                action_states.insert(action_state.entity_id, action_state.clone());
                                global_app_state
                                    .action_state
                                    .insert(action_state.owner_entity_id, action_states);
                            }
                        }
                    }
                }

                if table_name == "player_action_state" {
                    for current_table in table.iter() {
                        for row in current_table.inserts.iter() {
                            let player_action_state: entity::player_action_state::Model =
                                match serde_json::from_str(row) {
                                    Ok(player_action_state) => player_action_state,
                                    Err(error) => {
                                        error!(
                                            "InitialSubscription Insert player_action_state Error: {:?} -> {:?}",
                                            error, row
                                        );
                                        continue;
                                    }
                                };

                            let old_player_action_state = global_app_state
                                .player_action_state
                                .get(&player_action_state.entity_id);
                            if old_player_action_state.is_none() {
                                broadcast_tx
                                    .send(WebSocketMessages::PlayerActionStateChangeName(
                                        player_action_state.action_type.get_action_name(),
                                        player_action_state.entity_id,
                                    ))
                                    .unwrap();
                            } else {
                                let old_player_action_state = old_player_action_state.unwrap();
                                if old_player_action_state.action_type
                                    != player_action_state.action_type
                                {
                                    broadcast_tx
                                        .send(WebSocketMessages::PlayerActionStateChangeName(
                                            player_action_state.action_type.get_action_name(),
                                            player_action_state.entity_id,
                                        ))
                                        .unwrap();
                                }
                            }

                            broadcast_tx
                                .send(WebSocketMessages::PlayerActionState(
                                    player_action_state.clone(),
                                ))
                                .unwrap();

                            global_app_state
                                .player_action_state
                                .insert(player_action_state.entity_id, player_action_state.clone());
                        }
                    }
                }

                metrics::histogram!(
                    "bitraft_event_handler_transaction_update_duration_seconds",
                    &[("table", table_name.to_string())]
                )
                .record(start.elapsed().as_secs_f64());
            }

            debug!("Received {count} events");
            evenets.clear();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}

async fn create_websocket_connection(config: &Config) -> anyhow::Result<WebSocket> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!(
            "Basic {}",
            base64::prelude::BASE64_STANDARD.encode(format!(
                "{}:{}",
                config.spacetimedb.username, config.spacetimedb.password
            ))
        )
        .parse()?,
    );
    headers.insert(SEC_WEBSOCKET_PROTOCOL, "v1.json.spacetimedb".parse()?);
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==".parse()?);
    headers.insert(
        reqwest::header::USER_AGENT,
        format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION")).parse()?,
    );

    let response = Client::default()
        .get(format!(
            "{}/{}/{}",
            config.weboosocket_url(),
            "database/subscribe",
            config.spacetimedb.database
        ))
        .headers(headers)
        .upgrade()
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct InternalTransactionUpdate {
    pub(crate) user: Option<i64>,
    pub(crate) tables: Vec<Table>,
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
pub(crate) struct QueryUpdate {
    pub(crate) deletes: Vec<Box<str>>,
    pub(crate) inserts: Vec<Box<str>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Table {
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
    pub(crate) microseconds: OffsetDateTime,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", content = "c")]
pub(crate) enum WebSocketMessages {
    Subscribe {
        topics: Vec<String>,
    },
    ListSubscribedTopics,
    SubscribedTopics(Vec<String>),
    Unsubscribe {
        topic: String,
    },
    MobileEntityState(entity::mobile_entity_state::Model),
    Experience {
        experience: u64,
        level: u64,
        rank: u64,
        skill_name: String,
        user_id: i64,
    },
    TotalExperience {
        user_id: i64,
        experience: u64,
        experience_per_hour: u64,
    },
    MovedOutOfClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    MovedIntoClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerMovedIntoClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerMovedOutOfClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerActionState(entity::player_action_state::Model),
    PlayerActionStateChangeName(String, u64),
    Level {
        level: u64,
        user_id: i64,
        skill_name: String,
    },
    PlayerState(entity::player_state::Model),
    ClaimDescriptionState(entity::claim_description_state::Model),
    Message(String),
    ActionState(entity::action_state::Model),
}

impl WebSocketMessages {
    pub fn topics(&self) -> Option<Vec<(String, i64)>> {
        match self {
            WebSocketMessages::Experience {
                skill_name,
                user_id,
                ..
            } => Some(vec![
                (format!("experience:{}", skill_name), *user_id),
                ("experience".to_string(), *user_id),
            ]),
            WebSocketMessages::Level {
                user_id,
                skill_name,
                ..
            } => Some(vec![
                (format!("level:{}", skill_name), *user_id),
                ("level".to_string(), *user_id),
            ]),
            WebSocketMessages::PlayerMovedIntoClaim { user_id, .. } => {
                Some(vec![("player_moved_into_claim".to_string(), *user_id)])
            }
            WebSocketMessages::PlayerMovedOutOfClaim { user_id, .. } => {
                Some(vec![("player_moved_out_of_claim".to_string(), *user_id)])
            }
            WebSocketMessages::MovedOutOfClaim { claim_id, .. } => {
                Some(vec![("moved_out_of_claim".to_string(), *claim_id as i64)])
            }
            WebSocketMessages::MovedIntoClaim { claim_id, .. } => {
                Some(vec![("moved_into_claim".to_string(), *claim_id as i64)])
            }
            WebSocketMessages::PlayerState(player) => {
                Some(vec![("player_state".to_string(), player.entity_id)])
            }
            WebSocketMessages::MobileEntityState(mobile_entity_state) => Some(vec![(
                "mobile_entity_state".to_string(),
                mobile_entity_state.entity_id as i64,
            )]),
            WebSocketMessages::ClaimDescriptionState(claim) => {
                Some(vec![("claim".to_string(), claim.entity_id)])
            }
            WebSocketMessages::TotalExperience { user_id, .. } => {
                Some(vec![("total_experience".to_string(), *user_id)])
            }
            WebSocketMessages::PlayerActionState(player_action_state) => Some(vec![(
                "player_action_state".to_string(),
                player_action_state.entity_id as i64,
            )]),
            WebSocketMessages::PlayerActionStateChangeName(_, id) => Some(vec![(
                "player_action_state_change_name".to_string(),
                *id as i64,
            )]),
            WebSocketMessages::ActionState(action_state) => Some(vec![(
                "action_state".to_string(),
                action_state.owner_entity_id as i64,
            )]),
            WebSocketMessages::ListSubscribedTopics => None,
            WebSocketMessages::Subscribe { .. } => None,
            WebSocketMessages::SubscribedTopics(_) => None,
            WebSocketMessages::Unsubscribe { .. } => None,
            WebSocketMessages::Message(_) => None,
        }
    }
}
