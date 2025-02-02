use crate::config::Config;
use crate::{
    buildings, claim_tech_state, claims, collectible_desc, deployable_state, inventory,
    leaderboard, player_state, vault_state, AppState,
};
use ::entity::raw_event_data::Model as RawEventData;
use ::entity::user_state;
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use axum::http::HeaderMap;
use base64::Engine;
use entity::{raw_event_data, skill_desc};
use futures::{SinkExt, TryStreamExt};
use log::{debug, error, info};
use reqwest::Client;
use reqwest_websocket::{Message, RequestBuilderExt};
use sea_orm::{EntityTrait, IntoActiveModel, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedSender;

struct WebSocketAppState {
    user_map: HashMap<String, i64>,
}

pub fn start_websocket_bitcraft_logic(
    websocket_url: String,
    websocket_password: String,
    websocket_username: String,
    database_name: String,
    tmp_config: Config,
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
) {
    tokio::spawn(async move {
        let mut app_state = WebSocketAppState {
            user_map: HashMap::new(),
        };

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
                max_frame_size: Some(1024 * 1024 * 1500),
                max_message_size: Some(1024 * 1024 * 1500),
                ..Default::default()
            })
            .protocols(vec!["v1.text.spacetimedb"])
            .send()
            .await
            .unwrap();
        let mut websocket = response.into_websocket().await.unwrap();

        let tables_to_subscribe = vec![
            "UserState",
            "MobileEntityState",
            "ClaimTileState",
            "CombatActionDesc",
            "PlayerActionState",
            "CraftingRecipeDesc",
            "ActionState",
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

            let skill_id_to_skill_name = skill_desc::Entity::find()
                .select_only()
                .column(skill_desc::Column::Id)
                .column(skill_desc::Column::Name)
                .into_tuple::<(i64, String)>()
                .all(&db)
                .await
                .unwrap()
                .into_iter()
                .map(|(id, name)| (id, name))
                .collect::<HashMap<i64, String>>();

            loop {
                let mut evenets = Vec::with_capacity(1000);
                let mut tables: HashMap<String, Vec<TableWithOriginalEventTransactionUpdate>> =
                    HashMap::new();
                let db = db.clone();

                let count = rx.recv_many(&mut evenets, 1000).await;
                let mut raw_events_data = vec![];

                for event in evenets.iter() {
                    match event {
                        WebSocketMessage::TransactionUpdate(transaction_update) => {
                            let mut compressor =
                                async_compression::tokio::write::ZstdEncoder::new(vec![]);
                            let _ = compressor
                                .write_all(
                                    serde_json::to_string(&transaction_update)
                                        .unwrap()
                                        .as_bytes(),
                                )
                                .await;
                            compressor.flush().await.unwrap();
                            compressor.shutdown().await.unwrap();

                            let user_id =
                                transaction_update.caller_identity.__identity_bytes.clone();

                            let user_id = app_state.user_map.get(&user_id.as_ref().to_string());

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
                        _ => {}
                    }
                }

                raw_event_data::Entity::insert_many(raw_events_data)
                    .exec(&db)
                    .await
                    .unwrap();

                for event in evenets.iter() {
                    match event {
                        WebSocketMessage::TransactionUpdate(transaction_update) => {
                            metrics::counter!(
                                "websocket.message.count",
                                &[("type", "TransactionUpdate"),]
                            )
                            .increment(1);

                            if transaction_update.status.committed.tables.len() == 0 {
                                continue;
                            }

                            for table in transaction_update.status.committed.tables.iter() {
                                metrics::counter!(
                                    "websocket_message_table_count",
                                    &[
                                        ("type", "TransactionUpdate".to_string()),
                                        ("table", format!("{}", table.table_name.as_ref())),
                                    ]
                                )
                                .increment(1);

                                if let Some(table_vec) =
                                    tables.get_mut(&table.table_name.as_ref().to_string())
                                {
                                    table_vec.push(TableWithOriginalEventTransactionUpdate {
                                        table_id: table.table_id,
                                        table_name: table.table_name.clone(),
                                        deletes: table.deletes.clone(),
                                        inserts: table.inserts.clone(),
                                        original_event: transaction_update.clone(),
                                    });
                                } else {
                                    tables.insert(
                                        table.table_name.clone().as_ref().to_string(),
                                        vec![TableWithOriginalEventTransactionUpdate {
                                            table_id: table.table_id,
                                            table_name: table.table_name.clone(),
                                            deletes: table.deletes.clone(),
                                            inserts: table.inserts.clone(),
                                            original_event: transaction_update.clone(),
                                        }],
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

                            if subscription_update.database_update.tables.len() == 0 {
                                continue;
                            }

                            for table in subscription_update.database_update.tables.iter() {
                                metrics::counter!(
                                    "websocket_message_table_count",
                                    &[
                                        ("type", "InitialSubscription".to_string()),
                                        ("table", format!("{}", table.table_name.as_ref())),
                                    ]
                                )
                                .increment(1);

                                let start = std::time::Instant::now();

                                if table.table_name.as_ref() == "UserState" {
                                    for row in table.inserts.iter() {
                                        let user_state: user_state::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(user_state) => user_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert UserState Error: {:?} -> {:?}", error, row.text);
                                                    continue;
                                                }
                                            };

                                        app_state.user_map.insert(
                                            user_state.identity.__identity_bytes,
                                            user_state.entity_id,
                                        );
                                    }
                                }
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

                                if table.table_name.as_ref() == "MobileEntityState" {
                                    for row in table.inserts.iter() {
                                        let mobile_entity_state: entity::mobile_entity_state::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(mobile_entity_state) => mobile_entity_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert MobileEntityState Error: {:?} -> {:?}", error, row.text);
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

                                if table.table_name.as_ref() == "ClaimTileState" {
                                    for row in table.inserts.iter() {
                                        let claim_tile_state: entity::claim_tile_state::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(claim_tile_state) => claim_tile_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert ClaimTileState Error: {:?} -> {:?}", error, row.text);
                                                    continue;
                                                }
                                            };

                                        global_app_state.claim_tile_state.insert(
                                            claim_tile_state.entity_id,
                                            claim_tile_state.clone(),
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "CraftingRecipeDesc" {
                                    for row in table.inserts.iter() {
                                        let crafting_recipe_desc: entity::crafting_recipe_desc::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(crafting_recipe_desc) => crafting_recipe_desc,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert CraftingRecipeDesc Error: {:?} -> {:?}", error, row.text);
                                                    continue;
                                                }
                                            };

                                        global_app_state.crafting_recipe_desc.insert(
                                            crafting_recipe_desc.id,
                                            crafting_recipe_desc.clone(),
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "PlayerActionState" {
                                    for row in table.inserts.iter() {
                                        let player_action_state: entity::player_action_state::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(player_action_state) => player_action_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert PlayerActionState Error: {:?} -> {:?}", error, row.text);
                                                    continue;
                                                }
                                            };

                                        global_app_state.player_action_state.insert(
                                            player_action_state.entity_id,
                                            player_action_state.clone(),
                                        );
                                    }
                                }

                                if table.table_name.as_ref() == "ActionState" {
                                    for row in table.inserts.iter() {
                                        let action_state: entity::action_state::Model =
                                            match serde_json::from_str(&row.text) {
                                                Ok(action_state) => action_state,
                                                Err(error) => {
                                                    error!("InitialSubscription Insert ActionState Error: {:?} -> {:?}", error, row.text);
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

                    if table_name == "UserState" {
                        for row in table.iter() {
                            if row.inserts.len() == 0 {
                                continue;
                            }

                            match serde_json::from_str::<user_state::Model>(
                                &row.inserts[0].text.as_ref(),
                            ) {
                                Ok(user_state) => {
                                    app_state.user_map.insert(
                                        user_state.identity.__identity_bytes,
                                        user_state.entity_id,
                                    );
                                }
                                Err(error) => {
                                    error!(
                                        "InitialSubscription Insert UserState Error: {:?}",
                                        error
                                    );
                                }
                            }
                        }
                    }

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
                        let result = player_state::handle_transaction_update_player_state(
                            &db,
                            table,
                            broadcast_tx.clone(),
                        )
                        .await;

                        if result.is_err() {
                            error!("PlayerState transaction update failed: {:?}", result.err());
                        }
                    }

                    if table_name == "ExperienceState" {
                        let result = leaderboard::handle_transaction_update(
                            &db,
                            table,
                            &skill_id_to_skill_name,
                            broadcast_tx.clone(),
                        )
                        .await;

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
                        let result =
                            claims::handle_transaction_update(&db, table, broadcast_tx.clone())
                                .await;

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

                    if table_name == "MobileEntityState" {
                        for current_table in table.iter() {
                            let mut old_data = HashMap::new();

                            for row in current_table.deletes.iter() {
                                let mobile_entity_state: entity::mobile_entity_state::Model =
                                    match serde_json::from_str(&row.text) {
                                        Ok(mobile_entity_state) => mobile_entity_state,
                                        Err(error) => {
                                            error!("InitialSubscription Insert MobileEntityState Error: {:?} -> {:?}", error, row.text);
                                            continue;
                                        }
                                    };

                                old_data.insert(
                                    mobile_entity_state.entity_id,
                                    mobile_entity_state.clone(),
                                );
                            }

                            for row in current_table.inserts.iter() {
                                let mobile_entity_state: entity::mobile_entity_state::Model =
                                    match serde_json::from_str(&row.text) {
                                        Ok(mobile_entity_state) => mobile_entity_state,
                                        Err(error) => {
                                            error!("InitialSubscription Insert MobileEntityState Error: {:?} -> {:?}", error, row.text);
                                            continue;
                                        }
                                    };

                                global_app_state.mobile_entity_state.insert(
                                    mobile_entity_state.entity_id,
                                    mobile_entity_state.clone(),
                                );

                                if !app_state.user_map.iter().any(|(_, user_id)| {
                                    *user_id == mobile_entity_state.entity_id as i64
                                }) {
                                    continue;
                                }

                                if let Some(old_data) = old_data.get(&mobile_entity_state.entity_id)
                                {
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
                                        global_app_state
                                            .claim_tile_state
                                            .get(&old_data.chunk_index),
                                    ) {
                                        (Some(new_chunk), Some(old_chunk)) => {
                                            let new_chunk = new_chunk.value();
                                            let old_chunk = old_chunk.value();

                                            if new_chunk.claim_id != old_chunk.claim_id {
                                                broadcast_tx
                                                    .send(WebSocketMessages::MovedOutOfClaim {
                                                        user_id: mobile_entity_state.entity_id
                                                            as i64,
                                                        chunk_index: old_data.chunk_index,
                                                        claim_id: old_chunk.claim_id,
                                                    })
                                                    .unwrap();

                                                broadcast_tx
                                                    .send(
                                                        WebSocketMessages::PlayerMovedOutOfClaim {
                                                            user_id: mobile_entity_state.entity_id
                                                                as i64,
                                                            chunk_index: old_data.chunk_index,
                                                            claim_id: old_chunk.claim_id,
                                                        },
                                                    )
                                                    .unwrap();

                                                broadcast_tx
                                                    .send(WebSocketMessages::MovedIntoClaim {
                                                        user_id: mobile_entity_state.entity_id
                                                            as i64,
                                                        chunk_index: mobile_entity_state
                                                            .chunk_index,
                                                        claim_id: new_chunk.claim_id,
                                                    })
                                                    .unwrap();

                                                broadcast_tx
                                                    .send(WebSocketMessages::PlayerMovedIntoClaim {
                                                        user_id: mobile_entity_state.entity_id
                                                            as i64,
                                                        chunk_index: mobile_entity_state
                                                            .chunk_index,
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
                                        .send(WebSocketMessages::MobileEntityState(
                                            mobile_entity_state,
                                        ))
                                        .unwrap();
                                } else {
                                    broadcast_tx
                                        .send(WebSocketMessages::MobileEntityState(
                                            mobile_entity_state,
                                        ))
                                        .unwrap();
                                }
                            }
                        }
                    }

                    if table_name == "ClaimTileState" {
                        for current_table in table.iter() {
                            for row in current_table.inserts.iter() {
                                let claim_tile_state: entity::claim_tile_state::Model =
                                    match serde_json::from_str(&row.text) {
                                        Ok(claim_tile_state) => claim_tile_state,
                                        Err(error) => {
                                            error!("InitialSubscription Insert ClaimTileState Error: {:?} -> {:?}", error, row.text);
                                            continue;
                                        }
                                    };

                                global_app_state
                                    .claim_tile_state
                                    .insert(claim_tile_state.entity_id, claim_tile_state.clone());
                            }
                        }
                    }

                    if table_name == "ActionState" {
                        for current_table in table.iter() {
                            for row in current_table.inserts.iter() {
                                let action_state: entity::action_state::Model =
                                    match serde_json::from_str(&row.text) {
                                        Ok(action_state) => action_state,
                                        Err(error) => {
                                            error!("InitialSubscription Insert ActionState Error: {:?} -> {:?}", error, row.text);
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
                                    action_states
                                        .insert(action_state.entity_id, action_state.clone());
                                } else {
                                    let action_states = dashmap::DashMap::new();
                                    action_states
                                        .insert(action_state.entity_id, action_state.clone());
                                    global_app_state
                                        .action_state
                                        .insert(action_state.owner_entity_id, action_states);
                                }
                            }
                        }
                    }

                    if table_name == "PlayerActionState" {
                        for current_table in table.iter() {
                            for row in current_table.inserts.iter() {
                                let player_action_state: entity::player_action_state::Model =
                                    match serde_json::from_str(&row.text) {
                                        Ok(player_action_state) => player_action_state,
                                        Err(error) => {
                                            error!("InitialSubscription Insert PlayerActionState Error: {:?} -> {:?}", error, row.text);
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

                                global_app_state.player_action_state.insert(
                                    player_action_state.entity_id,
                                    player_action_state.clone(),
                                );
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
                        info!("Received identity token: {identity_token:?}");
                    }
                }
            } else {
                if let Message::Ping(_) = message {
                } else {
                    info!("Message: {:?}", message);
                }
            }
        }
    });
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
    pub(crate) __identity_bytes: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Address {
    pub(crate) __address_bytes: Box<str>,
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
pub(crate) struct Table {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) deletes: Vec<TableText>,
    pub(crate) inserts: Vec<TableText>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TableWithOriginalEventTransactionUpdate {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) deletes: Vec<TableText>,
    pub(crate) inserts: Vec<TableText>,
    pub(crate) original_event: TransactionUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TableText {
    #[serde(rename = "Text")]
    pub(crate) text: Box<str>,
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
