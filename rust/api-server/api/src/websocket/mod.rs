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
mod overworld;
mod portal_locations;
pub(crate) use metric_handles::WorkerReceiveMetrics;
use metric_handles::{ListenerMetrics, WorkerSendMetrics};
pub(crate) mod worker_channel;

const MAX_RECONNECT_RETRIES: u32 = 15;
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(120);

#[derive(Clone, Default)]
struct ConnectionTiming(Arc<Mutex<Option<Instant>>>);

impl ConnectionTiming {
    fn connected(&self) {
        *self.0.lock().unwrap() = Some(Instant::now());
    }

    fn uptime(&self) -> Option<Duration> {
        self.0.lock().unwrap().map(|connected| connected.elapsed())
    }
}

fn next_reconnect_failure_count(previous: u32, uptime: Option<Duration>) -> u32 {
    if uptime.is_some_and(|uptime| uptime >= STABLE_CONNECTION_DURATION) {
        0
    } else {
        previous.saturating_add(1)
    }
}

fn reconnect_delay(tries: u32, jitter_secs: u64) -> Duration {
    let base_wait_secs = (u64::from(tries.max(1)) * 45).min(300);
    Duration::from_secs(base_wait_secs + jitter_secs.min(90))
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
    global_app_state: AppState,
    token: String,
    db_name: &str,
    db_host: &str,
    timing: ConnectionTiming,
    region_number: Region,
) -> spacetimedb_sdk::Result<DbConnection> {
    let tmp_global_app_state = global_app_state.clone();
    let tmp_disconnect_global_app_state = global_app_state.clone();
    let tmp_db_name = db_name.to_owned();
    let tmp_disconnect_db_name = tmp_db_name.clone();

    DbConnection::builder()
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(move |_ctx, identity, _token| {
            timing.connected();
            tracing::debug!(
                region = tmp_db_name,
                "Connected to server {tmp_db_name} with {identity}"
            );
            metrics::gauge!(
                "bitcraft_database_connected",
                &[("region", tmp_db_name.clone())]
            )
            .set(1);

            tmp_global_app_state
                .connection_state
                .insert(tmp_db_name.clone(), true);
        })
        .with_row_deduplication(true)
        // Register our `on_connect_error` callback, which will print a message, then exit the process.
        .on_connect_error(on_connect_error)
        // Our `on_disconnect` callback, which will print a message, then exit the process.
        .on_disconnect(move |_ctx, err| {
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
        // If the user has previously connected, we'll have saved a token in the `on_connect` callback.
        // In that case, we'll load it and pass it to `with_token`,
        // so we can re-authenticate as the same `Identity`.
        // .with_token(Some(token.trim()))
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

fn connect_to_db_global(
    global_app_state: AppState,
    config: &Config,
    database: &str,
) -> anyhow::Result<()> {
    let shutdown = global_app_state.shutdown.clone();
    if shutdown.stop.is_cancelled() {
        return Ok(());
    }
    let ctx = connect_to_db(
        global_app_state,
        config.spacetimedb.password.clone(),
        database,
        config.spacetimedb_url(0).as_ref(),
        ConnectionTiming::default(),
        0
    )?;

    if !ctx.is_active() {
        tracing::error!(
            region = database,
            "Could not connect to the bitcraft server {} with module {database}",
            config.spacetimedb_url(0)
        );
        anyhow::bail!("Connection to {database} is inactive");
    }

    shutdown.clone().spawn_connection(async move {
        if let Err(error) = run_connection_until_shutdown(&ctx, &shutdown.stop).await {
            tracing::error!(%error, "Global database connection failed");
        }
    });

    Ok(())
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
    claim_event_tx: &tokio::sync::mpsc::Sender<entity::claim_event::ActiveModel>,
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
) -> anyhow::Result<()> {
    if global_app_state.shutdown.stop.is_cancelled() {
        return Ok(());
    }

    let region_number = database
        .split("-")
        .last()
        .unwrap()
        .parse::<entity::shared::Region>()?;

    let ctx = connect_to_db(
        global_app_state.clone(),
        config.spacetimedb.password.clone(),
        database,
        config.spacetimedb_url(region_number).as_ref(),
        timing,
        region_number,
    )?;

    if !ctx.is_active() {
        tracing::error!(
            region = database,
            "Could not connect to the bitcraft server {} with module {database}",
            config.spacetimedb_url(region_number)
        );
        anyhow::bail!("Connection to {database} is inactive");
    }

    crate::claims::events::register_listeners(
        &ctx,
        global_app_state.clone(),
        region_number,
        claim_event_tx.clone(),
    );

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

    let tables_to_subscribe = vec![
        // "user_state",
        // "mobile_entity_state",
        // "claim_tile_state",
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
        "character_stats_state",
        "identity_role",
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
        "player_housing_state",
        "permission_state",
        "portal_state",
    ];

    // "ability_custom_desc"
    // "ability_unlock_desc"
    // "achievement_desc"
    // "alert_desc"
    // "biome_desc"
    // "buff_desc"
    // "buff_type_desc"
    // "building_buff_desc"
    // "building_claim_desc"
    // "building_desc"
    // "building_function_type_mapping_desc"
    // "building_portal_desc"
    // "building_repairs_desc"
    // "building_spawn_desc"
    // "building_type_desc"
    // "cargo_desc"
    // "character_stat_desc"
    // "chest_rarity_desc"
    // "claim_tech_desc"
    // "climb_requirement_desc"
    // "clothing_desc"
    // "collectible_desc"
    // "combat_action_desc"
    // "combat_action_multi_hit_desc"
    // "construction_recipe_desc"
    // "construction_recipe_discovery_cargo_desc"
    // "construction_recipe_discovery_item_desc"
    // "construction_recipe_discovery_knowledge_desc"
    // "contribution_loot_desc"
    // "crafting_recipe_desc"
    // "crafting_recipe_discovery_cargo_desc"
    // "crafting_recipe_discovery_item_desc"
    // "crafting_recipe_discovery_knowledge_desc"
    // "deconstruction_recipe_desc"
    // "deployable_desc"
    // "distant_visible_entity_desc"
    // "elevator_desc"
    // "emote_desc"
    // "empire_color_desc"
    // "empire_icon_desc"
    // "empire_notification_desc"
    // "empire_rank_desc"
    // "empire_supplies_desc"
    // "empire_territory_desc"
    // "enemy_ai_params_desc"
    // "enemy_desc"
    // "enemy_scaling_desc"
    // "environment_debuff_desc"
    // "equipment_desc"
    // "equipment_preset_knowledge_desc"
    // "extraction_recipe_desc"
    // "food_desc"
    // "gate_desc"
    // "hexite_exchange_entry_desc"
    // "interior_environment_desc"
    // "interior_instance_desc"
    // "interior_network_desc"
    // "interior_portal_connections_desc"
    // "interior_shape_desc"
    // "interior_spawn_desc"
    // "item_conversion_recipe_desc"
    // "item_desc"
    // "item_list_desc"
    // "knowledge_scroll_desc"
    // "knowledge_scroll_type_desc"
    // "knowledge_stat_modifier_desc"
    // "loot_chest_desc"
    // "loot_rarity_desc"
    // "loot_table_desc"
    // "npc_desc"
    // "onboarding_reward_desc"
    // "parameters_desc"
    // "parameters_player_move_desc"
    // "pathfinding_desc"
    // "paving_recipe_discovery_cargo_desc"
    // "paving_recipe_discovery_item_desc"
    // "paving_recipe_discovery_knowledge_desc"
    // "paving_tile_desc"
    // "pillar_shaping_desc"
    // "pillar_shaping_recipe_discovery_cargo_desc"
    // "pillar_shaping_recipe_discovery_item_desc"
    // "pillar_shaping_recipe_discovery_knowledge_desc"
    // "player_action_desc"
    // "player_housing_desc"
    // "premium_item_desc"
    // "premium_service_desc"
    // "private_parameters_desc"
    // "prospecting_desc"
    // "quest_chain_desc"
    // "quest_drop_desc"
    // "quest_stage_desc"
    // "reserved_name_desc"
    // "resource_clump_desc"
    // "resource_desc"
    // "resource_growth_recipe_desc"
    // "resource_placement_recipe_desc"
    // "resource_placement_recipe_discovery_cargo_desc"
    // "resource_placement_recipe_discovery_item_desc"
    // "resource_placement_recipe_discovery_knowledge_desc"
    // "secondary_knowledge_desc"
    // "single_resource_to_clump_desc"
    // "skill_desc"
    // "targeting_matrix_desc"
    // "teleport_item_desc"
    // "terraform_recipe_desc"
    // "tool_desc"
    // "tool_type_desc"
    // "traveler_task_desc"
    // "traveler_task_knowledge_requirement_desc"
    // "traveler_trade_order_desc"
    // "wall_desc"
    // "weapon_desc"
    // "weapon_type_desc"
    // "wind_dbg_desc"
    // "wind_params_desc"

    let sql_subscribe = vec![
        ("equipment_desc", "SELECT * FROM equipment_desc"),
        ("equipment_state", "SELECT * FROM equipment_state"),
        ("location_state", "SELECT location_state.* FROM location_state JOIN building_state ON location_state.entity_id = building_state.entity_id"),
        ("location_state", "SELECT location_state.* FROM location_state JOIN portal_state ON location_state.entity_id = portal_state.entity_id"),
        // ("location_state", "SELECT * FROM location_state where dimension = 1"),
        (
            "mobile_entity_state",
            "SELECT mobile_entity_state.* FROM mobile_entity_state JOIN player_state ON mobile_entity_state.entity_id = player_state.entity_id",
        ),
    ];

    if config.spacetimedb.skip_state_tables {
        tracing::warn!(
            region = database,
            "RSS DIAGNOSTIC: zero table subscriptions; connection and workers only (spacetimedb.skip_state_tables=true)"
        );
    }

    // let tmp_region_number = region_number;
    // let tmp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    // let tmp_player_state_tx = player_state_tx.clone();
    // let tmp_player_username_state_tx = player_username_state_tx.clone();
    // let tmp_experience_state_tx = experience_state_tx.clone();
    // let tmp_inventory_state_tx = inventory_state_tx.clone();
    // let tmp_item_desc_tx = item_desc_tx.clone();
    // let tmp_cargo_desc_tx = cargo_desc_tx.clone();
    // let tmp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
    // let tmp_deployable_state_tx = deployable_state_tx.clone();
    // let tmp_claim_state_tx = claim_state_tx.clone();
    // let tmp_claim_local_state_tx = claim_local_state_tx.clone();
    // let tmp_claim_member_state_tx = claim_member_state_tx.clone();
    // let tmp_skill_desc_tx = skill_desc_tx.clone();
    // let tmp_claim_tech_state_tx = claim_tech_state_tx.clone();
    // let tmp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
    // let tmp_building_state_tx = building_state_tx.clone();
    // let tmp_building_desc_tx = building_desc_tx.clone();
    // let tmp_location_state_tx = location_state_tx.clone();
    // let tmp_building_nickname_state_tx = building_nickname_state_tx.clone();
    // let tmp_crafting_recipe_desc_tx = crafting_recipe_desc_tx.clone();
    // let tmp_item_list_desc_tx = item_list_desc_tx.clone();
    // let tmp_traveler_task_desc_tx = traveler_task_desc_tx.clone();
    // let tmp_traveler_task_state_tx = traveler_task_state_tx.clone();
    // let tmp_trade_order_state_tx = trade_order_state_tx.clone();
    // let tmp_buy_order_state_tx = buy_order_state_tx.clone();
    // let tmp_sell_order_state_tx = sell_order_state_tx.clone();
    // let tmp_user_state_tx = user_state_tx.clone();
    // let tmp_npc_desc_tx = npc_desc_tx.clone();
    // let tmp_collectible_desc_tx = collectible_desc_tx.clone();
    // let tmp_interior_network_desc_tx = interior_network_desc_tx.clone();
    // let tmp_dimension_description_state_tx = dimension_description_state_tx.clone();
    // let tmp_player_housing_state_tx = player_housing_state_tx.clone();
    // let tmp_permission_state_tx = permission_state_tx.clone();
    // let tmp_portal_state_tx = portal_state_tx.clone();
    // let tmp_resource_desc_tx = resource_desc_tx.clone();
    // let tmp_extraction_recipe_desc_tx = extraction_recipe_desc_tx.clone();
    // let tmp_progressive_action_state_tx = progressive_action_state_tx.clone();

    let subscription_builder = ctx
        .subscription_builder()
        // .on_applied(move |ctx: &SubscriptionEventContext| {
        //     tracing::debug!(region = region_number, "Handle Subscription response");
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "cargo_desc".to_string())]
        //     )
        //     .set(ctx.db.cargo_desc().count() as f64);
        //
        //     let cargo_desc = ctx.db.cargo_desc().iter().collect::<Vec<_>>();
        //     if !cargo_desc.is_empty() {
        //         send_worker_message(
        //             "cargo_desc",
        //             &tmp_cargo_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: cargo_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "player_username_state".to_string())]
        //     )
        //     .set(ctx.db.player_username_state().count() as f64);
        //     let player_username_state = ctx.db.player_username_state().iter().collect::<Vec<_>>();
        //     if !player_username_state.is_empty() {
        //         send_worker_message(
        //             "player_username_state",
        //             &tmp_player_username_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: player_username_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "claim_local_state".to_string())]
        //     )
        //     .set(ctx.db.claim_local_state().count() as f64);
        //     let claim_local_state = ctx.db.claim_local_state().iter().collect::<Vec<_>>();
        //     if !claim_local_state.is_empty() {
        //         send_worker_message(
        //             "claim_local_state",
        //             &tmp_claim_local_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: claim_local_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "claim_state".to_string())]
        //     )
        //     .set(ctx.db.claim_state().count() as f64);
        //     let claim_state = ctx.db.claim_state().iter().collect::<Vec<_>>();
        //     if !claim_state.is_empty() {
        //         send_worker_message(
        //             "claim_state",
        //             &tmp_claim_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: claim_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "deployable_state".to_string())]
        //     )
        //     .set(ctx.db.deployable_state().count() as f64);
        //     let deployable_state = ctx.db.deployable_state().iter().collect::<Vec<_>>();
        //     if !deployable_state.is_empty() {
        //         send_worker_message(
        //             "deployable_state",
        //             &tmp_deployable_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: deployable_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "item_desc".to_string())]
        //     )
        //     .set(ctx.db.item_desc().count() as f64);
        //     let item_desc = ctx.db.item_desc().iter().collect::<Vec<_>>();
        //     if !item_desc.is_empty() {
        //         send_worker_message(
        //             "item_desc",
        //             &tmp_item_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: item_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "skill_desc".to_string())]
        //     )
        //     .set(ctx.db.skill_desc().count() as f64);
        //     let skill_desc = ctx.db.skill_desc().iter().collect::<Vec<_>>();
        //     if !skill_desc.is_empty() {
        //         send_worker_message(
        //             "skill_desc",
        //             &tmp_skill_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: skill_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "building_desc".to_string())]
        //     )
        //     .set(ctx.db.building_desc().count() as f64);
        //     let building_desc = ctx.db.building_desc().iter().collect::<Vec<_>>();
        //     if !building_desc.is_empty() {
        //         send_worker_message(
        //             "building_desc",
        //             &tmp_building_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: building_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "claim_tech_desc".to_string())]
        //     )
        //     .set(ctx.db.claim_tech_desc().count() as f64);
        //     let claim_tech_desc = ctx.db.claim_tech_desc().iter().collect::<Vec<_>>();
        //     if !claim_tech_desc.is_empty() {
        //         send_worker_message(
        //             "claim_tech_desc",
        //             &tmp_claim_tech_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: claim_tech_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "crafting_recipe_desc".to_string())]
        //     )
        //     .set(ctx.db.crafting_recipe_desc().count() as f64);
        //     let crafting_recipe_desc = ctx.db.crafting_recipe_desc().iter().collect::<Vec<_>>();
        //     if !crafting_recipe_desc.is_empty() {
        //         send_worker_message(
        //             "crafting_recipe_desc",
        //             &tmp_crafting_recipe_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: crafting_recipe_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "item_list_desc".to_string())]
        //     )
        //     .set(ctx.db.item_list_desc().count() as f64);
        //     let item_list_desc = ctx.db.item_list_desc().iter().collect::<Vec<_>>();
        //     if !item_list_desc.is_empty() {
        //         send_worker_message(
        //             "item_list_desc",
        //             &tmp_item_list_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: item_list_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "traveler_task_desc".to_string())]
        //     )
        //     .set(ctx.db.traveler_task_desc().count() as f64);
        //     let traveler_task_desc = ctx.db.traveler_task_desc().iter().collect::<Vec<_>>();
        //     if !traveler_task_desc.is_empty() {
        //         send_worker_message(
        //             "traveler_task_desc",
        //             &tmp_traveler_task_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: traveler_task_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "npc_desc".to_string())]
        //     )
        //     .set(ctx.db.npc_desc().count() as f64);
        //     let npc_desc = ctx.db.npc_desc().iter().collect::<Vec<_>>();
        //     if !npc_desc.is_empty() {
        //         send_worker_message(
        //             "npc_desc",
        //             &tmp_npc_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: npc_desc,
        //             },
        //         );
        //     }
        //
        //     let npc_desc = ctx.db.progressive_action_state().iter().collect::<Vec<_>>();
        //     if !npc_desc.is_empty() {
        //         // @todo Write the SpacetimeUpdateMessages::Initial part
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "extraction_recipe_desc".to_string())]
        //     )
        //     .set(ctx.db.extraction_recipe_desc().count() as f64);
        //     let extraction_recipe_desc = ctx.db.extraction_recipe_desc().iter().collect::<Vec<_>>();
        //     if !extraction_recipe_desc.is_empty() {
        //         send_worker_message(
        //             "extraction_recipe_desc",
        //             &tmp_extraction_recipe_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: extraction_recipe_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "inventory_state".to_string())]
        //     )
        //     .set(ctx.db.inventory_state().count() as f64);
        //     let inventory_state = ctx.db.inventory_state().iter().collect::<Vec<_>>();
        //     if !inventory_state.is_empty() {
        //         send_worker_message(
        //             "inventory_state",
        //             &tmp_inventory_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: inventory_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "claim_member_state".to_string())]
        //     )
        //     .set(ctx.db.claim_member_state().count() as f64);
        //     let claim_member_state = ctx.db.claim_member_state().iter().collect::<Vec<_>>();
        //     if !claim_member_state.is_empty() {
        //         send_worker_message(
        //             "claim_member_state",
        //             &tmp_claim_member_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: claim_member_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "experience_state".to_string())]
        //     )
        //     .set(ctx.db.experience_state().count() as f64);
        //     let experience_state = ctx.db.experience_state().iter().collect::<Vec<_>>();
        //     if !experience_state.is_empty() {
        //         send_worker_message(
        //             "experience_state",
        //             &tmp_experience_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: experience_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "mobile_entity_state".to_string())]
        //     )
        //     .set(ctx.db.mobile_entity_state().count() as f64);
        //     let mobile_entity_state = ctx.db.mobile_entity_state().iter().collect::<Vec<_>>();
        //     if !mobile_entity_state.is_empty() {
        //         send_worker_message(
        //             "mobile_entity_state",
        //             &tmp_mobile_entity_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: mobile_entity_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "player_state".to_string())]
        //     )
        //     .set(ctx.db.player_state().count() as f64);
        //     let player_state = ctx.db.player_state().iter().collect::<Vec<_>>();
        //     if !player_state.is_empty() {
        //         send_worker_message(
        //             "player_state",
        //             &tmp_player_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: player_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "vault_state".to_string())]
        //     )
        //     .set(ctx.db.vault_state().count() as f64);
        //     let vault_state = ctx.db.vault_state().iter().collect::<Vec<_>>();
        //     if !vault_state.is_empty() {
        //         send_worker_message(
        //             "vault_state",
        //             &tmp_vault_state_collectibles_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: vault_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "claim_tech_state".to_string())]
        //     )
        //     .set(ctx.db.claim_tech_state().count() as f64);
        //     let claim_tech_state = ctx.db.claim_tech_state().iter().collect::<Vec<_>>();
        //     if !claim_tech_state.is_empty() {
        //         send_worker_message(
        //             "claim_tech_state",
        //             &tmp_claim_tech_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: claim_tech_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "building_state".to_string())]
        //     )
        //     .set(ctx.db.building_state().count() as f64);
        //     let building_state = ctx.db.building_state().iter().collect::<Vec<_>>();
        //     if !building_state.is_empty() {
        //         send_worker_message(
        //             "building_state",
        //             &tmp_building_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: building_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "location_state".to_string())]
        //     )
        //     .set(ctx.db.location_state().count() as f64);
        //     let location_state = ctx.db.location_state().iter().collect::<Vec<_>>();
        //     if !location_state.is_empty() {
        //         send_worker_message(
        //             "location_state",
        //             &tmp_location_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: location_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "building_nickname_state".to_string())]
        //     )
        //     .set(ctx.db.building_nickname_state().count() as f64);
        //     let building_nickname_state =
        //         ctx.db.building_nickname_state().iter().collect::<Vec<_>>();
        //     if !building_nickname_state.is_empty() {
        //         send_worker_message(
        //             "building_nickname_state",
        //             &tmp_building_nickname_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: building_nickname_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "traveler_task_state".to_string())]
        //     )
        //     .set(ctx.db.traveler_task_state().count() as f64);
        //     let traveler_task_state = ctx.db.traveler_task_state().iter().collect::<Vec<_>>();
        //     if !traveler_task_state.is_empty() {
        //         send_worker_message(
        //             "traveler_task_state",
        //             &tmp_traveler_task_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: traveler_task_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "trade_order_state".to_string())]
        //     )
        //     .set(ctx.db.trade_order_state().count() as f64);
        //     let trade_order_state = ctx.db.trade_order_state().iter().collect::<Vec<_>>();
        //     if !trade_order_state.is_empty() {
        //         send_worker_message(
        //             "trade_order_state",
        //             &tmp_trade_order_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: trade_order_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "user_state".to_string())]
        //     )
        //     .set(ctx.db.user_state().count() as f64);
        //     let user_state = ctx.db.user_state().iter().collect::<Vec<_>>();
        //     if !user_state.is_empty() {
        //         send_worker_message(
        //             "user_state",
        //             &tmp_user_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: user_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "sell_order_state".to_string())]
        //     )
        //     .set(ctx.db.sell_order_state().count() as f64);
        //     let sell_order_state = ctx.db.sell_order_state().iter().collect::<Vec<_>>();
        //     if !sell_order_state.is_empty() {
        //         send_worker_message(
        //             "sell_order_state",
        //             &tmp_sell_order_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: sell_order_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "buy_order_state".to_string())]
        //     )
        //     .set(ctx.db.buy_order_state().count() as f64);
        //     let buy_order_state = ctx.db.buy_order_state().iter().collect::<Vec<_>>();
        //     if !buy_order_state.is_empty() {
        //         send_worker_message(
        //             "buy_order_state",
        //             &tmp_buy_order_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: buy_order_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "collectible_desc".to_string())]
        //     )
        //     .set(ctx.db.collectible_desc().count() as f64);
        //     let collectible_desc = ctx.db.collectible_desc().iter().collect::<Vec<_>>();
        //     if !collectible_desc.is_empty() {
        //         send_worker_message(
        //             "collectible_desc",
        //             &tmp_collectible_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: collectible_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "interior_network_desc".to_string())]
        //     )
        //     .set(ctx.db.interior_network_desc().count() as f64);
        //     let interior_network_desc = ctx.db.interior_network_desc().iter().collect::<Vec<_>>();
        //     if !interior_network_desc.is_empty() {
        //         send_worker_message(
        //             "interior_network_desc",
        //             &tmp_interior_network_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: interior_network_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "dimension_description_state".to_string())]
        //     )
        //     .set(ctx.db.dimension_description_state().count() as f64);
        //     let dimension_description_state = ctx
        //         .db
        //         .dimension_description_state()
        //         .iter()
        //         .collect::<Vec<_>>();
        //     if !dimension_description_state.is_empty() {
        //         send_worker_message(
        //             "dimension_description_state",
        //             &tmp_dimension_description_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: dimension_description_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "player_housing_state".to_string())]
        //     )
        //     .set(ctx.db.player_housing_state().count() as f64);
        //     let player_housing_state = ctx.db.player_housing_state().iter().collect::<Vec<_>>();
        //     if !player_housing_state.is_empty() {
        //         send_worker_message(
        //             "player_housing_state",
        //             &tmp_player_housing_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: player_housing_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "permission_state".to_string())]
        //     )
        //     .set(ctx.db.permission_state().count() as f64);
        //     let permission_state = ctx.db.permission_state().iter().collect::<Vec<_>>();
        //     if !permission_state.is_empty() {
        //         send_worker_message(
        //             "permission_state",
        //             &tmp_permission_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: permission_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "portal_state".to_string())]
        //     )
        //     .set(ctx.db.portal_state().count() as f64);
        //     let portal_state = ctx.db.portal_state().iter().collect::<Vec<_>>();
        //     if !portal_state.is_empty() {
        //         send_worker_message(
        //             "portal_state",
        //             &tmp_portal_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: portal_state,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "resource_desc".to_string())]
        //     )
        //     .set(ctx.db.resource_desc().count() as f64);
        //     let resource_desc = ctx.db.resource_desc().iter().collect::<Vec<_>>();
        //     if !resource_desc.is_empty() {
        //         send_worker_message(
        //             "resource_desc",
        //             &tmp_resource_desc_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: resource_desc,
        //             },
        //         );
        //     }
        //
        //     metrics::gauge!(
        //         "worker_queue_initial_batch_size",
        //         &[("worker", "progressive_action_state".to_string())]
        //     )
        //     .set(ctx.db.progressive_action_state().count() as f64);
        //     let progressive_action_state =
        //         ctx.db.progressive_action_state().iter().collect::<Vec<_>>();
        //     if !progressive_action_state.is_empty() {
        //         send_worker_message(
        //             "progressive_action_state",
        //             &tmp_progressive_action_state_tx,
        //             SpacetimeUpdateMessages::Initial {
        //                 database_name: tmp_region_number,
        //                 data: progressive_action_state,
        //             },
        //         );
        //     }
        //
        //     // for resource_desc in ctx.db.user_state().iter() {
        //     //     if resource_desc.entity_id == 504403158285774600 {
        //     //         println!("ID: {} Name: {:?}", resource_desc.identity, resource_desc.entity_id);
        //     //     }
        //     // }
        //     //
        //     // for resource_desc in ctx.db.identity_role().iter() {
        //     //     println!("ID: {} Name: {:?}", resource_desc.identity, resource_desc.role);
        //     // }
        //
        //     tracing::debug!(region = region_number, "Handled Subscription response");
        // })
        .on_error(move |ctx: &ErrorContext, err: Error| {
            tracing::warn!(
                region = region_number,
                "Subscription failed: {} {:?}",
                err,
                ctx.event
            );
            // std::process::exit(1);
        });
    let queries = subscription_queries(
        &tables_to_subscribe,
        &sql_subscribe,
        *remove_desc,
        config.spacetimedb.skip_state_tables,
    );
    // Do not send an empty subscription request: keep the connection running
    // without subscriptions when isolating connection/worker RSS.
    let portal_dimensions = (!config.spacetimedb.skip_state_tables)
        .then(|| portal_locations::listen(&ctx));
    if !queries.is_empty() {
        subscription_builder.subscribe(queries);
    }

    let tmp_disconnect_db_name = database.to_string().clone();
    let tmp_disconnect_global_app_state = global_app_state.clone();

    // The SDK must keep processing events while we wait for subscriptions to apply.
    let connection = run_connection_until_shutdown(&ctx, &global_app_state.shutdown.stop);
    tokio::pin!(connection);
    let result = tokio::select! {
        result = &mut connection => result,
        result = async {
            // if let Some(dimensions) = portal_dimensions {
            //     tokio::try_join!(
            //         overworld::subscribe(&ctx),
            //         // portal_locations::subscribe(&ctx, dimensions),
            //     )?;
            // }
            Ok::<(), anyhow::Error>(())
        } => match result {
            Ok(()) => connection.await,
            Err(error) => Err(error),
        },
    };

    metrics::gauge!(
        "bitcraft_database_connected",
        &[("region", tmp_disconnect_db_name.clone())]
    )
    .set(0);

    tmp_disconnect_global_app_state
        .connection_state
        .insert(tmp_disconnect_db_name.clone(), false);

    result.map_err(anyhow::Error::from)
}

pub fn start_websocket_bitcraft_logic(config: Config, global_app_state: AppState) {
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
            Duration::from_millis(200),
        );

        let player_username_state_worker =
            crate::player_state::bitcraft::PlayerUsernameStateWorker::new(
                global_app_state.clone(),
                3000,
                Duration::from_millis(200),
            );

        let experience_state_worker = crate::leaderboard::bitcraft::ExperienceStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );

        let location_state_worker = crate::location_state::bitcraft::LocationStateWorker::new(
            global_app_state.clone(),
            10000,
            Duration::from_millis(200),
        );
        let inventory_state_worker = inventory_bitcraft::InventoryStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );

        let (item_desc_tx, item_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);
        let (item_list_desc_tx, item_list_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (cargo_desc_tx, cargo_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (vault_state_collectibles_tx, vault_state_collectibles_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let (deployable_state_tx, deployable_state_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let claim_state_worker = bitcraft::ClaimStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );

        let claim_local_state_worker = bitcraft::ClaimLocalStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );

        let claim_member_state_worker = bitcraft::ClaimMemberStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );

        let (skill_desc_tx, skill_desc_rx) = worker_channel::shutdown_channel(&global_app_state.shutdown);

        let claim_tech_state_worker = bitcraft::ClaimTechStateWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
        );
        let claim_tech_desc_worker = bitcraft::ClaimTechDescWorker::new(
            global_app_state.clone(),
            3000,
            Duration::from_millis(200),
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

        let mut remove_desc = false;

        let tmp_conf = config.clone();
        let tmp_global_app_state = global_app_state.clone();
        let tmp_database = config.spacetimedb.database.clone();

        // let result = connect_to_db_global(tmp_global_app_state, &tmp_conf, &tmp_database);
        //
        // if let Err(error) = result {
        //     tracing::error!(
        //         region = tmp_database,
        //         error = error.to_string(),
        //         "Error creating connection to {tmp_database} on {}",
        //         tmp_conf.spacetimedb_url()
        //     )
        // };

        config
            .spacetimedb
            .databases
            .iter()
            .filter(|value| !value.trim().is_empty())
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
                let tmp_conf = config.clone();
                let tmp_global_app_state = global_app_state.clone();
                let tmp_remove_desc = remove_desc;
                let tmp_database = database.clone();

                tmp_global_app_state.shutdown.clone().spawn_connection(async move {
                    let stop = tmp_global_app_state.shutdown.stop.clone();
                    if stop.is_cancelled() { return; }
                    let startup_wait_ms = {
                        let startup_wait_ranges_ms =
                            [(0, 500), (500, 2_000), (2_000, 5_000), (5_000, 15_000)];
                        let range_index = rand::rng().random_range(0..startup_wait_ranges_ms.len());
                        let (min_wait_ms, max_wait_ms) = startup_wait_ranges_ms[range_index];

                        rand::rng().random_range(min_wait_ms..=max_wait_ms)
                    };

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

                    if startup_wait_ms > 0 {
                        tracing::debug!(
                            region = tmp_database,
                            wait_ms = startup_wait_ms,
                            database_name = tmp_database.clone(),
                            spacetime_url = tmp_conf.spacetimedb_url(region),
                            "Delaying websocket connection startup"
                        );
                        tokio::select! { biased;
                            _ = stop.cancelled() => return,
                            _ = tokio::time::sleep(Duration::from_millis(startup_wait_ms)) => {},
                        }
                    }

                    let mut tries = 0;


                    let claim_event_tx = crate::claims::events::start_persistence_worker(
                        tmp_global_app_state.clone(),
                        region,
                    );

                    loop {
                        if stop.is_cancelled() { break; }
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
                            &claim_event_tx,
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

                        let jitter_secs = rand::rng().random_range(0..=90);
                        let wait = reconnect_delay(tries, jitter_secs);

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
            Duration::from_millis(200),
        );
        start_worker_item_desc(
            global_app_state.clone(),
            item_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_cargo_desc(
            global_app_state.clone(),
            cargo_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_deployable_state(
            global_app_state.clone(),
            deployable_state_rx,
            3000,
            Duration::from_millis(200),
        );
        claim_state_worker.start(&global_app_state.shutdown);
        claim_local_state_worker.start(&global_app_state.shutdown);
        claim_member_state_worker.start(&global_app_state.shutdown);
        start_worker_skill_desc(
            global_app_state.clone(),
            skill_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        claim_tech_state_worker.start(&global_app_state.shutdown);
        claim_tech_desc_worker.start(&global_app_state.shutdown);
        start_worker_building_state(
            global_app_state.clone(),
            building_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_building_desc(
            global_app_state.clone(),
            building_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_building_nickname_state(
            global_app_state.clone(),
            building_nickname_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_crafting_recipe_desc(
            global_app_state.clone(),
            crafting_recipe_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_item_list_desc(
            global_app_state.clone(),
            item_list_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_traveler_task_desc(
            global_app_state.clone(),
            traveler_task_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_traveler_task_state(
            global_app_state.clone(),
            traveler_task_state_rx,
            6000,
            Duration::from_millis(200),
        );
        start_worker_trade_order_state(
            global_app_state.clone(),
            trade_order_state_rx,
            6000,
            Duration::from_millis(200),
        );

        start_worker_npc_desc(
            global_app_state.clone(),
            npc_desc_rx,
            3000,
            Duration::from_millis(200),
        );

        start_worker_buy_order_state(
            global_app_state.clone(),
            buy_order_state_rx,
            3000,
            Duration::from_millis(200),
        );

        start_worker_sell_order_state(
            global_app_state.clone(),
            sell_order_state_rx,
            3000,
            Duration::from_millis(200),
        );

        start_worker_collectible_desc(
            global_app_state.clone(),
            collectible_desc_rx,
            3000,
            Duration::from_millis(200),
        );

        start_worker_user_state(global_app_state.clone(), user_state_rx);

        start_worker_interior_network_desc(
            global_app_state.clone(),
            interior_network_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_dimension_description_state(
            global_app_state.clone(),
            dimension_description_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_player_housing_state(
            global_app_state.clone(),
            player_housing_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_permission_state(
            global_app_state.clone(),
            permission_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_portal_state(
            global_app_state.clone(),
            portal_state_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_resource_desc(
            global_app_state.clone(),
            resource_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_extraction_recipe_desc(
            global_app_state.clone(),
            extraction_recipe_desc_rx,
            3000,
            Duration::from_millis(200),
        );
        start_worker_progressive_action_state(
            global_app_state.clone(),
            progressive_action_state_rx,
            3000,
            Duration::from_millis(200),
        );
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
    Unsubscribe { topic: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "t", content = "c")]
pub(crate) enum OutboundWebSocketMessages {
    SubscribedTopics(Vec<String>),
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
    fn new<const N: usize>(topics: [(Cow<'static, str>, Option<i64>); N]) -> Self {
        assert!(N <= 3);
        let mut topics = topics.into_iter();
        Self(std::array::from_fn(|_| topics.next()))
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
    pub fn topics(&self) -> Option<BroadcastTopics> {
        match self {
            OutboundWebSocketMessages::Experience {
                skill_name,
                user_id,
                ..
            } => Some(BroadcastTopics::new([
                (format!("experience:{skill_name}").into(), Some(*user_id)),
                ("experience".into(), Some(*user_id)),
            ])),
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
            OutboundWebSocketMessages::PlayerUsername { entity_id, .. } => Some(
                BroadcastTopics::new([("player_username".into(), Some(*entity_id))]),
            ),
            OutboundWebSocketMessages::MobileEntityState(mobile_entity_state) => {
                Some(BroadcastTopics::new([(
                    "mobile_entity_state".into(),
                    Some(mobile_entity_state.entity_id as i64),
                )]))
            }
            // WebSocketMessages::ClaimDescriptionState(claim) => {
            //     Some(vec![("claim".to_string(), claim.entity_id)])
            // }
            OutboundWebSocketMessages::TotalExperience { user_id, .. } => Some(
                BroadcastTopics::new([("total_experience".into(), Some(*user_id))]),
            ),
            OutboundWebSocketMessages::TotalProfessionExperience { user_id, .. } => Some(
                BroadcastTopics::new([("total_profession_experience".into(), Some(*user_id))]),
            ),
            OutboundWebSocketMessages::TotalAdventureExperience { user_id, .. } => Some(
                BroadcastTopics::new([("total_adventure_experience".into(), Some(*user_id))]),
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
            } => Some(BroadcastTopics::new([(
                "inventory_update".into(),
                Some(resolved_inventory.entity_id),
            )])),
            OutboundWebSocketMessages::InventoryRemove {
                resolved_inventory, ..
            } => Some(BroadcastTopics::new([
                (
                    "inventory_remove".into(),
                    Some(resolved_inventory.entity_id),
                ),
                (
                    "inventory_remove_owner".into(),
                    Some(resolved_inventory.entity_id),
                ),
            ])),
            OutboundWebSocketMessages::InventoryInsert {
                resolved_inventory,
                player_owner_id,
                ..
            } => Some(BroadcastTopics::new([
                (
                    "inventory_insert".into(),
                    Some(resolved_inventory.entity_id),
                ),
                (
                    "inventory_insert_owner".into(),
                    Some(resolved_inventory.owner_entity_id),
                ),
                (
                    "inventory_insert_player_owner".into(),
                    Some(*player_owner_id),
                ),
            ])),
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

    #[test]
    fn reconnect_backoff_is_capped_and_jitter_is_bounded() {
        assert_eq!(reconnect_delay(0, 0), Duration::from_secs(45));
        assert_eq!(reconnect_delay(1, 90), Duration::from_secs(135));
        assert_eq!(reconnect_delay(2, 0), Duration::from_secs(90));
        assert_eq!(reconnect_delay(7, 0), Duration::from_secs(300));
        assert_eq!(
            reconnect_delay(u32::MAX, u64::MAX),
            Duration::from_secs(390)
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
