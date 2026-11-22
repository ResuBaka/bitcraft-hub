pub(crate) mod subscriptions;

use crate::AppState;
use crate::auction_listing_state::bitcraft::{
    start_worker_buy_order_state, start_worker_sell_order_state,
};
use crate::buildings::bitcraft::{
    start_worker_building_desc, start_worker_building_nickname_state, start_worker_building_state,
};
use crate::cargo_desc::bitcraft::start_worker_cargo_desc;
use crate::claims::bitcraft;
use crate::collectible_desc::bitcraft::start_worker_collectible_desc;
use crate::config::Config;
use crate::crafting_recipe_desc::bitcraft::start_worker_crafting_recipe_desc;
use crate::deployable_state::bitcraft::start_worker_deployable_state;
use crate::extraction_recipe_desc::bitcraft::start_worker_extraction_recipe_desc;
use crate::houses::bitcraft::{
    start_worker_dimension_description_state, start_worker_interior_network_desc,
    start_worker_permission_state, start_worker_player_housing_state, start_worker_portal_state,
};
use crate::inventory::bitcraft as inventory_bitcraft;
use crate::item_list_desc::bitcraft::start_worker_item_list_desc;
use crate::items::bitcraft::start_worker_item_desc;
use crate::resource_desc::bitcraft::start_worker_resource_desc;

use self::worker_channel::UnboundedSender;
use crate::crafting::bitcraft::start_worker_progressive_action_state;
use crate::mobile_entity_state::bitcraft::start_worker_mobile_entity_state;
use crate::npc_desc::bitcraft::start_worker_npc_desc;
use crate::skill_descriptions::bitcraft::start_worker_skill_desc;
use crate::trading_orders::bitcraft::start_worker_trade_order_state;
use crate::traveler_task_desc::bitcraft::start_worker_traveler_task_desc;
use crate::traveler_task_state::bitcraft::start_worker_traveler_task_state;
use crate::user_state::bitcraft::start_worker_user_state;
use crate::vault_state::bitcraft::start_worker_vault_state_collectibles;
use crate::websocket::batched_worker::BatchedWorker;
use game_module::module_bindings::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::__codegen::{self as __sdk};
use spacetimedb_sdk::{Compression, DbContext, Error, Event, Table, TableWithPrimaryKey};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use tokio::time::Instant;
use ts_rs::TS;
use entity::shared::Region;

pub(crate) mod batched_worker;
pub(crate) mod initial_batches;
pub(crate) mod initial_sync;
mod metric_handles;
pub(super) mod outbound;
mod claim_locations;
mod leaderboard_staff;
pub(crate) mod player_stats;
mod player_activity;
#[expect(dead_code, reason = "Full overworld subscriptions are currently disabled")]
mod overworld;
#[expect(dead_code, reason = "Portal dimension subscriptions are currently disabled")]
mod portal_locations;
mod startup;
pub(crate) mod snapshot_write;
pub(crate) use metric_handles::WorkerReceiveMetrics;
use metric_handles::{ListenerMetrics, WorkerSendMetrics};
pub(crate) mod worker_channel;

const MAX_RECONNECT_RETRIES: u32 = 15;
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(120);

fn initialize_connection_metrics(region: &str) {
    metrics::gauge!("bitcraft_database_connected", "region" => region.to_owned()).set(0);
    // Zero means that this process has not yet connected to the region.
    metrics::gauge!("bitcraft_database_last_connected_timestamp_seconds", "region" => region.to_owned()).set(0);
}

fn record_connection_metrics(region: &str, timestamp: f64) {
    metrics::gauge!("bitcraft_database_connected", "region" => region.to_owned()).set(1);
    metrics::gauge!("bitcraft_database_last_connected_timestamp_seconds", "region" => region.to_owned()).set(timestamp);
}

#[derive(Clone, Default)]
struct ConnectionTiming(Arc<Mutex<Option<Instant>>>, Arc<tokio::sync::Notify>);

impl ConnectionTiming {
    fn connected(&self) {
        *self.0.lock().unwrap() = Some(Instant::now());
        self.1.notify_one();
    }

    fn uptime(&self) -> Option<Duration> {
        self.0.lock().unwrap().map(|connected| connected.elapsed())
    }

    async fn wait_connected(&self) {
        // Used only for the diagnostic path with no subscription callback.
        let notified = self.1.notified();
        if self.uptime().is_none() {
            notified.await;
        }
    }
}

fn next_reconnect_failure_count(previous: u32, uptime: Option<Duration>) -> u32 {
    if uptime.is_some_and(|uptime| uptime >= STABLE_CONNECTION_DURATION) {
        0
    } else {
        previous.saturating_add(1)
    }
}

fn reconnect_delay(tries: u32, base_wait_secs: u64) -> Duration {
    Duration::from_secs((u64::from(tries.max(1)) * base_wait_secs.clamp(15, 45)).min(60))
}

fn send_worker_message<T>(
    worker_name: &str,
    tx: &UnboundedSender<SpacetimeUpdateMessages<T>>,
    message: SpacetimeUpdateMessages<T>,
) {
    metrics::counter!(
        "worker_queue_sent_total",
        &[("worker", worker_name.to_string())]
    )
    .increment(1);
    if tx.send(message).is_err() {
        metrics::counter!(
            "worker_queue_send_errors_total",
            &[("worker", worker_name.to_string())]
        )
        .increment(1);
    }
}

fn connect_to_db(
    builder: __sdk::DbConnectionBuilder<RemoteModule>,
    global_app_state: AppState,
    token: String,
    db_name: &str,
    db_host: &str,
    timing: ConnectionTiming,
    region_number: Option<Region>,
) -> spacetimedb_sdk::Result<DbConnection> {
    let tmp_global_app_state = global_app_state.clone();
    let tmp_disconnect_global_app_state = global_app_state.clone();
    let tmp_db_name = db_name.to_owned();
    let tmp_disconnect_db_name = tmp_db_name.clone();

    builder
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(move |_ctx, identity, _token| {
            timing.connected();
            tracing::debug!(
                region = tmp_db_name,
                "Connected to server {tmp_db_name} with {identity}"
            );
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs_f64())
                .unwrap_or(0.0);
            record_connection_metrics(&tmp_db_name, timestamp);

            tmp_global_app_state
                .connection_state
                .insert(tmp_db_name.clone(), true);
        })
        .with_row_deduplication(true)
        // Register our `on_connect_error` callback, which will print a message, then exit the process.
        .on_connect_error(on_connect_error)
        // Our `on_disconnect` callback, which will print a message, then exit the process.
        .on_disconnect(move |_ctx, err| {
            if let Some(region_number) = region_number {
                tmp_disconnect_global_app_state.claim_locations.invalidate(region_number);
                tmp_disconnect_global_app_state.player_stats.invalidate(region_number);
                tmp_disconnect_global_app_state.player_equipment.invalidate(region_number);
                tmp_disconnect_global_app_state.player_activity.invalidate(region_number);
                player_activity::changed(&tmp_disconnect_global_app_state, None);
                tmp_disconnect_global_app_state.player_stats.invalidate_food(region_number);
                tmp_disconnect_global_app_state.player_stats.clear_tools(region_number);
                player_stats::changed(&tmp_disconnect_global_app_state, None);
                player_stats::character_changed(&tmp_disconnect_global_app_state, None, None);
            }
            metrics::gauge!(
                "bitcraft_database_connected",
                &[("region", tmp_disconnect_db_name.clone())]
            )
            .set(0);

            tmp_disconnect_global_app_state
                .connection_state
                .insert(tmp_disconnect_db_name.clone(), false);
            if let Some(err) = err {
                tracing::error!(
                    region = tmp_disconnect_db_name,
                    "Disconnected: {} : {}",
                    err,
                    tmp_disconnect_db_name
                );
                // std::process::exit(1);
            } else {
                tracing::error!(
                    region = tmp_disconnect_db_name,
                    "Disconnected {}.",
                    tmp_disconnect_db_name
                );
                // std::process::exit(0);
            }
        })
        // Authenticate with the configured token when one is provided.
        .with_token(Some(token.trim()).filter(|token| !token.is_empty()))
        // Set the database name we chose when we called `spacetime publish`.
        .with_database_name(db_name)
        // Set the URI of the SpacetimeDB host that's running our database.
        .with_uri(db_host)
        // Finalize configuration and connect!
        .with_compression(Compression::Gzip)
        .build()
}

/// Our `on_connect_error` callback: print the error, then exit the process.
fn on_connect_error(_ctx: &ErrorContext, err: Error) {
    tracing::warn!("Connection error: {:?}", err);
    // std::process::exit(1);
}

macro_rules! setup_spacetime_db_listeners {
    ($ctx:expr, $db_table_method:ident, $tx_channel:ident, $state_type:ty, $database_name_expr:expr, $database_region:expr, $worker_name:expr $(, $initial_batch_rows:expr)?) => {
        let table_name_str = stringify!($db_table_method);
        let database_name_runtime_string = $database_name_expr.to_string();

        let temp_tx = $tx_channel.clone();
        let event_metrics = ListenerMetrics::new(table_name_str, database_name_runtime_string.clone(), "update");
        let send_metrics = WorkerSendMetrics::new($worker_name);
        $ctx.db.$db_table_method().on_update(
            // Use $state_type for the old and new parameters
            move |ctx: &EventContext, old: &$state_type, new: &$state_type| {
                event_metrics.record(&ctx.event);

                send_metrics.send(
                    &temp_tx,
                    SpacetimeUpdateMessages::Update {
                        event: None,
                        database_name: $database_region,
                        old: old.clone(),
                        new: new.clone(),
                    },
                );
            },
        );

        let temp_tx = $tx_channel.clone();
        let event_metrics = ListenerMetrics::new(table_name_str, database_name_runtime_string.clone(), "insert");
        let send_metrics = WorkerSendMetrics::new($worker_name);
        $ctx.db.$db_table_method().on_insert(
            // Use $state_type for the new parameter
            move |ctx: &EventContext, new: &$state_type| {
                event_metrics.record(&ctx.event);

                if let Event::SubscribeApplied = ctx.event {
                    return;
                }

                send_metrics.send(
                    &temp_tx,
                    SpacetimeUpdateMessages::Insert {
                        event: None,
                        database_name: $database_region,
                        new: new.clone(),
                    },
                );
            },
        );

        let temp_tx = $tx_channel.clone();
        let event_metrics = ListenerMetrics::new(table_name_str, database_name_runtime_string.clone(), "initial");
        let initial_batch_rows = 0usize $(+ $initial_batch_rows)?;
        $ctx.db.on_initial_owned::<$state_type>(
            $worker_name,
            // Use $state_type for the new parameter
            move |ctx: &EventContext, new: Vec<$state_type>| {
                event_metrics.record(&ctx.event);

                if let Event::SubscribeApplied = ctx.event {
                } else {
                    return;
                }

                initial_batches::send_initial(
                    $worker_name,
                    &temp_tx,
                    new,
                    $database_region,
                    initial_batch_rows,
                );
            },
        );

        let temp_tx = $tx_channel.clone();
        let event_metrics = ListenerMetrics::new(table_name_str, database_name_runtime_string.clone(), "delete");
        let send_metrics = WorkerSendMetrics::new($worker_name);
        $ctx.db.$db_table_method().on_delete(
            // Use $state_type for the new parameter
            move |ctx: &EventContext, new: &$state_type| {
                event_metrics.record(&ctx.event);

                send_metrics.send(
                    &temp_tx,
                    SpacetimeUpdateMessages::Remove {
                        event: None,
                        database_name: $database_region,
                        delete: new.clone(),
                    },
                );
            },
        );
    };
}

async fn run_connection_until_shutdown(
    ctx: &DbConnection,
    stop: &tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    tokio::select! { biased;
        _ = stop.cancelled() => {
            // disconnect() queues a mutation; keep driving the SDK to apply it.
            if ctx.is_active() { ctx.disconnect()?; }
            ctx.run_async().await.map_err(anyhow::Error::from)
        },
        result = ctx.run_async() => result.map_err(anyhow::Error::from),
    }
}

/// Filter before subscribing, not in callbacks: disabled tables must never be
/// downloaded or retained in the SDK cache. Raw SQL names are explicit so joins
/// cannot accidentally bypass the diagnostic switch.
/// TEMPORARY: isolate connection/worker overhead with zero subscriptions.
fn subscription_queries(
    tables: &[&str],
    sql: &[(&str, &str)],
    remove_desc: bool,
    skip_state_tables: bool,
) -> Vec<String> {
    // Keep the normal subscription path below so skip_state_tables=false still
    // restores full synchronization for comparison.
    if skip_state_tables {
        return Vec::new();
    }
    tables
        .iter()
        .filter(|table| !remove_desc || !table.contains("_desc"))
        .filter(|table| !skip_state_tables || table.ends_with("_desc"))
        .map(|table| format!("select * from {table}"))
        .chain(
            sql.iter()
                .filter(|(table, _)| !skip_state_tables || table.ends_with("_desc"))
                .map(|(_, query)| (*query).to_owned()),
        )
        .collect()
}

#[allow(clippy::too_many_arguments)]
async fn connect_to_db_logic(
    global_app_state: AppState,
    config: &Config,
    database: &str,
    remove_desc: &bool,
    timing: ConnectionTiming,
    startup_permit: &mut Option<tokio::sync::OwnedSemaphorePermit>,
    startup_done: &mut Option<tokio::sync::oneshot::Sender<()>>,
    claim_event_tx: &tokio::sync::mpsc::Sender<entity::claim_event::ActiveModel>,
    craft_activity_tx: &UnboundedSender<entity::craft_activity::ActiveModel>,
    passive_craft_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PassiveCraftState>>,
    mobile_entity_state_tx: &UnboundedSender<SpacetimeUpdateMessages<MobileEntityState>>,
    player_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerState>>,
    player_username_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    experience_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    inventory_state_tx: &UnboundedSender<SpacetimeUpdateMessages<InventoryState>>,
    item_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ItemDesc>>,
    cargo_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CargoDesc>>,
    vault_state_collectibles_tx: &UnboundedSender<SpacetimeUpdateMessages<VaultState>>,
    deployable_state_tx: &UnboundedSender<SpacetimeUpdateMessages<DeployableStateV2>>,
    claim_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimState>>,
    claim_local_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimLocalState>>,
    claim_member_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimMemberState>>,
    skill_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<SkillDesc>>,
    claim_tech_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechState>>,
    claim_tech_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechDesc>>,
    building_state_tx: &UnboundedSender<SpacetimeUpdateMessages<BuildingState>>,
    building_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<BuildingDesc>>,
    building_nickname_state_tx: &UnboundedSender<SpacetimeUpdateMessages<BuildingNicknameState>>,
    crafting_recipe_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CraftingRecipeDesc>>,
    item_list_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ItemListDesc>>,
    traveler_task_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<TravelerTaskDesc>>,
    traveler_task_state_tx: &UnboundedSender<SpacetimeUpdateMessages<TravelerTaskState>>,
    trade_order_state_tx: &UnboundedSender<SpacetimeUpdateMessages<TradeOrderState>>,
    user_state_tx: &UnboundedSender<SpacetimeUpdateMessages<UserState>>,
    npc_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<NpcDesc>>,
    buy_order_state_tx: &UnboundedSender<SpacetimeUpdateMessages<AuctionListingState>>,
    sell_order_state_tx: &UnboundedSender<SpacetimeUpdateMessages<AuctionListingState>>,
    collectible_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CollectibleDesc>>,
    interior_network_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<InteriorNetworkDesc>>,
    dimension_description_state_tx: &UnboundedSender<
        SpacetimeUpdateMessages<DimensionDescriptionState>,
    >,
    player_housing_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerHousingState>>,
    permission_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PermissionState>>,
    portal_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PortalState>>,
    location_state_tx: &UnboundedSender<SpacetimeUpdateMessages<LocationState>>,
    resource_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ResourceDesc>>,
    extraction_recipe_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ExtractionRecipeDesc>>,
    progressive_action_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ProgressiveActionState>>,
    public_progressive_action_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PublicProgressiveActionState>>,
) -> anyhow::Result<()> {
    if global_app_state.shutdown.stop.is_cancelled() {
        return Ok(());
    }

    let region_number = database
        .split("-")
        .last()
        .unwrap()
        .parse::<entity::shared::Region>()?;

    let mut builder = DbConnection::builder();
    if let Some(dir) = &config.spacetimedb.region_capture_dir {
        // The SDK requires a fresh file for each connection, including retries.
        let path = dir.join(format!("region-{region_number}-{:016x}.capture", rand::random::<u64>()));
        tracing::info!(region = database, path = %path.display(), "Recording regional BSATN messages");
        builder = builder.with_bsatn_capture(path, 512 * 1024 * 1024);
    }
    let ctx = connect_to_db(
        builder,
        global_app_state.clone(),
        config.spacetimedb.password.clone(),
        database,
        config.spacetimedb_url(region_number).as_ref(),
        timing.clone(),
        Some(region_number),
    )?;

    if !ctx.is_active() {
        tracing::error!(
            region = database,
            "Could not connect to the bitcraft server {} with module {database}",
            config.spacetimedb_url(region_number)
        );
        anyhow::bail!("Connection to {database} is inactive");
    }

    global_app_state.claim_locations.reset(region_number);
    claim_locations::listen(&ctx, region_number, global_app_state.claim_locations.clone());
    global_app_state.player_stats.reset(region_number);
    global_app_state.player_activity.reset(region_number);
    player_activity::listen(&ctx, region_number, global_app_state.clone());
    if !*remove_desc && !config.spacetimedb.skip_state_tables {
        global_app_state.player_stats.begin_food(region_number);
        global_app_state.player_stats.begin_tools(region_number);
    }
    player_stats::listen(&ctx, region_number, global_app_state.clone());
    if !config.spacetimedb.skip_state_tables {
        crate::player_state::equipment::listen(&ctx, region_number, &global_app_state);
    }

    crate::claims::events::register_listeners(
        &ctx,
        global_app_state.clone(),
        region_number,
        claim_event_tx.clone(),
    );
    crate::crafting::events::register_listeners(&ctx, region_number, craft_activity_tx.clone());

    setup_spacetime_db_listeners!(
        ctx,
        mobile_entity_state,
        mobile_entity_state_tx,
        MobileEntityState,
        database,
        region_number,
        "mobile_entity_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        player_state,
        player_state_tx,
        PlayerState,
        database,
        region_number,
        "player_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        player_username_state,
        player_username_state_tx,
        PlayerUsernameState,
        database,
        region_number,
        "player_username_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        experience_state,
        experience_state_tx,
        ExperienceState,
        database,
        region_number,
        "experience_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        inventory_state,
        inventory_state_tx,
        InventoryState,
        database,
        region_number,
        "inventory_state",
        config.spacetimedb.initial_batch_rows
    );
    setup_spacetime_db_listeners!(
        ctx,
        item_desc,
        item_desc_tx,
        ItemDesc,
        database,
        region_number,
        "item_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        cargo_desc,
        cargo_desc_tx,
        CargoDesc,
        database,
        region_number,
        "cargo_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        vault_state,
        vault_state_collectibles_tx,
        VaultState,
        database,
        region_number,
        "vault_state",
        config.spacetimedb.initial_batch_rows
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_state,
        claim_state_tx,
        ClaimState,
        database,
        region_number,
        "claim_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        deployable_state_v_2,
        deployable_state_tx,
        DeployableStateV2,
        database,
        region_number,
        "deployable_state_v2"
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_local_state,
        claim_local_state_tx,
        ClaimLocalState,
        database,
        region_number,
        "claim_local_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_member_state,
        claim_member_state_tx,
        ClaimMemberState,
        database,
        region_number,
        "claim_member_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        skill_desc,
        skill_desc_tx,
        SkillDesc,
        database,
        region_number,
        "skill_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_tech_state,
        claim_tech_state_tx,
        ClaimTechState,
        database,
        region_number,
        "claim_tech_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_tech_desc,
        claim_tech_desc_tx,
        ClaimTechDesc,
        database,
        region_number,
        "claim_tech_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        building_state,
        building_state_tx,
        BuildingState,
        database,
        region_number,
        "building_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        building_desc,
        building_desc_tx,
        BuildingDesc,
        database,
        region_number,
        "building_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        location_state,
        location_state_tx,
        LocationState,
        database,
        region_number,
        "location_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        building_nickname_state,
        building_nickname_state_tx,
        BuildingNicknameState,
        database,
        region_number,
        "building_nickname_state"
    );

    setup_spacetime_db_listeners!(
        ctx,
        crafting_recipe_desc,
        crafting_recipe_desc_tx,
        CraftingRecipeDesc,
        database,
        region_number,
        "crafting_recipe_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        item_list_desc,
        item_list_desc_tx,
        ItemListDesc,
        database,
        region_number,
        "item_list_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        traveler_task_desc,
        traveler_task_desc_tx,
        TravelerTaskDesc,
        database,
        region_number,
        "traveler_task_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        traveler_task_state,
        traveler_task_state_tx,
        TravelerTaskState,
        database,
        region_number,
        "traveler_task_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        trade_order_state,
        trade_order_state_tx,
        TradeOrderState,
        database,
        region_number,
        "trade_order_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        sell_order_state,
        sell_order_state_tx,
        AuctionListingState,
        database,
        region_number,
        "sell_order_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        buy_order_state,
        buy_order_state_tx,
        AuctionListingState,
        database,
        region_number,
        "buy_order_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        npc_desc,
        npc_desc_tx,
        NpcDesc,
        database,
        region_number,
        "npc_desc"
    );

    setup_spacetime_db_listeners!(
        ctx,
        user_state,
        user_state_tx,
        UserState,
        database,
        region_number,
        "user_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        collectible_desc,
        collectible_desc_tx,
        CollectibleDesc,
        database,
        region_number,
        "collectible_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        interior_network_desc,
        interior_network_desc_tx,
        InteriorNetworkDesc,
        database,
        region_number,
        "interior_network_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        dimension_description_state,
        dimension_description_state_tx,
        DimensionDescriptionState,
        database,
        region_number,
        "dimension_description_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        player_housing_state,
        player_housing_state_tx,
        PlayerHousingState,
        database,
        region_number,
        "player_housing_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        permission_state,
        permission_state_tx,
        PermissionState,
        database,
        region_number,
        "permission_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        portal_state,
        portal_state_tx,
        PortalState,
        database,
        region_number,
        "portal_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        resource_desc,
        resource_desc_tx,
        ResourceDesc,
        database,
        region_number,
        "resource_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        extraction_recipe_desc,
        extraction_recipe_desc_tx,
        ExtractionRecipeDesc,
        database,
        region_number,
        "extraction_recipe_desc"
    );
    setup_spacetime_db_listeners!(
        ctx,
        progressive_action_state,
        progressive_action_state_tx,
        ProgressiveActionState,
        database,
        region_number,
        "progressive_action_state"
    );
    setup_spacetime_db_listeners!(
        ctx,
        public_progressive_action_state,
        public_progressive_action_state_tx,
        PublicProgressiveActionState,
        database,
        region_number,
        "public_progressive_action_state"
    );
    setup_spacetime_db_listeners!(ctx, passive_craft_state, passive_craft_state_tx, PassiveCraftState, database, region_number, "passive_craft_state");

    let tables_to_subscribe = vec![
        // "user_state",
        // "mobile_entity_state",
        "claim_tile_state",
        // "combat_action_desc",
        "item_desc",
        "item_list_desc",
        "cargo_desc",
        // "equipment_desc",
        // "player_action_state",
        "crafting_recipe_desc",
        // "action_state",
        "player_state",
        // "equipment_state",
        "skill_desc",
        "player_username_state",
        "building_desc",
        "building_state",
        "building_nickname_state",
        "vault_state",
        "experience_state",
        "claim_tech_state",
        "claim_state",
        "claim_member_state",
        "claim_local_state",
        "deployable_state_v2",
        "claim_treasury_event",
        "inventory_state",
        "collectible_desc",
        "claim_tech_desc",
        "resource_desc",
        "extraction_recipe_desc",
        "progressive_action_state",
        "public_progressive_action_state",
        "craft_event",
        "craft_completed_event",
        "passive_craft_state",
        // "claim_description_state", -> claim_state
        // "location_state where dimension = 1", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN player_state ps ON player_state.entity_id = location_state.entity_id", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN building_state ps ON building_state.entity_id = building_state.entity_id", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN deployable_state ps ON deployable_state.entity_id = deployable_state.entity_id", // This currently takes to much cpu to run
        "traveler_task_desc",
        "traveler_task_state",
        // "trade_order_state",
        "buy_order_state",
        "sell_order_state",
        "npc_desc",
        "interior_network_desc",
        "dimension_description_state",
        "dimension_network_state",
        "player_housing_state",
        "permission_state",
        "portal_state",
    ];

    let sql_subscribe = vec![
        ("equipment_desc", "SELECT * FROM equipment_desc"),
        ("equipment_state", "SELECT * FROM equipment_state"),
        ("location_state", "SELECT location_state.* FROM location_state JOIN building_state ON building_state.entity_id = location_state.entity_id"),
        ("location_state", "SELECT location_state.* FROM location_state JOIN portal_state ON portal_state.entity_id = location_state.entity_id"),
        ("location_state", "SELECT location_state.* FROM location_state JOIN claim_tile_state ON claim_tile_state.entity_id = location_state.entity_id"),
        // ("location_state", "SELECT * FROM location_state where dimension = 1"),
        (
            "mobile_entity_state",
            "SELECT mobile_entity_state.* FROM mobile_entity_state JOIN player_state ON player_state.entity_id = mobile_entity_state.entity_id",
        ),
    ];

    if config.spacetimedb.skip_state_tables {
        tracing::warn!(
            region = database,
            "RSS DIAGNOSTIC: zero table subscriptions; connection and workers only (spacetimedb.skip_state_tables=true)"
        );
    }


    let (initial_tx, initial_rx) = tokio::sync::oneshot::channel();
    let initial_completion = Arc::new(Mutex::new(Some(initial_tx)));
    let initial_failed = initial_completion.clone();
    let claim_locations = global_app_state.claim_locations.clone();
    let claim_locations_on_error = global_app_state.claim_locations.clone();
    let claim_locations_enabled = !config.spacetimedb.skip_state_tables;
    let equipment_on_applied = global_app_state.player_equipment.clone();
    let equipment_on_error = global_app_state.player_equipment.clone();
    let subscription_builder = ctx
        .subscription_builder()
        .on_applied(move |_ctx: &SubscriptionEventContext| {
            if claim_locations_enabled {
                claim_locations.finish_initial(region_number);
                equipment_on_applied.ready(region_number);
            }
            if let Some(tx) = initial_completion.lock().unwrap().take() {
                let _ = tx.send(Ok(()));
            }
        })
        .on_error(move |ctx: &ErrorContext, err: Error| {
            claim_locations_on_error.invalidate(region_number);
            equipment_on_error.invalidate(region_number);
            tracing::warn!(
                region = region_number,
                "Subscription failed: {} {:?}",
                err,
                ctx.event
            );
            if let Some(tx) = initial_failed.lock().unwrap().take() {
                let _ = tx.send(Err(anyhow::anyhow!("Initial subscription failed: {err}")));
            }
        });
    let queries = subscription_queries(
        &tables_to_subscribe,
        &sql_subscribe,
        *remove_desc,
        config.spacetimedb.skip_state_tables,
    );
    if !config.spacetimedb.skip_state_tables {
        crate::player_state::equipment::subscribe_presets(&ctx, region_number, &global_app_state);
        let activity_on_applied = global_app_state.clone();
        let activity_on_error = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| {
                activity_on_applied.player_activity.ready(region_number);
                player_activity::changed(&activity_on_applied, None);
            })
            .on_error(move |_: &ErrorContext, err: Error| {
                activity_on_error.player_activity.invalidate(region_number);
                player_activity::changed(&activity_on_error, None);
                tracing::warn!(region = region_number, "Player activity subscription failed: {err}");
            })
            .subscribe(["SELECT player_action_state.* FROM player_action_state JOIN player_state ON player_state.entity_id = player_action_state.entity_id"]);
        let stats_on_applied = global_app_state.clone();
        let stats_on_error = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| {
                stats_on_applied.player_stats.ready(region_number);
                player_stats::changed(&stats_on_applied, None);
            })
            .on_error(move |_: &ErrorContext, err: Error| {
                stats_on_error.player_stats.invalidate(region_number);
                player_stats::changed(&stats_on_error, None);
                player_stats::character_changed(&stats_on_error, None, None);
                tracing::warn!(region = region_number, "Player stats subscription failed: {err}");
            })
            .subscribe([
                "SELECT health_state.* FROM health_state JOIN player_state ON player_state.entity_id = health_state.entity_id",
                "SELECT stamina_state.* FROM stamina_state JOIN player_state ON player_state.entity_id = stamina_state.entity_id",
                "SELECT active_buff_state.* FROM active_buff_state JOIN player_state ON player_state.entity_id = active_buff_state.entity_id",
            ]);
        let character_stats_on_applied = global_app_state.clone();
        let character_stats_on_error = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| {
                player_stats::changed(&character_stats_on_applied, None);
                player_stats::character_changed(&character_stats_on_applied, None, None);
            })
            .on_error(move |_: &ErrorContext, err: Error| {
                character_stats_on_error.player_stats.clear_character_stats(region_number);
                player_stats::changed(&character_stats_on_error, None);
                player_stats::character_changed(&character_stats_on_error, None, None);
                tracing::warn!(region = region_number, "Player character stats subscription failed: {err}");
            })
            .subscribe(["SELECT character_stats_state.* FROM character_stats_state JOIN player_state ON player_state.entity_id = character_stats_state.entity_id"]);
    }
    if !*remove_desc && !config.spacetimedb.skip_state_tables {
        let stats_on_applied = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| player_stats::changed(&stats_on_applied, None))
            .on_error(move |_: &ErrorContext, err: Error| {
                tracing::warn!(region = region_number, "Buff descriptions subscription failed: {err}");
            })
            .subscribe(["SELECT * FROM buff_desc", "SELECT * FROM buff_type_desc"]);
        let tools_on_applied = global_app_state.clone();
        let tools_on_error = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| {
                tools_on_applied.player_stats.update_tools(region_number, |index| index.ready = true);
                player_stats::character_changed(&tools_on_applied, None, None);
            })
            .on_error(move |_: &ErrorContext, err: Error| {
                tools_on_error.player_stats.clear_tools(region_number);
                player_stats::character_changed(&tools_on_error, None, None);
                tracing::warn!(region = region_number, "Tool descriptions subscription failed: {err}");
            })
            .subscribe(["SELECT * FROM tool_desc", "SELECT * FROM tool_type_desc"]);
        let food_on_applied = global_app_state.clone();
        let food_on_error = global_app_state.clone();
        ctx.subscription_builder()
            .on_applied(move |_: &SubscriptionEventContext| {
                food_on_applied.player_stats.finish_food(region_number);
                player_stats::changed(&food_on_applied, None);
            })
            .on_error(move |_: &ErrorContext, err: Error| {
                food_on_error.player_stats.invalidate_food(region_number);
                player_stats::changed(&food_on_error, None);
                tracing::warn!(region = region_number, "Food descriptions subscription failed: {err}");
            })
            .subscribe(["SELECT * FROM food_desc"]);
    }
    // Do not send an empty subscription request: keep the connection running
    // without subscriptions when isolating connection/worker RSS.
    let _portal_dimensions = (!config.spacetimedb.skip_state_tables)
        .then(|| portal_locations::listen(&ctx));
    let has_subscription = !queries.is_empty();
    if has_subscription {
        subscription_builder.subscribe(queries);
    }

    let tmp_disconnect_db_name = database.to_string().clone();
    let tmp_disconnect_global_app_state = global_app_state.clone();

    // The SDK must keep processing events while we wait for subscriptions to apply.
    let connection_stop = global_app_state.shutdown.stop.child_token();
    let connection = run_connection_until_shutdown(&ctx, &connection_stop);
    let result = startup::run(
        connection,
        async {
            if has_subscription {
                initial_rx
                    .await
                    .map_err(|_| anyhow::anyhow!("Initial subscription ended before applying"))??;
            } else {
                timing.wait_connected().await;
            }
            tracing::info!(region = database, "Connection startup complete; releasing startup slot");
            if let Some(done) = startup_done.take() {
                let _ = done.send(());
            }
            // if let Some(dimensions) = _portal_dimensions {
            //     tokio::try_join!(
            //         overworld::subscribe(&ctx),
            //         // portal_locations::subscribe(&ctx, dimensions),
            //     )?;
            // }
            Ok::<(), anyhow::Error>(())
        },
        &connection_stop,
        startup_permit,
    )
    .await;

    global_app_state.player_equipment.invalidate(region_number);
    metrics::gauge!(
        "bitcraft_database_connected",
        &[("region", tmp_disconnect_db_name.clone())]
    )
    .set(0);

    tmp_disconnect_global_app_state
        .connection_state
        .insert(tmp_disconnect_db_name.clone(), false);

    result
}

pub fn start_websocket_bitcraft_logic(config: Config, global_app_state: AppState) {
    let db_flush_delay = Duration::from_millis(config.database.worker_flush_ms);
    if !config.spacetimedb.skip_state_tables && !config.spacetimedb.database.trim().is_empty() {
        leaderboard_staff::start(&config, global_app_state.clone());
    }
    tracing::info!(
        backend = worker_channel::BACKEND,
        "Starting worker input queues"
    );
    global_app_state.shutdown.clone().spawn_connection(async move {
        if global_app_state.shutdown.stop.is_cancelled() { return; }
        let (mobile_entity_state_tx, mobile_entity_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (user_state_tx, user_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let player_state_worker = crate::player_state::bitcraft::PlayerStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );

        let player_username_state_worker =
            crate::player_state::bitcraft::PlayerUsernameStateWorker::new(
                global_app_state.clone(),
                3000,
                db_flush_delay,
            );

        let experience_state_worker = crate::leaderboard::bitcraft::ExperienceStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );

        let location_state_worker = crate::location_state::bitcraft::LocationStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );
        let inventory_state_worker = inventory_bitcraft::InventoryStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
            config.database.inventory_snapshot_write_bytes,
        );

        let (item_desc_tx, item_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (item_list_desc_tx, item_list_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (cargo_desc_tx, cargo_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (vault_state_collectibles_tx, vault_state_collectibles_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (deployable_state_tx, deployable_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let claim_state_worker = bitcraft::ClaimStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );

        let claim_local_state_worker = bitcraft::ClaimLocalStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );

        let claim_member_state_worker = bitcraft::ClaimMemberStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );

        let (skill_desc_tx, skill_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let claim_tech_state_worker = bitcraft::ClaimTechStateWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );
        let claim_tech_desc_worker = bitcraft::ClaimTechDescWorker::new(
            global_app_state.clone(),
            3000,
            db_flush_delay,
        );
        let (building_state_tx, building_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (building_desc_tx, building_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (building_nickname_state_tx, building_nickname_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (crafting_recipe_desc_tx, crafting_recipe_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (traveler_task_desc_tx, traveler_task_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (traveler_task_state_tx, traveler_task_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (trade_order_state_tx, trade_order_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (buy_order_state_tx, buy_order_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (sell_order_state_tx, sell_order_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (npc_desc_tx, npc_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (collectible_desc_tx, collectible_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (interior_network_desc_tx, interior_network_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (dimension_description_state_tx, dimension_description_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (player_housing_state_tx, player_housing_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (permission_state_tx, permission_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (portal_state_tx, portal_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (resource_desc_tx, resource_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (extraction_recipe_desc_tx, extraction_recipe_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (progressive_action_state_tx, progressive_action_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (public_progressive_action_state_tx, public_progressive_action_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let craft_activity_tx = crate::crafting::events::start_worker(global_app_state.clone(), db_flush_delay);
        let (passive_craft_state_tx, passive_craft_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let mut remove_desc = false;

        let startup_gate = Arc::new(tokio::sync::Semaphore::new(1));
        let mut previous_startup = None;
        // Register every configured region before spawning any connection task,
        // including regions still waiting behind the startup gate.
        for database in config.spacetimedb.regional_databases() {
            initialize_connection_metrics(database);
        }
        config
            .spacetimedb
            .regional_databases()
            .for_each(|database| {
                let tmp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
                let tmp_player_state_tx = player_state_worker.tx();
                let tmp_player_username_state_tx = player_username_state_worker.tx();
                let tmp_experience_state_tx = experience_state_worker.tx();
                let tmp_inventory_state_tx = inventory_state_worker.tx();
                let tmp_item_desc_tx = item_desc_tx.clone();
                let tmp_cargo_desc_tx = cargo_desc_tx.clone();
                let tmp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
                let tmp_deployable_state_tx = deployable_state_tx.clone();
                let tmp_claim_state_tx = claim_state_worker.tx();
                let tmp_claim_local_state_tx = claim_local_state_worker.tx();
                let tmp_claim_member_state_tx = claim_member_state_worker.tx();
                let tmp_skill_desc_tx = skill_desc_tx.clone();
                let tmp_claim_tech_state_tx = claim_tech_state_worker.tx();
                let tmp_claim_tech_desc_tx = claim_tech_desc_worker.tx();
                let tmp_building_state_tx = building_state_tx.clone();
                let tmp_building_desc_tx = building_desc_tx.clone();
                let tmp_building_nickname_state_tx = building_nickname_state_tx.clone();
                let tmp_crafting_recipe_desc_tx = crafting_recipe_desc_tx.clone();
                let tmp_item_list_desc_tx = item_list_desc_tx.clone();
                let tmp_traveler_task_desc_tx = traveler_task_desc_tx.clone();
                let tmp_traveler_task_state_tx = traveler_task_state_tx.clone();
                let tmp_trade_order_state_tx = trade_order_state_tx.clone();
                let tmp_user_state_tx = user_state_tx.clone();
                let tmp_npc_desc_tx = npc_desc_tx.clone();
                let tmp_buy_order_state_tx = buy_order_state_tx.clone();
                let tmp_sell_order_state_tx = sell_order_state_tx.clone();
                let tmp_collectible_desc_tx = collectible_desc_tx.clone();
                let tmp_interior_network_desc_tx = interior_network_desc_tx.clone();
                let tmp_dimension_description_state_tx = dimension_description_state_tx.clone();
                let tmp_player_housing_state_tx = player_housing_state_tx.clone();
                let tmp_permission_state_tx = permission_state_tx.clone();
                let tmp_portal_state_tx = portal_state_tx.clone();
                let tmp_location_state_tx = location_state_worker.tx();
                let tmp_resource_desc_tx = resource_desc_tx.clone();
                let tmp_extraction_recipe_desc_tx = extraction_recipe_desc_tx.clone();
                let tmp_progressive_action_state_tx = progressive_action_state_tx.clone();
                let tmp_public_progressive_action_state_tx = public_progressive_action_state_tx.clone();
                let tmp_craft_activity_tx = craft_activity_tx.clone();
                let tmp_passive_craft_state_tx = passive_craft_state_tx.clone();
                let tmp_conf = config.clone();
                let tmp_global_app_state = global_app_state.clone();
                let tmp_remove_desc = remove_desc;
                let tmp_database = database.clone();
                let startup_gate = startup_gate.clone();
                let previous_initial = previous_startup.take();
                let (startup_done, startup_ready) = tokio::sync::oneshot::channel();
                previous_startup = Some(startup_ready);

                tmp_global_app_state.shutdown.clone().spawn_connection(async move {
                    let stop = tmp_global_app_state.shutdown.stop.clone();
                    let mut startup_done = Some(startup_done);
                    if !startup::wait_previous(previous_initial, &stop).await { return; }
                    let region = match tmp_database
                        .split('-')
                        .next_back()
                        .unwrap_or_default()
                        .parse::<entity::shared::Region>()
                    {
                        Ok(region) => region,
                        Err(error) => {
                            tracing::error!(%error, database = tmp_database, "Invalid database region");
                            return;
                        }
                    };

                    tracing::debug!(
                        region = tmp_database,
                        database_name = tmp_database.clone(),
                        spacetime_url = tmp_conf.spacetimedb_url(region),
                        "Waiting for websocket startup slot"
                    );
                    let mut startup_permit = startup::acquire(
                        &startup_gate,
                        &stop,
                    ).await;
                    if startup_permit.is_none() { return; }

                    let mut tries = 0;


                    let claim_event_tx = crate::claims::events::start_persistence_worker(
                        tmp_global_app_state.clone(),
                        region,
                        db_flush_delay,
                    );

                    loop {
                        if stop.is_cancelled() { break; }
                        // Reconnects share the same gate too. Failed initial
                        // attempts retain their slot across the retry backoff.
                        if startup_permit.is_none() {
                            startup_permit = startup::acquire(&startup_gate, &stop).await;
                            if startup_permit.is_none() { break; }
                        }
                        metrics::gauge!(
                            "bitcraft_database_connected",
                            &[("region", tmp_database.clone())]
                        )
                        .set(0);

                        let timing = ConnectionTiming::default();
                        tmp_global_app_state
                            .connection_state
                            .insert(tmp_database.clone(), false);

                        let result = connect_to_db_logic(
                            tmp_global_app_state.clone(),
                            &tmp_conf,
                            &tmp_database,
                            &tmp_remove_desc,
                            timing.clone(),
                            &mut startup_permit,
                            &mut startup_done,
                            &claim_event_tx,
                            &tmp_craft_activity_tx,
                            &tmp_passive_craft_state_tx,
                            &tmp_mobile_entity_state_tx,
                            &tmp_player_state_tx,
                            &tmp_player_username_state_tx,
                            &tmp_experience_state_tx,
                            &tmp_inventory_state_tx,
                            &tmp_item_desc_tx,
                            &tmp_cargo_desc_tx,
                            &tmp_vault_state_collectibles_tx,
                            &tmp_deployable_state_tx,
                            &tmp_claim_state_tx,
                            &tmp_claim_local_state_tx,
                            &tmp_claim_member_state_tx,
                            &tmp_skill_desc_tx,
                            &tmp_claim_tech_state_tx,
                            &tmp_claim_tech_desc_tx,
                            &tmp_building_state_tx,
                            &tmp_building_desc_tx,
                            &tmp_building_nickname_state_tx,
                            &tmp_crafting_recipe_desc_tx,
                            &tmp_item_list_desc_tx,
                            &tmp_traveler_task_desc_tx,
                            &tmp_traveler_task_state_tx,
                            &tmp_trade_order_state_tx,
                            &tmp_user_state_tx,
                            &tmp_npc_desc_tx,
                            &tmp_buy_order_state_tx,
                            &tmp_sell_order_state_tx,
                            &tmp_collectible_desc_tx,
                            &tmp_interior_network_desc_tx,
                            &tmp_dimension_description_state_tx,
                            &tmp_player_housing_state_tx,
                            &tmp_permission_state_tx,
                            &tmp_portal_state_tx,
                            &tmp_location_state_tx,
                            &tmp_resource_desc_tx,
                            &tmp_extraction_recipe_desc_tx,
                            &tmp_progressive_action_state_tx,
                            &tmp_public_progressive_action_state_tx,
                        )
                        .await;

                        if stop.is_cancelled() { break; }

                        if let Err(error) = result {
                            tracing::error!(
                                region = tmp_database,
                                error = error.to_string(),
                                "Connection to {tmp_database} on {} failed",
                                tmp_conf.spacetimedb_url(region)
                            );

                        }

                        // Count every unsuccessful/short-lived attempt, including
                        // build errors and clean disconnects. Only actual connected
                        // uptime can reset the budget, not time spent connecting.
                        tries = next_reconnect_failure_count(tries, timing.uptime());

                        if tries > MAX_RECONNECT_RETRIES {
                            tracing::error!(
                                region = tmp_database,
                                tries,
                                database_name = tmp_database.clone(),
                                spacetime_url = tmp_conf.spacetimedb_url(region),
                                "Websocket reconnect retry budget exhausted"
                            );
                            break;
                        }

                        let base_wait_secs = rand::rng().random_range(15..=45);
                        let wait = reconnect_delay(tries, base_wait_secs);

                        tracing::warn!(
                            region = tmp_database,
                            tries,
                            wait = wait.as_secs(),
                            database_name = tmp_database.clone(),
                            spacetime_url = tmp_conf.spacetimedb_url(region),
                            "Retrying websocket connection"
                        );
                        tokio::select! { biased;
                            _ = stop.cancelled() => break,
                            _ = tokio::time::sleep(wait) => {},
                        }
                    }
                });

                remove_desc = true;
            });

        start_worker_mobile_entity_state(global_app_state.clone(), mobile_entity_state_rx);
        player_state_worker.start(&global_app_state.shutdown);
        player_username_state_worker.start(&global_app_state.shutdown);
        experience_state_worker.start(&global_app_state.shutdown);
        location_state_worker.start(&global_app_state.shutdown);
        inventory_state_worker.start(&global_app_state.shutdown);
        start_worker_vault_state_collectibles(
            global_app_state.clone(),
            vault_state_collectibles_rx,
            3000,
            db_flush_delay,
        );
        start_worker_item_desc(
            global_app_state.clone(),
            item_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_cargo_desc(
            global_app_state.clone(),
            cargo_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_deployable_state(
            global_app_state.clone(),
            deployable_state_rx,
            3000,
            db_flush_delay,
        );
        claim_state_worker.start(&global_app_state.shutdown);
        claim_local_state_worker.start(&global_app_state.shutdown);
        claim_member_state_worker.start(&global_app_state.shutdown);
        start_worker_skill_desc(
            global_app_state.clone(),
            skill_desc_rx,
            3000,
            db_flush_delay,
        );
        claim_tech_state_worker.start(&global_app_state.shutdown);
        claim_tech_desc_worker.start(&global_app_state.shutdown);
        start_worker_building_state(
            global_app_state.clone(),
            building_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_building_desc(
            global_app_state.clone(),
            building_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_building_nickname_state(
            global_app_state.clone(),
            building_nickname_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_crafting_recipe_desc(
            global_app_state.clone(),
            crafting_recipe_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_item_list_desc(
            global_app_state.clone(),
            item_list_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_traveler_task_desc(
            global_app_state.clone(),
            traveler_task_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_traveler_task_state(
            global_app_state.clone(),
            traveler_task_state_rx,
            6000,
            db_flush_delay,
        );
        start_worker_trade_order_state(
            global_app_state.clone(),
            trade_order_state_rx,
            6000,
            db_flush_delay,
        );

        start_worker_npc_desc(
            global_app_state.clone(),
            npc_desc_rx,
            3000,
            db_flush_delay,
        );

        start_worker_buy_order_state(
            global_app_state.clone(),
            buy_order_state_rx,
            3000,
            db_flush_delay,
        );

        start_worker_sell_order_state(
            global_app_state.clone(),
            sell_order_state_rx,
            3000,
            db_flush_delay,
        );

        start_worker_collectible_desc(
            global_app_state.clone(),
            collectible_desc_rx,
            3000,
            db_flush_delay,
        );

        start_worker_user_state(global_app_state.clone(), user_state_rx);

        start_worker_interior_network_desc(
            global_app_state.clone(),
            interior_network_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_dimension_description_state(
            global_app_state.clone(),
            dimension_description_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_player_housing_state(
            global_app_state.clone(),
            player_housing_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_permission_state(
            global_app_state.clone(),
            permission_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_portal_state(
            global_app_state.clone(),
            portal_state_rx,
            3000,
            db_flush_delay,
        );
        start_worker_resource_desc(
            global_app_state.clone(),
            resource_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_extraction_recipe_desc(
            global_app_state.clone(),
            extraction_recipe_desc_rx,
            3000,
            db_flush_delay,
        );
        start_worker_progressive_action_state(
            global_app_state.clone(),
            progressive_action_state_rx,
            3000,
            db_flush_delay,
        );
        crate::crafting::public_progressive_action::start_worker(
            global_app_state.clone(),
            public_progressive_action_state_rx,
            3000,
            db_flush_delay,
        );
        crate::crafting::passive::start_worker(global_app_state.clone(), passive_craft_state_rx, 3000, db_flush_delay);
    });
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub(crate) enum SpacetimeUpdateMessages<T> {
    Initial {
        data: Vec<T>,
        database_name: entity::shared::Region,
        batches: Option<initial_batches::InitialBatches<T>>,
    },
    Insert {
        event: Option<Box<__sdk::Event<game_module::module_bindings::Reducer>>>,
        new: T,
        database_name: entity::shared::Region,
    },
    Update {
        event: Option<Box<__sdk::Event<game_module::module_bindings::Reducer>>>,
        old: T,
        new: T,
        database_name: entity::shared::Region,
    },
    Remove {
        event: Option<Box<__sdk::Event<game_module::module_bindings::Reducer>>>,
        delete: T,
        database_name: entity::shared::Region,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "t", content = "c")]
pub(crate) enum InboundWebSocketMessages {
    Subscribe { topics: Vec<String> },
    ListSubscribedTopics,
    SyncLeaderboardBoard { board_id: i32, epoch: Option<String>, revision: Option<u32> },
    Unsubscribe { topic: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "t", content = "c")]
pub(crate) enum OutboundWebSocketMessages {
    SubscribedTopics(Vec<String>),
    LeaderboardUpdated(crate::leaderboard::GetTop100Response),
    LeaderboardBoardUpdated(crate::leaderboard::live::LeaderboardBoardUpdate),
    LeaderboardBoardDelta(crate::leaderboard::live::LeaderboardBoardDelta),
    LeaderboardBoardReset(crate::leaderboard::live::LeaderboardBoardUpdate),
    MobileEntityState(entity::mobile_entity_state::Model),
    PlayerLocation(crate::player_state::PlayerLocationUpdate),
    ExperienceHistoryChanged { user_id: i64 },
    ClaimExperienceHistoryChanged { claim_id: i64 },
    ClaimCraftingChanged { claim_id: Option<String> },
    PlayerCraftingChanged { player_id: Option<String> },
    PlayerStatsChanged { player_id: Option<String>, stats: Option<crate::player_state::stats::StatsSnapshot> },
    PlayerCharacterStatsChanged { player_id: Option<String>, delta: Option<crate::player_state::stats::CharacterStatsDelta> },
    PlayerActivityChanged { player_id: Option<String>, activity: Option<crate::player_state::activity::ActivitySnapshot> },
    CraftProgressChanged { claim_id: Option<String>, player_id: String, craft_id: String, progress: i32, lock_expiration: String },
    PassiveCraftChanged { claim_id: Option<String>, player_id: String, craft_id: String, status: String, slot: Option<u32>, timestamp: String },
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
        rank: u64,
    },
    TotalProfessionExperience {
        user_id: i64,
        experience: u64,
        rank: u64,
    },
    TotalAdventureExperience {
        user_id: i64,
        experience: u64,
        rank: u64,
    },
    TimePlayed {
        user_id: i64,
        time: u64,
        rank: u64,
    },
    TimeSignedIn {
        user_id: i64,
        time: u64,
        rank: u64,
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
    PlayerPresenceChanged { player_id: String, signed_in: bool },
    TravelerTaskState(entity::traveler_task_state::Model),
    TravelerTaskStateDelete(entity::traveler_task_state::Model),
    // ClaimDescriptionState(entity::claim_description_state::Model),
    ClaimLocalState(entity::claim_local_state::Model),
    Message(String),
    ActionState(entity::action_state::Model),
    InsertSellOrder(entity::auction_listing_state::AuctionListingState),
    UpdateSellOrder(entity::auction_listing_state::AuctionListingState),
    RemoveSellOrder(entity::auction_listing_state::AuctionListingState),
    InsertBuyOrder(entity::auction_listing_state::AuctionListingState),
    UpdateBuyOrder(entity::auction_listing_state::AuctionListingState),
    RemoveBuyOrder(entity::auction_listing_state::AuctionListingState),
    InventoryUpdate {
        resolved_inventory: entity::inventory::ResolvedInventory,
    },
    InventoryRemove {
        resolved_inventory: entity::inventory::ResolvedInventory,
    },
    InventoryInsert {
        resolved_inventory: entity::inventory::ResolvedInventory,
        player_owner_id: i64,
    },
    PlayerUsername {
        entity_id: i64,
        username: String,
    },
}

/// At most three routing topics; fixed names borrow static storage.
#[derive(Debug, PartialEq)]
pub(crate) struct BroadcastTopics([Option<(Cow<'static, str>, Option<i64>)>; 3]);

impl BroadcastTopics {
    pub(crate) fn new<const N: usize>(topics: [(Cow<'static, str>, Option<i64>); N]) -> Self {
        assert!(N <= 3);
        let mut topics = topics.into_iter();
        Self(std::array::from_fn(|_| topics.next()))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &(Cow<'static, str>, Option<i64>)> {
        self.0.iter().flatten()
    }

    pub(crate) fn player_location(id: u64) -> Self {
        Self::new([("player_location".into(), Some(id as i64))])
    }

    pub(crate) fn mobile_entity(id: u64) -> Self {
        Self::new([("mobile_entity_state".into(), Some(id as i64))])
    }

    pub(crate) fn player_presence(id: i64) -> Self {
        Self::new([("player_presence".into(), Some(id))])
    }

    pub(crate) fn experience(id: i64, skill: &str) -> Self {
        Self::new([
            (format!("experience:{skill}").into(), Some(id)),
            ("experience".into(), Some(id)),
        ])
    }

    pub(crate) fn total_experience(id: i64) -> Self {
        Self::new([("total_experience".into(), Some(id))])
    }

    pub(crate) fn total_profession_experience(id: i64) -> Self {
        Self::new([("total_profession_experience".into(), Some(id))])
    }

    pub(crate) fn total_adventure_experience(id: i64) -> Self {
        Self::new([("total_adventure_experience".into(), Some(id))])
    }

    pub(crate) fn inventory_update(id: i64) -> Self {
        Self::new([("inventory_update".into(), Some(id))])
    }

    pub(crate) fn inventory_remove(id: i64) -> Self {
        // Preserve the existing inventory_remove_owner route's entity ID.
        Self::new([
            ("inventory_remove".into(), Some(id)),
            ("inventory_remove_owner".into(), Some(id)),
        ])
    }

    pub(crate) fn inventory_insert(id: i64, owner: i64, player_owner: i64) -> Self {
        Self::new([
            ("inventory_insert".into(), Some(id)),
            ("inventory_insert_owner".into(), Some(owner)),
            ("inventory_insert_player_owner".into(), Some(player_owner)),
        ])
    }
}

impl IntoIterator for BroadcastTopics {
    type Item = (Cow<'static, str>, Option<i64>);
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<Self::Item>, 3>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().flatten()
    }
}

impl OutboundWebSocketMessages {
    /// The protocol's `t` tag, without inspecting or serializing the payload.
    fn message_type(&self) -> &'static str {
        match self {
            Self::SubscribedTopics(..) => "SubscribedTopics",
            Self::LeaderboardUpdated(..) => "LeaderboardUpdated",
            Self::LeaderboardBoardUpdated(..) => "LeaderboardBoardUpdated",
            Self::LeaderboardBoardDelta(..) => "LeaderboardBoardDelta",
            Self::LeaderboardBoardReset(..) => "LeaderboardBoardReset",
            Self::MobileEntityState(..) => "MobileEntityState",
            Self::PlayerLocation(..) => "PlayerLocation",
            Self::ExperienceHistoryChanged { .. } => "ExperienceHistoryChanged",
            Self::ClaimExperienceHistoryChanged { .. } => "ClaimExperienceHistoryChanged",
            Self::ClaimCraftingChanged { .. } => "ClaimCraftingChanged",
            Self::PlayerCraftingChanged { .. } => "PlayerCraftingChanged",
            Self::PlayerStatsChanged { .. } => "PlayerStatsChanged",
            Self::PlayerCharacterStatsChanged { .. } => "PlayerCharacterStatsChanged",
            Self::PlayerActivityChanged { .. } => "PlayerActivityChanged",
            Self::CraftProgressChanged { .. } => "CraftProgressChanged",
            Self::PassiveCraftChanged { .. } => "PassiveCraftChanged",
            Self::Experience { .. } => "Experience",
            Self::TotalExperience { .. } => "TotalExperience",
            Self::TotalProfessionExperience { .. } => "TotalProfessionExperience",
            Self::TotalAdventureExperience { .. } => "TotalAdventureExperience",
            Self::TimePlayed { .. } => "TimePlayed",
            Self::TimeSignedIn { .. } => "TimeSignedIn",
            Self::MovedOutOfClaim { .. } => "MovedOutOfClaim",
            Self::MovedIntoClaim { .. } => "MovedIntoClaim",
            Self::PlayerMovedIntoClaim { .. } => "PlayerMovedIntoClaim",
            Self::PlayerMovedOutOfClaim { .. } => "PlayerMovedOutOfClaim",
            Self::PlayerActionState(..) => "PlayerActionState",
            Self::PlayerActionStateChangeName(..) => "PlayerActionStateChangeName",
            Self::Level { .. } => "Level",
            Self::PlayerState(..) => "PlayerState",
            Self::PlayerPresenceChanged { .. } => "PlayerPresenceChanged",
            Self::TravelerTaskState(..) => "TravelerTaskState",
            Self::TravelerTaskStateDelete(..) => "TravelerTaskStateDelete",
            Self::ClaimLocalState(..) => "ClaimLocalState",
            Self::Message(..) => "Message",
            Self::ActionState(..) => "ActionState",
            Self::InsertSellOrder(..) => "InsertSellOrder",
            Self::UpdateSellOrder(..) => "UpdateSellOrder",
            Self::RemoveSellOrder(..) => "RemoveSellOrder",
            Self::InsertBuyOrder(..) => "InsertBuyOrder",
            Self::UpdateBuyOrder(..) => "UpdateBuyOrder",
            Self::RemoveBuyOrder(..) => "RemoveBuyOrder",
            Self::InventoryUpdate { .. } => "InventoryUpdate",
            Self::InventoryRemove { .. } => "InventoryRemove",
            Self::InventoryInsert { .. } => "InventoryInsert",
            Self::PlayerUsername { .. } => "PlayerUsername",
        }
    }

    pub fn topics(&self) -> Option<BroadcastTopics> {
        match self {
            OutboundWebSocketMessages::LeaderboardUpdated(_) => Some(
                BroadcastTopics::new([("leaderboard".into(), None)]),
            ),
            OutboundWebSocketMessages::LeaderboardBoardUpdated(board) => {
                Some(crate::leaderboard::live::topic(board.board_id))
            }
            OutboundWebSocketMessages::LeaderboardBoardDelta(board) => {
                Some(crate::leaderboard::live::delta_topic(board.board_id))
            }
            OutboundWebSocketMessages::LeaderboardBoardReset(board) => {
                Some(crate::leaderboard::live::delta_topic(board.board_id))
            }
            OutboundWebSocketMessages::Experience {
                skill_name,
                user_id,
                ..
            } => Some(BroadcastTopics::experience(*user_id, skill_name)),
            OutboundWebSocketMessages::ClaimLocalState(claim_local_state) => {
                Some(BroadcastTopics::new([(
                    "claim_local_state".into(),
                    Some(claim_local_state.entity_id),
                )]))
            }
            OutboundWebSocketMessages::Level {
                user_id,
                skill_name,
                ..
            } => Some(BroadcastTopics::new([
                (format!("level:{skill_name}").into(), Some(*user_id)),
                ("level".into(), Some(*user_id)),
            ])),
            OutboundWebSocketMessages::PlayerMovedIntoClaim { user_id, .. } => Some(
                BroadcastTopics::new([("player_moved_into_claim".into(), Some(*user_id))]),
            ),
            OutboundWebSocketMessages::PlayerMovedOutOfClaim { user_id, .. } => Some(
                BroadcastTopics::new([("player_moved_out_of_claim".into(), Some(*user_id))]),
            ),
            OutboundWebSocketMessages::MovedOutOfClaim { claim_id, .. } => Some(
                BroadcastTopics::new([("moved_out_of_claim".into(), Some(*claim_id as i64))]),
            ),
            OutboundWebSocketMessages::MovedIntoClaim { claim_id, .. } => Some(
                BroadcastTopics::new([("moved_into_claim".into(), Some(*claim_id as i64))]),
            ),
            OutboundWebSocketMessages::PlayerState(player) => Some(BroadcastTopics::new([(
                "player_state".into(),
                Some(player.entity_id),
            )])),
            OutboundWebSocketMessages::PlayerPresenceChanged { player_id, .. } => {
                Some(BroadcastTopics::player_presence(player_id.parse().ok()?))
            }
            OutboundWebSocketMessages::PlayerUsername { entity_id, .. } => Some(
                BroadcastTopics::new([("player_username".into(), Some(*entity_id))]),
            ),
            OutboundWebSocketMessages::MobileEntityState(mobile_entity_state) => {
                Some(BroadcastTopics::mobile_entity(mobile_entity_state.entity_id))
            }
            OutboundWebSocketMessages::PlayerLocation(location) => Some(BroadcastTopics::player_location(location.entity_id)),
            OutboundWebSocketMessages::ExperienceHistoryChanged { user_id } => Some(
                BroadcastTopics::new([("experience_history".into(), Some(*user_id))]),
            ),
            OutboundWebSocketMessages::ClaimExperienceHistoryChanged { claim_id } => Some(
                BroadcastTopics::new([("claim_experience_history".into(), Some(*claim_id))]),
            ),
            OutboundWebSocketMessages::ClaimCraftingChanged { claim_id } => Some(
                BroadcastTopics::new([("claim_crafting".into(), claim_id.as_ref().and_then(|id| id.parse().ok()))]),
            ),
            OutboundWebSocketMessages::PlayerCraftingChanged { player_id } => Some(
                BroadcastTopics::new([("player_crafting".into(), player_id.as_ref().and_then(|id| id.parse().ok()))]),
            ),
            OutboundWebSocketMessages::PlayerStatsChanged { player_id, .. } => Some(
                BroadcastTopics::new([("player_stats".into(), player_id.as_ref().and_then(|id| id.parse().ok()))]),
            ),
            OutboundWebSocketMessages::PlayerCharacterStatsChanged { player_id, .. } => Some(
                BroadcastTopics::new([("player_character_stats".into(), player_id.as_ref().and_then(|id| id.parse().ok()))]),
            ),
            OutboundWebSocketMessages::PlayerActivityChanged { player_id, .. } => Some(
                BroadcastTopics::new([("player_activity".into(), player_id.as_ref().and_then(|id| id.parse().ok()))]),
            ),
            OutboundWebSocketMessages::CraftProgressChanged { claim_id, player_id, .. } => {
                let player = player_id.parse::<i64>().ok()?;
                if let Some(claim) = claim_id.as_ref().and_then(|id| id.parse::<i64>().ok()) {
                    Some(BroadcastTopics::new([("player_craft_progress".into(), Some(player)), ("claim_craft_progress".into(), Some(claim))]))
                } else {
                    Some(BroadcastTopics::new([("player_craft_progress".into(), Some(player))]))
                }
            },
            OutboundWebSocketMessages::PassiveCraftChanged { claim_id, player_id, .. } => {
                let player = player_id.parse::<i64>().ok()?;
                if let Some(claim) = claim_id.as_ref().and_then(|id| id.parse::<i64>().ok()) {
                    Some(BroadcastTopics::new([("player_passive_craft".into(), Some(player)), ("claim_passive_craft".into(), Some(claim))]))
                } else {
                    Some(BroadcastTopics::new([("player_passive_craft".into(), Some(player))]))
                }
            },
            // WebSocketMessages::ClaimDescriptionState(claim) => {
            //     Some(vec![("claim".to_string(), claim.entity_id)])
            // }
            OutboundWebSocketMessages::TotalExperience { user_id, .. } => Some(
                BroadcastTopics::total_experience(*user_id),
            ),
            OutboundWebSocketMessages::TotalProfessionExperience { user_id, .. } => Some(
                BroadcastTopics::total_profession_experience(*user_id),
            ),
            OutboundWebSocketMessages::TotalAdventureExperience { user_id, .. } => Some(
                BroadcastTopics::total_adventure_experience(*user_id),
            ),
            OutboundWebSocketMessages::TimePlayed { user_id, .. } => {
                Some(BroadcastTopics::new([(
                    "time_played".into(),
                    Some(*user_id),
                )]))
            }
            OutboundWebSocketMessages::TimeSignedIn { user_id, .. } => Some(BroadcastTopics::new(
                [("time_signed_in".into(), Some(*user_id))],
            )),
            OutboundWebSocketMessages::PlayerActionState(player_action_state) => {
                Some(BroadcastTopics::new([(
                    "player_action_state".into(),
                    Some(player_action_state.entity_id as i64),
                )]))
            }
            OutboundWebSocketMessages::PlayerActionStateChangeName(_, id) => {
                Some(BroadcastTopics::new([(
                    "player_action_state_change_name".into(),
                    Some(*id as i64),
                )]))
            }
            OutboundWebSocketMessages::ActionState(action_state) => Some(BroadcastTopics::new([(
                "action_state".into(),
                Some(action_state.owner_entity_id as i64),
            )])),
            OutboundWebSocketMessages::TravelerTaskState(traveler_task_state) => {
                Some(BroadcastTopics::new([
                    (
                        "traveler_task_state".into(),
                        Some(traveler_task_state.entity_id),
                    ),
                    (
                        "traveler_task_state:player".into(),
                        Some(traveler_task_state.player_entity_id),
                    ),
                ]))
            }
            OutboundWebSocketMessages::TravelerTaskStateDelete(traveler_task_state) => {
                Some(BroadcastTopics::new([
                    (
                        "traveler_task_state".into(),
                        Some(traveler_task_state.entity_id),
                    ),
                    (
                        "traveler_task_state:player".into(),
                        Some(traveler_task_state.player_entity_id),
                    ),
                ]))
            }
            OutboundWebSocketMessages::SubscribedTopics(_) => None,
            OutboundWebSocketMessages::Message(_) => None,
            OutboundWebSocketMessages::InsertSellOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "insert_sell_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "insert_sell_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("insert_sell_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::UpdateSellOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "update_sell_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "update_sell_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("update_sell_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::RemoveSellOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "remove_sell_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "remove_sell_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("remove_sell_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::InsertBuyOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "update_buy_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "update_buy_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("update_buy_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::UpdateBuyOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "update_buy_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "update_buy_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("update_buy_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::RemoveBuyOrder(auction_listing_state) => {
                Some(BroadcastTopics::new([
                    (
                        "remove_buy_order".into(),
                        Some(auction_listing_state.entity_id as i64),
                    ),
                    (
                        "remove_buy_order:item_id".into(),
                        Some(auction_listing_state.item_id as i64),
                    ),
                    ("remove_buy_order".into(), None),
                ]))
            }
            OutboundWebSocketMessages::InventoryUpdate {
                resolved_inventory, ..
            } => Some(BroadcastTopics::inventory_update(resolved_inventory.entity_id)),
            OutboundWebSocketMessages::InventoryRemove {
                resolved_inventory, ..
            } => Some(BroadcastTopics::inventory_remove(resolved_inventory.entity_id)),
            OutboundWebSocketMessages::InventoryInsert {
                resolved_inventory,
                player_owner_id,
                ..
            } => Some(BroadcastTopics::inventory_insert(
                resolved_inventory.entity_id,
                resolved_inventory.owner_entity_id,
                *player_owner_id,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConnectionTiming, Duration, InboundWebSocketMessages, MAX_RECONNECT_RETRIES,
        OutboundWebSocketMessages, STABLE_CONNECTION_DURATION, next_reconnect_failure_count,
        reconnect_delay, subscription_queries,
    };
    use serde_json::json;

    #[test]
    fn connection_metrics_exist_before_connect_and_retain_last_timestamp() {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            super::initialize_connection_metrics("bitcraft-live-11");
            super::initialize_connection_metrics("bitcraft-live-12");
            let initial = handle.render();
            assert!(initial.contains("bitcraft_database_connected{region=\"bitcraft-live-12\"} 0"));
            assert!(initial.contains("bitcraft_database_last_connected_timestamp_seconds{region=\"bitcraft-live-11\"} 0"));
            super::record_connection_metrics("bitcraft-live-11", 1234.5);
            metrics::gauge!("bitcraft_database_connected", "region" => "bitcraft-live-11").set(0);
            let disconnected = handle.render();
            assert!(disconnected.contains("bitcraft_database_connected{region=\"bitcraft-live-11\"} 0"));
            assert!(disconnected.contains("bitcraft_database_last_connected_timestamp_seconds{region=\"bitcraft-live-11\"} 1234.5"));
            super::record_connection_metrics("bitcraft-live-11", 2345.0);
            assert!(handle.render().contains("bitcraft_database_last_connected_timestamp_seconds{region=\"bitcraft-live-11\"} 2345"));
        });
    }

    #[test]
    fn routing_topics_borrow_static_names_and_keep_dynamic_names_and_ids() {
        use std::borrow::Cow;
        let message = OutboundWebSocketMessages::Experience {
            experience: 10,
            level: 1,
            rank: 2,
            skill_name: "Mining".into(),
            user_id: 7,
        };
        let topics: Vec<_> = message.topics().unwrap().into_iter().collect();
        assert!(matches!(&topics[0].0, Cow::Owned(name) if name == "experience:Mining"));
        assert!(matches!(&topics[1].0, Cow::Borrowed("experience")));
        assert_eq!(topics[0].1, Some(7));
        assert_eq!(topics[1].1, Some(7));
        let inventory = entity::inventory::ResolvedInventory {
            entity_id: 1,
            region: 1,
            pockets: vec![],
            inventory_index: 2,
            cargo_index: 3,
            owner_entity_id: 4,
            player_owner_entity_id: 5,
            nickname: None,
            claim: None,
        };
        let topics: Vec<_> = OutboundWebSocketMessages::InventoryInsert {
            resolved_inventory: inventory.clone(),
            player_owner_id: 6,
        }
        .topics()
        .unwrap()
        .into_iter()
        .collect();
        assert_eq!(
            topics,
            [
                (Cow::Borrowed("inventory_insert"), Some(1)),
                (Cow::Borrowed("inventory_insert_owner"), Some(4)),
                (Cow::Borrowed("inventory_insert_player_owner"), Some(6))
            ]
        );
        let topics: Vec<_> = OutboundWebSocketMessages::InventoryRemove {
            resolved_inventory: inventory,
        }
        .topics()
        .unwrap()
        .into_iter()
        .collect();
        // Preserve the existing remove-owner routing ID, even though it differs
        // from insert-owner routing. Fixing the protocol is a separate change.
        assert_eq!(
            topics,
            [
                (Cow::Borrowed("inventory_remove"), Some(1)),
                (Cow::Borrowed("inventory_remove_owner"), Some(1))
            ]
        );
    }

    #[test]
    fn rss_diagnostic_has_zero_subscriptions_even_on_later_regions() {
        let tables = [
            "cargo_desc",
            "item_desc",
            "inventory_state",
            "experience_state",
            "dimension_description_state",
            "identity_role",
            "claim_treasury_event",
        ];
        let sql = [
            ("equipment_desc", "SELECT * FROM equipment_desc"),
            ("equipment_state", "SELECT * FROM equipment_state"),
            (
                "mobile_entity_state",
                "SELECT mobile_entity_state.* FROM mobile_entity_state JOIN player_state ON mobile_entity_state.entity_id = player_state.entity_id",
            ),
        ];
        assert!(subscription_queries(&tables, &sql, false, true).is_empty());
        assert!(subscription_queries(&tables, &sql, true, true).is_empty());
    }

    #[test]
    fn disabling_rss_diagnostic_restores_state_subscriptions() {
        let tables = ["item_desc", "inventory_state"];
        let sql = [("equipment_state", "SELECT * FROM equipment_state")];
        assert_eq!(
            subscription_queries(&tables, &sql, false, false),
            [
                "select * from item_desc",
                "select * from inventory_state",
                "SELECT * FROM equipment_state",
            ]
        );
        assert_eq!(
            subscription_queries(&tables, &sql, true, false),
            [
                "select * from inventory_state",
                "SELECT * FROM equipment_state",
            ]
        );
    }

    #[test]
    fn failed_connections_exhaust_the_retry_budget() {
        let mut tries = 0;
        // Initial attempt plus fifteen retries, including build/handshake errors
        // where on_connect never ran.
        for attempt in 1..=MAX_RECONNECT_RETRIES + 1 {
            tries = next_reconnect_failure_count(tries, None);
            assert_eq!(tries, attempt);
            assert_eq!(tries > MAX_RECONNECT_RETRIES, attempt == 16);
        }
    }

    #[test]
    fn short_connections_count_regardless_of_disconnect_reason() {
        for uptime in [Duration::ZERO, Duration::from_secs(119)] {
            assert_eq!(next_reconnect_failure_count(4, Some(uptime)), 5);
        }
    }

    #[test]
    fn stable_connections_reset_the_retry_budget() {
        for uptime in [STABLE_CONNECTION_DURATION, Duration::from_secs(600)] {
            assert_eq!(next_reconnect_failure_count(14, Some(uptime)), 0);
        }
        // A failed connection after recovery starts counting again.
        assert_eq!(next_reconnect_failure_count(0, None), 1);
    }

    #[test]
    fn connection_timing_starts_only_after_on_connect() {
        let timing = ConnectionTiming::default();
        let callback_timing = timing.clone();
        assert_eq!(timing.uptime(), None);
        callback_timing.connected();
        assert!(timing.uptime().is_some());
    }

    #[tokio::test]
    async fn diagnostic_startup_waits_for_on_connect_without_missing_an_early_callback() {
        let timing = ConnectionTiming::default();
        let connected = timing.wait_connected();
        tokio::pin!(connected);
        assert!(futures::poll!(connected.as_mut()).is_pending());
        timing.connected();
        tokio::time::timeout(Duration::from_secs(1), connected).await.unwrap();

        let already_connected = ConnectionTiming::default();
        already_connected.connected();
        tokio::time::timeout(Duration::from_secs(1), already_connected.wait_connected()).await.unwrap();
    }

    #[test]
    fn reconnect_backoff_is_capped_and_random_base_is_bounded() {
        assert_eq!(reconnect_delay(0, 0), Duration::from_secs(15));
        assert_eq!(reconnect_delay(1, 45), Duration::from_secs(45));
        assert_eq!(reconnect_delay(2, 15), Duration::from_secs(30));
        assert_eq!(reconnect_delay(2, 45), Duration::from_secs(60));
        assert_eq!(reconnect_delay(7, 15), Duration::from_secs(60));
        assert_eq!(
            reconnect_delay(u32::MAX, u64::MAX),
            Duration::from_secs(60)
        );
        assert_eq!(next_reconnect_failure_count(u32::MAX, None), u32::MAX);
    }

    #[test]
    fn inbound_commands_preserve_wire_format() {
        let cases = [
            (
                InboundWebSocketMessages::Subscribe {
                    topics: vec!["player_state.42".to_string()],
                },
                json!({"t": "Subscribe", "c": {"topics": ["player_state.42"]}}),
            ),
            (
                InboundWebSocketMessages::Unsubscribe {
                    topic: "player_state.42".to_string(),
                },
                json!({"t": "Unsubscribe", "c": {"topic": "player_state.42"}}),
            ),
            (
                InboundWebSocketMessages::ListSubscribedTopics,
                json!({"t": "ListSubscribedTopics"}),
            ),
        ];

        for (message, expected) in cases {
            assert_eq!(serde_json::to_value(&message).unwrap(), expected);
            let parsed: InboundWebSocketMessages =
                serde_json::from_value(expected.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
            assert!(serde_json::from_value::<OutboundWebSocketMessages>(expected).is_err());
        }
    }

    #[test]
    fn inbound_messages_reject_server_responses() {
        for message in [
            json!({"t": "SubscribedTopics", "c": ["player_state.42"]}),
            json!({"t": "Message", "c": "joined"}),
            json!({"t": "PlayerUsername", "c": {"entity_id": 42, "username": "player"}}),
        ] {
            assert!(serde_json::from_value::<InboundWebSocketMessages>(message).is_err());
        }
    }

    #[test]
    fn direct_responses_preserve_wire_format_and_have_no_broadcast_topics() {
        let message =
            OutboundWebSocketMessages::SubscribedTopics(vec!["player_state.42".to_string()]);
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({"t": "SubscribedTopics", "c": ["player_state.42"]})
        );
        assert_eq!(message.topics(), None);
        assert_eq!(
            OutboundWebSocketMessages::Message("joined".to_string()).topics(),
            None
        );
    }
}
