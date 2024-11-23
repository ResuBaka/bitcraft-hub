mod buildings;
mod cargo_desc;
mod claim_tech_desc;
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
mod deployable_state;
mod trading_orders;

use axum::{
    http::StatusCode,
    middleware,
    routing::{get, get_service},
    Router,
};
use reqwest_websocket::{Message, RequestBuilderExt};
use service::sea_orm::{Database, DatabaseConnection};
use std::collections::hash_map::Iter;
use std::collections::HashMap;
use tower_http::cors::{Any, CorsLayer};

use crate::config::Config;
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
use std::fs::File;
use std::future::Future;
use std::io::Write;
use std::ops::Add;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use tokio_util::sync::CancellationToken;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use entity::trade_order;

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

    let client = create_default_client(config.clone());
    
    // donwload_all_Tables(
    //     &client,
    //     &config.spacetimedb.domain.clone(),
    //     &config.spacetimedb.protocol.clone(),
    //     &config.spacetimedb.database.clone(),
    //     &config.storage_path.clone().into(),
    // ).await;

    if config.import_enabled {
        import_data(config.clone());
    }

    let state = AppState {
        conn,
        storage_path: PathBuf::from(config.storage_path.clone()),
    };

    let app = create_app(&config, state);

    let websocket_url = config.weboosocket_url();
    let websocket_password = config.spacetimedb.password.clone();
    let websocket_username = config.spacetimedb.username.clone();
    let database_name = config.spacetimedb.database.clone();

    if config.live_updates_ws {
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

fn create_app(config: &Config, state: AppState) -> Router {
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
        .default_headers(default_header)
        .build()
        .unwrap()
}

async fn create_importer_default_db_connection(config: Config) -> DatabaseConnection {
    let mut connection_options = ConnectOptions::new(config.database.url.clone());
    connection_options
        .max_connections(10)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
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

                let temp_config = config.clone();
                tasks.push(tokio::spawn(skill_descriptions::import_job_skill_desc(temp_config)));

                let temp_config = config.clone();
                tasks.push(tokio::spawn(player_state::import_job_player_state(temp_config)));

                let temp_config = config.clone();
                tasks.push(tokio::spawn(player_state::import_job_player_username_state(temp_config)));

                let temp_config = config.clone();
                tasks.push(tokio::spawn(leaderboard::import_job_experience_state(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(claim_tech_state::import_job_claim_tech_state(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(claim_tech_desc::import_job_claim_tech_desc(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(claims::import_job_claim_description_state(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(items::import_job_item_desc(temp_config)));                
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(cargo_desc::import_job_cargo_desc(temp_config)));               
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(inventory::import_job_item_desc(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(deployable_state::import_job_deployable_state(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(buildings::import_job_building_desc(temp_config)));
                
                let temp_config = config.clone();
                tasks.push(tokio::spawn(buildings::import_job_building_state(temp_config)));

                futures::future::join_all(tasks).await;
            });
    });
}

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

enum WorkerMessage {
    Work(WebSocketMessage),
    Shutdown,
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

pub async fn donwload_all_Tables(
    client: &reqwest::Client,
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
        "WeaponTypeDesc"
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
        "VaultState"
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
        "TradeSessionLoopTimer"
    ];
    
    for table in desc_tables {
        download_all_Table(client, domain, protocol, database, table, storage_path, "desc").await;
    }
    
    for table in state_tables {
        download_all_Table(client, domain, protocol, database, table, storage_path, "state").await;
    }
    
    for table in rest_tables {
        download_all_Table(client, domain, protocol, database, table, storage_path, "rest").await;
    }

}

///
/// Donwload the table and save it to the storage path with the type as the folder before the name
pub async fn download_all_Table(
    client: &reqwest::Client,
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
        },
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
