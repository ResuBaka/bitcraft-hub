use super::module_bindings::*;
use crate::config::{Config, SpacetimeDbConfig};
use crate::{
    AppState, buildings, cargo_desc, claim_tech_state, claims, collectible_desc, deployable_state,
    inventory, items, leaderboard, player_state, skill_descriptions, vault_state,
};
use ::entity::raw_event_data::Model as RawEventData;
use ::entity::user_state;
use axum::http::HeaderMap;
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use entity::mobile_entity_state;
#[allow(unused_imports)]
use entity::{raw_event_data, skill_desc};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::{debug, error, info};
use reqwest::ClientBuilder;
use reqwest_websocket::{Message, RequestBuilderExt, WebSocket};
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait, QuerySelect, sea_query};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spacetimedb_sdk::{
    Compression, DbContext, Error, Identity, Table, TableWithPrimaryKey, credentials,
};
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tokio::time::{Duration, sleep};
use tracing::instrument::WithSubscriber;
use tracing::warn;

fn connect_to_db(db_name: &str, db_host: &str) -> DbConnection {
    DbConnection::builder()
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(on_connected)
        // Register our `on_connect_error` callback, which will print a message, then exit the process.
        .on_connect_error(on_connect_error)
        // Our `on_disconnect` callback, which will print a message, then exit the process.
        .on_disconnect(on_disconnected)
        // If the user has previously connected, we'll have saved a token in the `on_connect` callback.
        // In that case, we'll load it and pass it to `with_token`,
        // so we can re-authenticate as the same `Identity`.
        .with_token(creds_store().load().expect("Error loading credentials"))
        // Set the database name we chose when we called `spacetime publish`.
        .with_module_name(db_name)
        // Set the URI of the SpacetimeDB host that's running our database.
        .with_uri(db_host)
        // Finalize configuration and connect!
        .with_compression(Compression::Brotli)
        .build()
        .expect("Failed to connect")
}

// /// Register subscriptions for all rows of both tables.
// fn subscribe_to_tables(ctx: &DbConnection) {
//     ctx.subscription_builder()
//         .on_applied(on_sub_applied)
//         .on_error(on_sub_error)
//         .subscribe(["SELECT * FROM player_state", "SELECT * FROM mobile_entity_state"]);
// }

/// Or `on_error` callback:
/// print the error, then exit the process.
fn on_sub_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Subscription failed: {}", err);
    // std::process::exit(1);
}

fn creds_store() -> credentials::File {
    credentials::File::new("bitcraft-beta")
}

/// Our `on_connect` callback: save our credentials to a file.
fn on_connected(_ctx: &DbConnection, _identity: spacetimedb_sdk::Identity, token: &str) {
    if let Err(e) = creds_store().save(token) {
        eprintln!("Failed to save credentials: {:?}", e);
    }
}

/// Our `on_connect_error` callback: print the error, then exit the process.
fn on_connect_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Connection error: {:?}", err);
    // std::process::exit(1);
}

/// Our `on_disconnect` callback: print a note, then exit the process.
fn on_disconnected(_ctx: &ErrorContext, err: Option<Error>) {
    if let Some(err) = err {
        eprintln!("Disconnected: {}", err);
        // std::process::exit(1);
    } else {
        println!("Disconnected.");
        // std::process::exit(0);
    }
}
fn connect_to_db_logic(
    config: &Config,
    database: &String,
    mobile_entity_state_tx: &UnboundedSender<SpacetimeUpdateMessages<MobileEntityState>>,
    player_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerState>>,
    player_username_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    experience_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
) {
    let ctx = connect_to_db(&database, config.spacetimedb_url().as_ref());
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db.mobile_entity_state().on_update(
        move |_ctx: &EventContext, old: &MobileEntityState, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db
        .mobile_entity_state()
        .on_insert(move |_ctx: &EventContext, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db
        .mobile_entity_state()
        .on_delete(move |_ctx: &EventContext, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_player_state_tx = player_state_tx.clone();
    ctx.db.player_state().on_update(
        move |_ctx: &EventContext, old: &PlayerState, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_player_state_tx = player_state_tx.clone();
    ctx.db
        .player_state()
        .on_insert(move |_ctx: &EventContext, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_player_state_tx = player_state_tx.clone();
    ctx.db
        .player_state()
        .on_delete(move |_ctx: &EventContext, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_update(
        move |_ctx: &EventContext, old: &PlayerUsernameState, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_insert(
        move |_ctx: &EventContext, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        },
    );
    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_delete(
        move |_ctx: &EventContext, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        },
    );

    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db.experience_state().on_update(
        move |_ctx: &EventContext, old: &ExperienceState, new: &ExperienceState| {
            tracing::info!("good update for experience_state");
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db
        .experience_state()
        .on_insert(move |_ctx: &EventContext, new: &ExperienceState| {
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db
        .experience_state()
        .on_delete(move |_ctx: &EventContext, new: &ExperienceState| {
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let tables_to_subscribe = vec![
        // "user_state",
        // "mobile_entity_state",
        // "claim_tile_state",
        // "combat_action_desc",
        // "item_desc",
        // "cargo_desc",
        // "player_action_state",
        // "crafting_recipe_desc",
        // "action_state",
        "player_state",
        // "skill_desc",
        "player_username_state",
        // "building_desc",
        // "building_state",
        // "vault_state",
        "experience_state",
        // "claim_tech_state",
        // "claim_state",
        // "claim_member_state",
        // "claim_local_state",
        // "deployable_state",
        // "collectible_desc",
        // "claim_tech_desc",
        // "claim_description_state", -> claim_state
        // "mobile_entity_state",
        // "claim_tile_state",
        // "crafting_recipe_desc",
        // "player_action_state",
        // "action_state",
        // "location_state",
        // "inventory_state",
    ];

    let temp_tx = mobile_entity_state_tx.clone();
    ctx.subscription_builder()
        .on_applied(move |ctx: &SubscriptionEventContext| {})
        .on_error(on_sub_error)
        .subscribe(
            tables_to_subscribe
                .into_iter()
                .map(|table| format!("select * from {table}"))
                .collect::<Vec<_>>(),
        );

    tokio::spawn(async move {
        let _ = ctx.run_async().await;
    });
}

pub fn start_websocket_bitcraft_logic(
    config: Config,
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
) {
    tokio::spawn(async move {
        let (mobile_entity_state_tx, mobile_entity_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (player_state_tx, player_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (player_username_state_tx, player_username_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (experience_state_tx, experience_state_rx) = tokio::sync::mpsc::unbounded_channel();

        config.spacetimedb.databases.iter().for_each(|database| {
            connect_to_db_logic(
                &config,
                database,
                &mobile_entity_state_tx,
                &player_state_tx,
                &player_username_state_tx,
                &experience_state_tx,
            )
        });
        start_worker_mobile_entity_state(
            broadcast_tx.clone(),
            global_app_state.clone(),
            mobile_entity_state_rx,
        );
        start_worker_player_state(
            broadcast_tx.clone(),
            global_app_state.clone(),
            player_state_rx,
            1000,
            Duration::from_millis(25),
        );
        start_worker_player_username_state(
            broadcast_tx.clone(),
            global_app_state.clone(),
            player_username_state_rx,
            1000,
            Duration::from_millis(25),
        );
        start_worker_experience_state(
            broadcast_tx.clone(),
            global_app_state.clone(),
            experience_state_rx,
            2000,
            Duration::from_millis(25),
        );
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

enum SpacetimeUpdateMessages<T> {
    Insert { new: T },
    Update { old: T, new: T },
    Remove { delete: T },
}

impl From<MobileEntityState> for ::entity::mobile_entity_state::Model {
    fn from(value: MobileEntityState) -> Self {
        mobile_entity_state::Model {
            entity_id: value.entity_id,
            chunk_index: value.chunk_index,
            timestamp: value.timestamp,
            location_x: value.location_x,
            location_z: value.location_z,
            destination_x: value.destination_x,
            destination_z: value.destination_z,
            dimension: value.dimension,
            is_running: value.is_running,
        }
    }
}

impl From<PlayerState> for ::entity::player_state::Model {
    fn from(value: PlayerState) -> Self {
        let teleport_location = ::entity::player_state::TeleportLocation {
            location: ::entity::player_state::OffsetCoordinatesSmallMessage {
                x: value.teleport_location.location.x.clone(),
                z: value.teleport_location.location.z.clone(),
                dimension: value.teleport_location.location.dimension.clone(),
            },
            location_type: match value.teleport_location.location_type {
                TeleportLocationType::BirthLocation => {
                    ::entity::player_state::TeleportLocationType::BirthLocation
                }
                TeleportLocationType::TradingPost => {
                    ::entity::player_state::TeleportLocationType::TradingPost
                }
                TeleportLocationType::HomeLocation => {
                    ::entity::player_state::TeleportLocationType::HomeLocation
                }
                TeleportLocationType::CustomLocation => {
                    ::entity::player_state::TeleportLocationType::CustomLocation
                }
                TeleportLocationType::Waystone => {
                    ::entity::player_state::TeleportLocationType::Waystone
                }
            },
        };

        ::entity::player_state::Model {
            teleport_location,
            entity_id: value.entity_id as i64,
            time_played: value.time_played,
            session_start_timestamp: value.session_start_timestamp,
            time_signed_in: value.time_signed_in,
            sign_in_timestamp: value.sign_in_timestamp,
            signed_in: value.signed_in,
            traveler_tasks_expiration: value.traveler_tasks_expiration,
        }
    }
}

impl From<PlayerUsernameState> for ::entity::player_username_state::Model {
    fn from(value: PlayerUsernameState) -> Self {
        ::entity::player_username_state::Model {
            entity_id: value.entity_id as i64,
            username: value.username,
        }
    }
}

fn start_worker_mobile_entity_state(
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Insert { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    broadcast_tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                SpacetimeUpdateMessages::Update { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    broadcast_tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                SpacetimeUpdateMessages::Remove { delete, .. } => {
                    global_app_state
                        .mobile_entity_state
                        .remove(&delete.entity_id);
                }
            }
        }
    });
}

fn start_worker_player_state(
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::player_state::Column::EntityId)
            .update_columns([
                ::entity::player_state::Column::TimePlayed,
                ::entity::player_state::Column::SessionStartTimestamp,
                ::entity::player_state::Column::TimeSignedIn,
                ::entity::player_state::Column::SignInTimestamp,
                ::entity::player_state::Column::SignedIn,
                ::entity::player_state::Column::TeleportLocation,
                ::entity::player_state::Column::TravelerTasksExpiration,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(player_state = id, "Could not delete player_state");
                                }

                                tracing::info!("SpacetimeUpdateMessages::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::player_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_player_username_state(
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
                .update_columns([::entity::player_username_state::Column::Username])
                .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_username_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(player_username_state = id, "Could not delete player_username_state");
                                }

                                tracing::info!("SpacetimeUpdateMessages::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::player_username_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_experience_state(
    broadcast_tx: UnboundedSender<WebSocketMessages>,
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([::entity::experience_state::Column::Experience])
        .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let id = new.entity_id;
                                new.experience_stacks.iter().for_each(|es| {
                                    messages.push(::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    })
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let id = new.entity_id;
                                new.experience_stacks.iter().for_each(|es| {
                                    messages.push(::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    })
                                });
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let id = delete.entity_id as i64;
                                let vec_es = delete.experience_stacks.iter().map(|es| {
                                    if let Some(index) = messages.iter().position(|value| value.skill_id == es.skill_id && value.entity_id == id) {
                                        messages.remove(index);
                                    }

                                    ::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    }
                                }).collect::<Vec<_>>();

                                for es in vec_es {
                                    if let Err(error) = es.delete(&global_app_state.conn).await {
                                        tracing::error!(experience_state = id, "Could not delete experience_state");
                                    }
                                }
                                tracing::info!("SpacetimeUpdateMessages::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::experience_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

//
// async fn create_websocket_connection(config: &Config) -> anyhow::Result<WebSocket> {
//     let mut headers = HeaderMap::new();
//     headers.insert(
//         "Authorization",
//         format!("Bearer {}", config.spacetimedb.password).parse()?,
//     );
//     headers.insert(SEC_WEBSOCKET_PROTOCOL, "v1.json.spacetimedb".parse()?);
//     headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==".parse()?);
//     headers.insert("Sec-WebSocket-Version", "13".parse()?);
//     headers.insert(
//         reqwest::header::USER_AGENT,
//         format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION")).parse()?,
//     );
//
//     let response = ClientBuilder::default()
//         .timeout(Duration::from_millis(5000))
//         .connect_timeout(Duration::from_millis(2500))
//         .build()?
//         .get(format!(
//             "{}/{}/{}/{}",
//             config.weboosocket_url(),
//             "database",
//             config.spacetimedb.database,
//             "subscribe"
//         ))
//         .headers(headers)
//         .upgrade()
//         // .web_socket_config(tungstenite::protocol::WebSocketConfig::default()
//         //    .max_frame_size(Some(1024 * 1024 * 1500))
//         //    .max_message_size(Some(1024 * 1024 * 1500))
//         // )
//         .web_socket_config(tungstenite::protocol::WebSocketConfig {
//             max_frame_size: Some(1024 * 1024 * 1500),
//             max_message_size: Some(1024 * 1024 * 1500),
//             ..Default::default()
//         })
//         .protocols(vec!["v1.json.spacetimedb"])
//         .send()
//         .await?;
//
//     Ok(response.into_websocket().await?)
// }

//
// fn start_websocket_message_thread(
//     broadcast_tx: UnboundedSender<WebSocketMessages>,
//     global_app_state: Arc<AppState>,
//     mut rx: UnboundedReceiver<WebSocketMessage>,
//     tmp_config: Config,
// ) {
//     tokio::spawn(async move {
//         let db = crate::create_importer_default_db_connection(tmp_config.clone()).await;
//
//         let skill_id_to_skill_name = skill_desc::Entity::find()
//             .select_only()
//             .column(skill_desc::Column::Id)
//             .column(skill_desc::Column::Name)
//             .into_tuple::<(i64, String)>()
//             .all(&db)
//             .await
//             .unwrap()
//             .into_iter()
//             .collect::<HashMap<i64, String>>();
//
//         loop {
//             let mut evenets = Vec::with_capacity(1000);
//             let mut tables: HashMap<String, Vec<TableWithOriginalEventTransactionUpdate>> =
//                 HashMap::new();
//             let db = db.clone();
//
//             let count = rx.recv_many(&mut evenets, 1000).await;
//             let mut raw_events_data = vec![];
//
//             for event in evenets.iter() {
//                 if let WebSocketMessage::TransactionUpdate(transaction_update) = event {
//                     let mut compressor = async_compression::tokio::write::ZstdEncoder::new(vec![]);
//                     let _ = compressor
//                         .write_all(
//                             serde_json::to_string(&transaction_update)
//                                 .unwrap()
//                                 .as_bytes(),
//                         )
//                         .await;
//                     compressor.flush().await.unwrap();
//                     compressor.shutdown().await.unwrap();
//
//                     let user_id = transaction_update.caller_identity.__identity__.clone();
//
//                     let user_id = if let Some(user_id_ref) = global_app_state.connected_user_map.get(&user_id.to_string()) {
//                         Some(user_id_ref.to_owned())
//                     } else {
//                         None
//                     };
//
//                     raw_events_data.push(
//                         RawEventData {
//                             timestamp: transaction_update
//                                 .timestamp
//                                 .__timestamp_micros_since_unix_epoch__,
//                             request_id: transaction_update.reducer_call.request_id as i64,
//                             reducer_name: transaction_update
//                                 .reducer_call
//                                 .reducer_name
//                                 .clone()
//                                 .parse()
//                                 .unwrap(),
//                             reducer_id: transaction_update.reducer_call.reducer_id as i64,
//                             event_data: compressor.into_inner(),
//                             user_id,
//                         }
//                             .into_active_model(),
//                     );
//                 }
//             }
//
//             //raw_event_data::Entity::insert_many(raw_events_data)
//             //    .exec(&db)
//             //    .await
//             //    .unwrap();
//
//             for event in evenets.iter() {
//                 match event {
//                     WebSocketMessage::TransactionUpdate(transaction_update) => {
//                         metrics::counter!(
//                             "websocket.message.count",
//                             &[("type", "TransactionUpdate"),]
//                         )
//                             .increment(1);
//
//                         if transaction_update.status.failed.is_some() {
//                             error!(
//                                 "Transaction with error {}",
//                                 transaction_update.status.failed.as_ref().unwrap()
//                             );
//                             continue;
//                         }
//
//                         if transaction_update
//                             .status
//                             .committed
//                             .as_ref()
//                             .unwrap()
//                             .tables
//                             .is_empty()
//                         {
//                             continue;
//                         }
//
//                         for table in transaction_update
//                             .status
//                             .committed
//                             .as_ref()
//                             .unwrap()
//                             .tables
//                             .iter()
//                         {
//                             metrics::counter!(
//                                 "websocket_message_table_count",
//                                 &[
//                                     ("type", "TransactionUpdate".to_string()),
//                                     ("table", format!("{}", table.table_name)),
//                                 ]
//                             )
//                                 .increment(1);
//
//                             if let Some(table_vec) = tables.get_mut(&table.table_name.to_string()) {
//                                 //TODO this probebly has to be rewriten
//                                 table.updates.iter().for_each(|updates| {
//                                     table_vec.push(TableWithOriginalEventTransactionUpdate {
//                                         table_id: table.table_id,
//                                         table_name: table.table_name.clone(),
//                                         deletes: updates.deletes.clone(),
//                                         inserts: updates.inserts.clone(),
//                                         original_event: transaction_update.clone(),
//                                     });
//                                 })
//                             } else {
//                                 tables.insert(
//                                     table.table_name.clone().as_ref().to_string(),
//                                     table
//                                         .updates
//                                         .iter()
//                                         .map(|updates| TableWithOriginalEventTransactionUpdate {
//                                             table_id: table.table_id,
//                                             table_name: table.table_name.clone(),
//                                             deletes: updates.deletes.clone(),
//                                             inserts: updates.inserts.clone(),
//                                             original_event: transaction_update.clone(),
//                                         })
//                                         .collect::<Vec<TableWithOriginalEventTransactionUpdate>>(),
//                                 );
//                             }
//                         }
//
//                         metrics::gauge!(
//                             "websocket_message_inflight_gauge",
//                             &[("type", "TransactionUpdate"),]
//                         )
//                             .decrement(1);
//                     }
//                     WebSocketMessage::InitialSubscription(subscription_update) => {
//                         metrics::counter!(
//                             "websocket.message.count",
//                             &[("type", "InitialSubscription"),]
//                         )
//                             .increment(1);
//
//                         if subscription_update.database_update.tables.is_empty() {
//                             continue;
//                         }
//
//                         for table in subscription_update.database_update.tables.iter() {
//                             metrics::counter!(
//                                 "websocket_message_table_count",
//                                 &[
//                                     ("type", "InitialSubscription".to_string()),
//                                     ("table", format!("{}", table.table_name)),
//                                 ]
//                             )
//                                 .increment(1);
//
//                             let start = std::time::Instant::now();
//
//                             if table.table_name.as_ref() == "user_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let user_state: user_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(user_state) => user_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert user_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.connected_user_map.insert(
//                                             user_state.identity.__identity__,
//                                             user_state.entity_id,
//                                         );
//                                     }
//                                 }
//                             }
//                             if table.table_name.as_ref() == "player_username_state" {
//                                 let result =
//                                     player_state::handle_initial_subscription_player_username_state(&db, table)
//                                         .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "player_username_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "player_state" {
//                                 let result =
//                                     player_state::handle_initial_subscription_player_state(
//                                         &db, table,
//                                     )
//                                         .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "player_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "experience_state" {
//                                 info!("experience_state initial subscription");
//                                 let result =
//                                     leaderboard::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "experience_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "building_state" {
//                                 let result =
//                                     buildings::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "building_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "building_desc" {
//                                 let result = buildings::handle_initial_subscription_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "building_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "inventory_state" {
//                                 let result =
//                                     inventory::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "inventory_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "item_desc" {
//                                 let result = items::handle_initial_subscription_item_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "item_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "cargo_desc" {
//                                 let result = cargo_desc::handle_initial_subscription_cargo_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "cargo_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tech_state" {
//                                 let result = claim_tech_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_tech_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "skill_desc" {
//                                 let result = skill_descriptions::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "skill_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tech_desc" {
//                                 let result = crate::claim_tech_desc::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_tech_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_state" {
//                                 let result = crate::claims::claim_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_local_state" {
//                                 let result = crate::claims::claim_local_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_local_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_local_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_member_state" {
//                                 let result = crate::claims::claim_member_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_member_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_member_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             // if table.table_name.as_ref() == "claim_description_state" {
//                             //     let result =
//                             //         claims::handle_initial_subscription(&global_app_state, table)
//                             //             .await;
//                             //
//                             //     if result.is_err() {
//                             //         error!(
//                             //             "claim_description_state initial subscription failed: {:?}",
//                             //             result.err()
//                             //         );
//                             //     }
//                             // }
//
//                             if table.table_name.as_ref() == "deployable_state" {
//                                 let result =
//                                     deployable_state::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "deployable_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "vault_state" {
//                                 let result =
//                                     vault_state::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "vault_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "collectible_desc" {
//                                 let result =
//                                     collectible_desc::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "collectible_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "mobile_entity_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let mobile_entity_state: entity::mobile_entity_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(mobile_entity_state) => mobile_entity_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.mobile_entity_state.insert(
//                                             mobile_entity_state.entity_id,
//                                             mobile_entity_state.clone(),
//                                         );
//
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MobileEntityState(
//                                                 mobile_entity_state,
//                                             ))
//                                             .unwrap();
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tile_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let claim_tile_state: entity::claim_tile_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(claim_tile_state) => claim_tile_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.claim_tile_state.insert(
//                                             claim_tile_state.entity_id,
//                                             claim_tile_state.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "crafting_recipe_desc" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let crafting_recipe_desc: entity::crafting_recipe::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(crafting_recipe_desc) => crafting_recipe_desc,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert crafting_recipe_desc Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.crafting_recipe_desc.insert(
//                                             crafting_recipe_desc.id,
//                                             crafting_recipe_desc.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "player_action_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let player_action_state: entity::player_action_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(player_action_state) => player_action_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert player_action_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.player_action_state.insert(
//                                             player_action_state.entity_id,
//                                             player_action_state.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "location_state" {
//                                 let mut num_entries = 0;
//                                 for update in table.updates.iter() {
//                                     num_entries = update.inserts.len();
//                                     for row in update.inserts.iter() {
//                                         let location_state: entity::location::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(location_state) => location_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert location_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.location_state.insert(
//                                             location_state.entity_id,
//                                             location_state.clone(),
//                                         );
//                                     }
//                                 }
//
//                                 info!(
//                                     "location_state initial subscription success",
//                                 );
//                             }
//
//                             if table.table_name.as_ref() == "action_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let action_state: entity::action_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(action_state) => action_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert action_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         if let Some(action_states) = global_app_state
//                                             .action_state
//                                             .get_mut(&action_state.owner_entity_id)
//                                         {
//                                             action_states.insert(
//                                                 action_state.entity_id,
//                                                 action_state.clone(),
//                                             );
//                                         } else {
//                                             let action_states = dashmap::DashMap::new();
//                                             action_states.insert(
//                                                 action_state.entity_id,
//                                                 action_state.clone(),
//                                             );
//                                             global_app_state.action_state.insert(
//                                                 action_state.owner_entity_id,
//                                                 action_states,
//                                             );
//                                         }
//                                     }
//                                 }
//                             }
//
//                             metrics::histogram!(
//                                 "bitraft_event_handler_initial_subscription_duration_seconds",
//                                 &[("table", table.table_name.as_ref().to_string())]
//                             )
//                                 .record(start.elapsed().as_secs_f64());
//                         }
//
//                         metrics::gauge!(
//                             "websocket_message_inflight_gauge",
//                             &[("type", "InitialSubscription"),]
//                         )
//                             .decrement(1);
//                     }
//                     WebSocketMessage::IdentityToken(identity_token) => {
//                         println!("IdentityToken: {identity_token:?}");
//                     }
//                 }
//             }
//
//             for (table_name, table) in tables.iter() {
//                 debug!("Received table: {table_name} -> {:?}", table.len());
//                 let start = std::time::Instant::now();
//
//                 if table_name == "user_state" {
//                     for row in table.iter() {
//                         if row.inserts.is_empty() {
//                             continue;
//                         }
//
//                         match serde_json::from_str::<user_state::Model>(&row.inserts[0]) {
//                             Ok(user_state) => {
//                                 global_app_state.connected_user_map
//                                     .insert(user_state.identity.__identity__, user_state.entity_id);
//                             }
//                             Err(error) => {
//                                 error!("InitialSubscription Insert UserState Error: {:?}", error);
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "player_username_state" {
//                     let result =
//                         player_state::handle_transaction_update_player_username_state(&db, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "player_username_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//                 if table_name == "player_state" {
//                     let result = player_state::handle_transaction_update_player_state(
//                         &db,
//                         table,
//                         broadcast_tx.clone(),
//                     )
//                         .await;
//
//                     if result.is_err() {
//                         error!("player_state transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "experience_state" {
//                     let result = leaderboard::handle_transaction_update(
//                         &db,
//                         table,
//                         &skill_id_to_skill_name,
//                         broadcast_tx.clone(),
//                     )
//                         .await;
//
//                     if result.is_err() {
//                         error!(
//                             "experience_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "inventory_state" {
//                     let result = inventory::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "inventory_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "building_state" {
//                     let result = buildings::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "building_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "building_desc" {
//                     // let result = buildings::handle_transaction_update_desc(&global_app_state, table).await;
//                     //
//                     // if result.is_err() {
//                     //     error!(
//                     //         "building_desc transaction update failed: {:?}",
//                     //         result.err()
//                     //     );
//                     // }
//                 }
//
//                 if table_name == "item_desc" {
//                     let result =
//                         items::handle_transaction_update_item_desc(&global_app_state, table).await;
//
//                     if result.is_err() {
//                         error!("item_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "cargo_desc" {
//                     let result =
//                         cargo_desc::handle_transaction_update_cargo_desc(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!("cargo_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "claim_tech_state" {
//                     let result =
//                         claim_tech_state::handle_transaction_update(&global_app_state, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_tech_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "skill_desc" {
//                     let result =
//                         skill_descriptions::handle_transaction_update(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!("skill_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "claim_tech_desc" {
//                     let result =
//                         crate::claim_tech_desc::handle_transaction_update(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_tech_desc transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_state" {
//                     let result =
//                         crate::claims::claim_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_member_state" {
//                     let result =
//                         crate::claims::claim_member_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_member_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_local_state" {
//                     let result =
//                         crate::claims::claim_local_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_local_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 // if table_name == "claim_description_state" {
//                 //     let result = claims::handle_transaction_update(
//                 //         &global_app_state,
//                 //         table,
//                 //         broadcast_tx.clone(),
//                 //     )
//                 //         .await;
//                 //
//                 //     if result.is_err() {
//                 //         error!(
//                 //             "claim_description_state transaction update failed: {:?}",
//                 //             result.err()
//                 //         );
//                 //     }
//                 // }
//
//                 if table_name == "deployable_state" {
//                     let result = deployable_state::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "deployable_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "vault_state" {
//                     let result = vault_state::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!("vault_state transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "mobile_entity_state" {
//                     for current_table in table.iter() {
//                         let mut old_data = HashMap::new();
//
//                         for row in current_table.deletes.iter() {
//                             let mobile_entity_state: entity::mobile_entity_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(mobile_entity_state) => mobile_entity_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             old_data
//                                 .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());
//                         }
//
//                         for row in current_table.inserts.iter() {
//                             let mobile_entity_state: entity::mobile_entity_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(mobile_entity_state) => mobile_entity_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .mobile_entity_state
//                                 .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());
//
//                             if !global_app_state.connected_user_map.iter().any(|connected_user| {
//                                 *connected_user == mobile_entity_state.entity_id as i64
//                             }) {
//                                 continue;
//                             }
//
//                             if let Some(old_data) = old_data.get(&mobile_entity_state.entity_id) {
//                                 let new_location_x = if mobile_entity_state.location_x == 0 {
//                                     mobile_entity_state.location_x
//                                 } else {
//                                     mobile_entity_state.location_x / 3 / 1000
//                                 };
//
//                                 let new_location_z = if mobile_entity_state.location_z == 0 {
//                                     mobile_entity_state.location_z
//                                 } else {
//                                     mobile_entity_state.location_z / 3 / 1000
//                                 };
//
//                                 let old_location_x = if old_data.location_x == 0 {
//                                     old_data.location_x
//                                 } else {
//                                     old_data.location_x / 3 / 1000
//                                 };
//
//                                 let old_location_z = if old_data.location_z == 0 {
//                                     old_data.location_z
//                                 } else {
//                                     old_data.location_z / 3 / 1000
//                                 };
//
//                                 let change_x = new_location_x - old_location_x;
//                                 let change_z = new_location_z - old_location_z;
//
//                                 if change_x == 0 && change_z == 0 {
//                                     continue;
//                                 }
//
//                                 match (
//                                     global_app_state
//                                         .claim_tile_state
//                                         .get(&mobile_entity_state.chunk_index),
//                                     global_app_state.claim_tile_state.get(&old_data.chunk_index),
//                                 ) {
//                                     (Some(new_chunk), Some(old_chunk)) => {
//                                         let new_chunk = new_chunk.value();
//                                         let old_chunk = old_chunk.value();
//
//                                         if new_chunk.claim_id != old_chunk.claim_id {
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::MovedOutOfClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: old_data.chunk_index,
//                                                     claim_id: old_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::PlayerMovedOutOfClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: old_data.chunk_index,
//                                                     claim_id: old_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::MovedIntoClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: mobile_entity_state.chunk_index,
//                                                     claim_id: new_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::PlayerMovedIntoClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: mobile_entity_state.chunk_index,
//                                                     claim_id: new_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//                                         }
//                                     }
//                                     (Some(new_chunk), None) => {
//                                         let new_chunk = new_chunk.value();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MovedIntoClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: mobile_entity_state.chunk_index,
//                                                 claim_id: new_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::PlayerMovedIntoClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: mobile_entity_state.chunk_index,
//                                                 claim_id: new_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                     }
//                                     (_, Some(old_chunk)) => {
//                                         let old_chunk = old_chunk.value();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MovedOutOfClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: old_data.chunk_index,
//                                                 claim_id: old_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::PlayerMovedOutOfClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: old_data.chunk_index,
//                                                 claim_id: old_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                     }
//                                     (_, _) => {}
//                                 }
//
//                                 broadcast_tx
//                                     .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
//                                     .unwrap();
//                             } else {
//                                 broadcast_tx
//                                     .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
//                                     .unwrap();
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "claim_tile_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let claim_tile_state: entity::claim_tile_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(claim_tile_state) => claim_tile_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .claim_tile_state
//                                 .insert(claim_tile_state.entity_id, claim_tile_state.clone());
//                         }
//                     }
//                 }
//
//                 if table_name == "action_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let action_state: entity::action_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(action_state) => action_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert action_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             broadcast_tx
//                                 .send(WebSocketMessages::ActionState(action_state.clone()))
//                                 .unwrap();
//                             if let Some(action_states) = global_app_state
//                                 .action_state
//                                 .get_mut(&action_state.owner_entity_id)
//                             {
//                                 action_states.insert(action_state.entity_id, action_state.clone());
//                             } else {
//                                 let action_states = dashmap::DashMap::new();
//                                 action_states.insert(action_state.entity_id, action_state.clone());
//                                 global_app_state
//                                     .action_state
//                                     .insert(action_state.owner_entity_id, action_states);
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "player_action_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//
//                             let player_action_state: entity::player_action_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(player_action_state) => player_action_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert player_action_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             let old_player_action_state = global_app_state
//                                 .player_action_state
//                                 .get(&player_action_state.entity_id);
//                             if old_player_action_state.is_none() {
//                                 broadcast_tx
//                                     .send(WebSocketMessages::PlayerActionStateChangeName(
//                                         player_action_state.action_type.get_action_name(),
//                                         player_action_state.entity_id,
//                                     ))
//                                     .unwrap();
//                             } else {
//                                 let old_player_action_state = old_player_action_state.unwrap();
//                                 if old_player_action_state.action_type
//                                     != player_action_state.action_type
//                                 {
//                                     broadcast_tx
//                                         .send(WebSocketMessages::PlayerActionStateChangeName(
//                                             player_action_state.action_type.get_action_name(),
//                                             player_action_state.entity_id,
//                                         ))
//                                         .unwrap();
//                                 }
//                             }
//
//                             broadcast_tx
//                                 .send(WebSocketMessages::PlayerActionState(
//                                     player_action_state.clone(),
//                                 ))
//                                 .unwrap();
//
//                             global_app_state
//                                 .player_action_state
//                                 .insert(player_action_state.entity_id, player_action_state.clone());
//                         }
//                     }
//                 }
//
//                 if table_name == "location_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let location_state: entity::location::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(location_state) => location_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert location_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .location_state
//                                 .insert(location_state.entity_id, location_state.clone());
//                         }
//                     }
//                 }
//
//                 metrics::histogram!(
//                     "bitraft_event_handler_transaction_update_duration_seconds",
//                     &[("table", table_name.to_string())]
//                 )
//                     .record(start.elapsed().as_secs_f64());
//             }
//
//             debug!("Received {count} events");
//             evenets.clear();
//             tokio::time::sleep(Duration::from_millis(50)).await;
//         }
//     });
// }

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
    // ClaimDescriptionState(entity::claim_description_state::Model),
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
            // WebSocketMessages::ClaimDescriptionState(claim) => {
            //     Some(vec![("claim".to_string(), claim.entity_id)])
            // }
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
