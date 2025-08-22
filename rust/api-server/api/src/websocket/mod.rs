use crate::AppState;
use crate::auction_listing_state::bitcraft::{
    start_worker_buy_order_state, start_worker_sell_order_state,
};
use crate::buildings::bitcraft::{
    start_worker_building_desc, start_worker_building_nickname_state, start_worker_building_state,
};
use crate::cargo_desc::bitcraft::start_worker_cargo_desc;
use crate::claims::bitcraft::{
    start_worker_claim_local_state, start_worker_claim_member_state, start_worker_claim_state,
    start_worker_claim_tech_desc, start_worker_claim_tech_state,
};
use crate::collectible_desc::bitcraft::start_worker_collectible_desc;
use crate::config::Config;
use crate::crafting_recipe_desc::bitcraft::start_worker_crafting_recipe_desc;
use crate::deployable_state::bitcraft::start_worker_deployable_state;
use crate::inventory::bitcraft::start_worker_inventory_state;
use crate::item_list_desc::bitcraft::start_worker_item_list_desc;
use crate::items::bitcraft::start_worker_item_desc;
use crate::leaderboard::bitcraft::start_worker_experience_state;
use crate::location_state::bitcraft::start_worker_location_state;
use crate::mobile_entity_state::bitcraft::start_worker_mobile_entity_state;
use crate::npc_desc::bitcraft::start_worker_npc_desc;
use crate::player_state::bitcraft::{
    start_worker_player_state, start_worker_player_username_state,
};
use crate::skill_descriptions::bitcraft::start_worker_skill_desc;
use crate::trading_orders::bitcraft::start_worker_trade_order_state;
use crate::traveler_task_desc::bitcraft::start_worker_traveler_task_desc;
use crate::traveler_task_state::bitcraft::start_worker_traveler_task_state;
use crate::user_state::bitcraft::start_worker_user_state;
use crate::vault_state::bitcraft::start_worker_vault_state_collectibles;
use game_module::module_bindings::*;
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::__codegen::{self as __sdk};
use spacetimedb_sdk::{
    Compression, DbContext, Error, Event, Identity, Table, TableWithPrimaryKey, Timestamp,
    credentials,
};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

fn connect_to_db(
    global_app_state: AppState,
    token: String,
    db_name: &str,
    db_host: &str,
) -> spacetimedb_sdk::Result<DbConnection> {
    let tmp_global_app_state = global_app_state.clone();
    let tmp_disconnect_global_app_state = global_app_state.clone();
    let tmp_db_name = db_name.to_owned();
    let tmp_disconnect_db_name = tmp_db_name.clone();

    DbConnection::builder()
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(move |_ctx, identity, token| {
            tracing::info!("Connected to server {tmp_db_name} with {identity}");
            metrics::gauge!(
                "bitcraft_database_connected",
                &[("region", tmp_db_name.clone())]
            )
            .set(1);

            tmp_global_app_state
                .connection_state
                .insert(tmp_db_name, true);
            if let Err(e) = creds_store().save(token) {
                tracing::warn!("Failed to save credentials: {:?}", e);
            }
        })
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
                tracing::error!("Disconnected: {} : {}", err, tmp_disconnect_db_name);
                // std::process::exit(1);
            } else {
                tracing::error!("Disconnected {}.", tmp_disconnect_db_name);
                // std::process::exit(0);
            }
        })
        // If the user has previously connected, we'll have saved a token in the `on_connect` callback.
        // In that case, we'll load it and pass it to `with_token`,
        // so we can re-authenticate as the same `Identity`.
        // .with_token(creds_store().load().expect("Error loading credentials"))
        .with_token(Some(token.trim()))
        // Set the database name we chose when we called `spacetime publish`.
        .with_module_name(db_name)
        // Set the URI of the SpacetimeDB host that's running our database.
        .with_uri(db_host)
        // Finalize configuration and connect!
        .with_compression(Compression::Brotli)
        .build()
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
    tracing::warn!("Subscription failed: {}", err);
    // std::process::exit(1);
}

fn creds_store() -> credentials::File {
    credentials::File::new("bitcraft-ea")
}

/// Our `on_connect_error` callback: print the error, then exit the process.
fn on_connect_error(_ctx: &ErrorContext, err: Error) {
    tracing::warn!("Connection error: {:?}", err);
    // std::process::exit(1);
}

macro_rules! setup_spacetime_db_listeners {
    ($ctx:expr, $db_table_method:ident, $tx_channel:ident, $state_type:ty, $database_name_expr:expr $(, skipp_subscribe_applied => $skipp_subscribe_applied:expr)?) => {
        let table_name_str = stringify!($db_table_method);
        let database_name_runtime_string = $database_name_expr.to_string();
        let database_name_arc: Arc<String> = Arc::new($database_name_expr.to_string());
        let skipp_subscribe_applied = true;

        $( skipp_subscribe_applied = $skipp_subscribe_applied; )?

        let temp_tx = $tx_channel.clone();
        let labels_update: [(&'static str, Cow<'static, str>); 3] = [
            ("table", Cow::Borrowed(table_name_str)),
            // Clone the runtime string for this specific label set
            ("database", Cow::Owned(database_name_runtime_string.clone())),
            ("type", Cow::Borrowed("update")),
        ];

        let tmp_database_name_arc = database_name_arc.clone();
        $ctx.db.$db_table_method().on_update(
            // Use $state_type for the old and new parameters
            move |ctx: &EventContext, old: &$state_type, new: &$state_type| {
                metrics::counter!("game_message_events", &labels_update).increment(1);
                let mut caller_identity = None;
                let mut reducer = None;
                let mut timestamp = None;
                if let Event::Reducer(event) = ctx.event.clone() {
                    caller_identity = Some(event.caller_identity);
                    reducer = Some(event.reducer);
                    timestamp = Some(event.timestamp)
                }
                temp_tx
                    .send(SpacetimeUpdateMessages::Update {
                        event: ctx.event.clone(),
                        database_name: tmp_database_name_arc.clone(),
                        old: old.clone(),
                        new: new.clone(),
                        caller_identity,
                        reducer,
                        timestamp,
                    })
                    .unwrap();
            },
        );

        let temp_tx = $tx_channel.clone();
        let labels_insert: [(&'static str, Cow<'static, str>); 3] = [
            ("table", Cow::Borrowed(table_name_str)),
            // Clone again for this label set
            ("database", Cow::Owned(database_name_runtime_string.clone())),
            ("type", Cow::Borrowed("insert")),
        ];
        let tmp_database_name_arc = database_name_arc.clone();
        $ctx.db.$db_table_method().on_insert(
            // Use $state_type for the new parameter
            move |ctx: &EventContext, new: &$state_type| {
                metrics::counter!("game_message_events", &labels_insert).increment(1);

                if skipp_subscribe_applied && let Event::SubscribeApplied = ctx.event {
                    return;
                }

                let mut caller_identity = None;
                let mut reducer = None;
                let mut timestamp = None;
                if let Event::Reducer(event) = ctx.event.clone() {
                    caller_identity = Some(event.caller_identity);
                    reducer = Some(event.reducer);
                    timestamp = Some(event.timestamp)
                }
                temp_tx
                    .send(SpacetimeUpdateMessages::Insert {
                        event: ctx.event.clone(),
                        database_name: tmp_database_name_arc.clone(),
                        new: new.clone(),
                        caller_identity,
                        reducer,
                        timestamp,
                    })
                    .unwrap();
            },
        );

        let temp_tx = $tx_channel.clone();
        let labels_delete: [(&'static str, Cow<'static, str>); 3] = [
            ("table", Cow::Borrowed(table_name_str)),
            // Clone for the final label set
            ("database", Cow::Owned(database_name_runtime_string.clone())),
            ("type", Cow::Borrowed("delete")),
        ];
        let tmp_database_name_arc = database_name_arc.clone();
        $ctx.db.$db_table_method().on_delete(
            // Use $state_type for the new parameter
            move |ctx: &EventContext, new: &$state_type| {
                metrics::counter!("game_message_events", &labels_delete).increment(1);
                let mut caller_identity = None;
                let mut reducer = None;
                let mut timestamp = None;
                if let Event::Reducer(event) = ctx.event.clone() {
                    caller_identity = Some(event.caller_identity);
                    reducer = Some(event.reducer);
                    timestamp = Some(event.timestamp)
                }
                temp_tx
                    .send(SpacetimeUpdateMessages::Remove {
                        event: ctx.event.clone(),
                        database_name: tmp_database_name_arc.clone(),
                        delete: new.clone(),
                        caller_identity,
                        reducer,
                        timestamp,
                    })
                    .unwrap();
            },
        );
    };
}

fn connect_to_db_global(
    global_app_state: AppState,
    config: &Config,
    database: &str,
) -> anyhow::Result<()> {
    let ctx = connect_to_db(
        global_app_state,
        config.spacetimedb.password.clone(),
        database,
        config.spacetimedb_url().as_ref(),
    )?;

    if !ctx.is_active() {
        tracing::error!(
            "Could not connect to the bitcraft server {} with module {database}",
            config.spacetimedb_url()
        );
        return Ok(());
    }

    tokio::spawn(async move {
        let _ = ctx.run_async().await;
    });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn connect_to_db_logic(
    global_app_state: AppState,
    config: &Config,
    database: &str,
    remove_desc: &bool,
    mobile_entity_state_tx: &UnboundedSender<SpacetimeUpdateMessages<MobileEntityState>>,
    player_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerState>>,
    player_username_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    experience_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    inventory_state_tx: &UnboundedSender<SpacetimeUpdateMessages<InventoryState>>,
    item_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ItemDesc>>,
    cargo_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CargoDesc>>,
    vault_state_collectibles_tx: &UnboundedSender<SpacetimeUpdateMessages<VaultState>>,
    deployable_state_tx: &UnboundedSender<SpacetimeUpdateMessages<DeployableState>>,
    claim_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimState>>,
    claim_local_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimLocalState>>,
    claim_member_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimMemberState>>,
    skill_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<SkillDesc>>,
    claim_tech_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechState>>,
    claim_tech_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechDesc>>,
    building_state_tx: &UnboundedSender<SpacetimeUpdateMessages<BuildingState>>,
    building_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<BuildingDesc>>,
    location_state_tx: &UnboundedSender<SpacetimeUpdateMessages<LocationState>>,
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
) -> anyhow::Result<()> {
    let ctx = connect_to_db(
        global_app_state,
        config.spacetimedb.password.clone(),
        database,
        config.spacetimedb_url().as_ref(),
    )?;

    if !ctx.is_active() {
        tracing::error!(
            "Could not connect to the bitcraft server {} with module {database}",
            config.spacetimedb_url()
        );
        return Ok(());
    }

    setup_spacetime_db_listeners!(
        ctx,
        mobile_entity_state,
        mobile_entity_state_tx,
        MobileEntityState,
        database
    );
    setup_spacetime_db_listeners!(ctx, player_state, player_state_tx, PlayerState, database);
    setup_spacetime_db_listeners!(
        ctx,
        player_username_state,
        player_username_state_tx,
        PlayerUsernameState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        experience_state,
        experience_state_tx,
        ExperienceState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        inventory_state,
        inventory_state_tx,
        InventoryState,
        database
    );
    setup_spacetime_db_listeners!(ctx, item_desc, item_desc_tx, ItemDesc, database);
    setup_spacetime_db_listeners!(ctx, cargo_desc, cargo_desc_tx, CargoDesc, database);
    setup_spacetime_db_listeners!(
        ctx,
        vault_state,
        vault_state_collectibles_tx,
        VaultState,
        database
    );
    setup_spacetime_db_listeners!(ctx, claim_state, claim_state_tx, ClaimState, database);
    setup_spacetime_db_listeners!(
        ctx,
        deployable_state,
        deployable_state_tx,
        DeployableState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_local_state,
        claim_local_state_tx,
        ClaimLocalState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_member_state,
        claim_member_state_tx,
        ClaimMemberState,
        database
    );
    setup_spacetime_db_listeners!(ctx, skill_desc, skill_desc_tx, SkillDesc, database);
    setup_spacetime_db_listeners!(
        ctx,
        claim_tech_state,
        claim_tech_state_tx,
        ClaimTechState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        claim_tech_desc,
        claim_tech_desc_tx,
        ClaimTechDesc,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        building_state,
        building_state_tx,
        BuildingState,
        database
    );
    setup_spacetime_db_listeners!(ctx, building_desc, building_desc_tx, BuildingDesc, database);
    setup_spacetime_db_listeners!(
        ctx,
        location_state,
        location_state_tx,
        LocationState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        building_nickname_state,
        building_nickname_state_tx,
        BuildingNicknameState,
        database
    );

    setup_spacetime_db_listeners!(
        ctx,
        crafting_recipe_desc,
        crafting_recipe_desc_tx,
        CraftingRecipeDesc,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        item_list_desc,
        item_list_desc_tx,
        ItemListDesc,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        traveler_task_desc,
        traveler_task_desc_tx,
        TravelerTaskDesc,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        traveler_task_state,
        traveler_task_state_tx,
        TravelerTaskState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        trade_order_state,
        trade_order_state_tx,
        TradeOrderState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        sell_order_state,
        sell_order_state_tx,
        AuctionListingState,
        database
    );
    setup_spacetime_db_listeners!(
        ctx,
        buy_order_state,
        buy_order_state_tx,
        AuctionListingState,
        database
    );
    setup_spacetime_db_listeners!(ctx, npc_desc, npc_desc_tx, NpcDesc, database);

    setup_spacetime_db_listeners!(ctx, user_state, user_state_tx, UserState, database);
    setup_spacetime_db_listeners!(
        ctx,
        collectible_desc,
        collectible_desc_tx,
        CollectibleDesc,
        database
    );

    let tables_to_subscribe = vec![
        "user_state",
        "mobile_entity_state",
        // "claim_tile_state",
        // "combat_action_desc",
        "item_desc",
        "item_list_desc",
        "cargo_desc",
        // "player_action_state",
        "crafting_recipe_desc",
        // "action_state",
        "player_state",
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
        "deployable_state",
        "inventory_state",
        "collectible_desc",
        "claim_tech_desc",
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
    ];

    let tmp_database = database.to_string().clone();
    let tmp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    let tmp_player_state_tx = player_state_tx.clone();
    let tmp_player_username_state_tx = player_username_state_tx.clone();
    let tmp_experience_state_tx = experience_state_tx.clone();
    let tmp_inventory_state_tx = inventory_state_tx.clone();
    let tmp_item_desc_tx = item_desc_tx.clone();
    let tmp_cargo_desc_tx = cargo_desc_tx.clone();
    let tmp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
    let tmp_deployable_state_tx = deployable_state_tx.clone();
    let tmp_claim_state_tx = claim_state_tx.clone();
    let tmp_claim_local_state_tx = claim_local_state_tx.clone();
    let tmp_claim_member_state_tx = claim_member_state_tx.clone();
    let tmp_skill_desc_tx = skill_desc_tx.clone();
    let tmp_claim_tech_state_tx = claim_tech_state_tx.clone();
    let tmp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
    let tmp_building_state_tx = building_state_tx.clone();
    let tmp_building_desc_tx = building_desc_tx.clone();
    let tmp_location_state_tx = location_state_tx.clone();
    let tmp_building_nickname_state_tx = building_nickname_state_tx.clone();
    let tmp_crafting_recipe_desc_tx = crafting_recipe_desc_tx.clone();
    let tmp_item_list_desc_tx = item_list_desc_tx.clone();
    let tmp_traveler_task_desc_tx = traveler_task_desc_tx.clone();
    let tmp_traveler_task_state_tx = traveler_task_state_tx.clone();
    let tmp_trade_order_state_tx = trade_order_state_tx.clone();
    let tmp_buy_order_state_tx = buy_order_state_tx.clone();
    let tmp_sell_order_state_tx = sell_order_state_tx.clone();
    let tmp_user_state_tx = user_state_tx.clone();
    let tmp_npc_desc_tx = npc_desc_tx.clone();
    let tmp_collectible_desc_tx = collectible_desc_tx.clone();
    ctx.subscription_builder()
        .on_applied(move |ctx: &SubscriptionEventContext| {
            tracing::debug!("Handle Subscription response");
            let database_name_arc: Arc<String> = Arc::new(tmp_database);

            let tmp_database_name_arc = database_name_arc.clone();
            let cargo_desc = ctx.db.cargo_desc().iter().collect::<Vec<_>>();
            if !cargo_desc.is_empty() {
                let _ = tmp_cargo_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: cargo_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let player_username_state = ctx.db.player_username_state().iter().collect::<Vec<_>>();
            if !player_username_state.is_empty() {
                let _ = tmp_player_username_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: player_username_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let claim_local_state = ctx.db.claim_local_state().iter().collect::<Vec<_>>();
            if !claim_local_state.is_empty() {
                let _ = tmp_claim_local_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: claim_local_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let claim_state = ctx.db.claim_state().iter().collect::<Vec<_>>();
            if !claim_state.is_empty() {
                let _ = tmp_claim_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: claim_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let deployable_state = ctx.db.deployable_state().iter().collect::<Vec<_>>();
            if !deployable_state.is_empty() {
                let _ = tmp_deployable_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: deployable_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let item_desc = ctx.db.item_desc().iter().collect::<Vec<_>>();
            if !item_desc.is_empty() {
                let _ = tmp_item_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: item_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let skill_desc = ctx.db.skill_desc().iter().collect::<Vec<_>>();
            if !skill_desc.is_empty() {
                let _ = tmp_skill_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: skill_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let building_desc = ctx.db.building_desc().iter().collect::<Vec<_>>();
            if !building_desc.is_empty() {
                let _ = tmp_building_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: building_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let claim_tech_desc = ctx.db.claim_tech_desc().iter().collect::<Vec<_>>();
            if !claim_tech_desc.is_empty() {
                let _ = tmp_claim_tech_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: claim_tech_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let crafting_recipe_desc = ctx.db.crafting_recipe_desc().iter().collect::<Vec<_>>();
            if !crafting_recipe_desc.is_empty() {
                let _ = tmp_crafting_recipe_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: crafting_recipe_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let item_list_desc = ctx.db.item_list_desc().iter().collect::<Vec<_>>();
            if !item_list_desc.is_empty() {
                let _ = tmp_item_list_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: item_list_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let traveler_task_desc = ctx.db.traveler_task_desc().iter().collect::<Vec<_>>();
            if !traveler_task_desc.is_empty() {
                let _ = tmp_traveler_task_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: traveler_task_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let npc_desc = ctx.db.npc_desc().iter().collect::<Vec<_>>();
            if !npc_desc.is_empty() {
                let _ = tmp_npc_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: npc_desc,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let inventory_state = ctx.db.inventory_state().iter().collect::<Vec<_>>();
            if !inventory_state.is_empty() {
                let _ = tmp_inventory_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: inventory_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let claim_member_state = ctx.db.claim_member_state().iter().collect::<Vec<_>>();
            if !claim_member_state.is_empty() {
                let _ = tmp_claim_member_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: claim_member_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let experience_state = ctx.db.experience_state().iter().collect::<Vec<_>>();
            if !experience_state.is_empty() {
                let _ = tmp_experience_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: experience_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let mobile_entity_state = ctx.db.mobile_entity_state().iter().collect::<Vec<_>>();
            if !mobile_entity_state.is_empty() {
                let _ = tmp_mobile_entity_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: mobile_entity_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let player_state = ctx.db.player_state().iter().collect::<Vec<_>>();
            if !player_state.is_empty() {
                let _ = tmp_player_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: player_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let vault_state = ctx.db.vault_state().iter().collect::<Vec<_>>();
            if !vault_state.is_empty() {
                let _ = tmp_vault_state_collectibles_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: vault_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let claim_tech_state = ctx.db.claim_tech_state().iter().collect::<Vec<_>>();
            if !claim_tech_state.is_empty() {
                let _ = tmp_claim_tech_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: claim_tech_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let building_state = ctx.db.building_state().iter().collect::<Vec<_>>();
            if !building_state.is_empty() {
                let _ = tmp_building_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: building_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let location_state = ctx.db.location_state().iter().collect::<Vec<_>>();
            if !location_state.is_empty() {
                let _ = tmp_location_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: location_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let building_nickname_state =
                ctx.db.building_nickname_state().iter().collect::<Vec<_>>();
            if !building_nickname_state.is_empty() {
                let _ = tmp_building_nickname_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: building_nickname_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let traveler_task_state = ctx.db.traveler_task_state().iter().collect::<Vec<_>>();
            if !traveler_task_state.is_empty() {
                let _ = tmp_traveler_task_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: traveler_task_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let trade_order_state = ctx.db.trade_order_state().iter().collect::<Vec<_>>();
            if !trade_order_state.is_empty() {
                let _ = tmp_trade_order_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: trade_order_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let user_state = ctx.db.user_state().iter().collect::<Vec<_>>();
            if !user_state.is_empty() {
                let _ = tmp_user_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: user_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let sell_order_state = ctx.db.sell_order_state().iter().collect::<Vec<_>>();
            if !sell_order_state.is_empty() {
                let _ = tmp_sell_order_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: sell_order_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let buy_order_state = ctx.db.buy_order_state().iter().collect::<Vec<_>>();
            if !buy_order_state.is_empty() {
                let _ = tmp_buy_order_state_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: buy_order_state,
                });
            }

            let tmp_database_name_arc = database_name_arc.clone();
            let collectible_desc = ctx.db.collectible_desc().iter().collect::<Vec<_>>();
            if !collectible_desc.is_empty() {
                let _ = tmp_collectible_desc_tx.send(SpacetimeUpdateMessages::Initial {
                    database_name: tmp_database_name_arc.clone(),
                    data: collectible_desc,
                });
            }

            tracing::debug!("Handled Subscription response");
        })
        .on_error(on_sub_error)
        .subscribe(
            tables_to_subscribe
                .into_iter()
                .filter_map(|table| {
                    if *remove_desc && table.contains("_desc") {
                        return None;
                    }

                    Some(format!("select * from {table}"))
                })
                .collect::<Vec<_>>(),
        );

    tokio::spawn(async move {
        let _ = ctx.run_async().await;
    });

    Ok(())
}

pub fn start_websocket_bitcraft_logic(config: Config, global_app_state: AppState) {
    tokio::spawn(async move {
        let (mobile_entity_state_tx, mobile_entity_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (user_state_tx, user_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (player_state_tx, player_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (player_username_state_tx, player_username_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (experience_state_tx, experience_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (inventory_state_tx, inventory_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (item_desc_tx, item_desc_rx) = tokio::sync::mpsc::unbounded_channel();
        let (item_list_desc_tx, item_list_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (cargo_desc_tx, cargo_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (vault_state_collectibles_tx, vault_state_collectibles_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (deployable_state_tx, deployable_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_state_tx, claim_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_local_state_tx, claim_local_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_member_state_tx, claim_member_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (skill_desc_tx, skill_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_tech_state_tx, claim_tech_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (claim_tech_desc_tx, claim_tech_desc_rx) = tokio::sync::mpsc::unbounded_channel();
        let (building_state_tx, building_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (building_desc_tx, building_desc_rx) = tokio::sync::mpsc::unbounded_channel();
        let (location_state_tx, location_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (building_nickname_state_tx, building_nickname_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (crafting_recipe_desc_tx, crafting_recipe_desc_desc_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (traveler_task_desc_tx, traveler_task_desc_rx) = tokio::sync::mpsc::unbounded_channel();
        let (traveler_task_state_tx, traveler_task_state_rx) =
            tokio::sync::mpsc::unbounded_channel();
        let (trade_order_state_tx, trade_order_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (buy_order_state_tx, buy_order_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (sell_order_state_tx, sell_order_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (npc_desc_tx, npc_desc_rx) = tokio::sync::mpsc::unbounded_channel();
        let (collectible_desc_tx, collectible_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut remove_desc = false;

        let tmp_conf = config.clone();
        let tmp_global_app_state = global_app_state.clone();
        let tmp_database = config.spacetimedb.database.clone();

        let result = connect_to_db_global(tmp_global_app_state, &tmp_conf, &tmp_database);

        if let Err(error) = result {
            tracing::error!(
                error = error.to_string(),
                "Error creating connection to {tmp_database} on {}",
                tmp_conf.spacetimedb_url()
            )
        };

        config
            .spacetimedb
            .databases
            .iter()
            .filter(|value| !value.trim().is_empty())
            .for_each(|database| {
                let tmp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
                let tmp_player_state_tx = player_state_tx.clone();
                let tmp_player_username_state_tx = player_username_state_tx.clone();
                let tmp_experience_state_tx = experience_state_tx.clone();
                let tmp_inventory_state_tx = inventory_state_tx.clone();
                let tmp_item_desc_tx = item_desc_tx.clone();
                let tmp_cargo_desc_tx = cargo_desc_tx.clone();
                let tmp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
                let tmp_deployable_state_tx = deployable_state_tx.clone();
                let tmp_claim_state_tx = claim_state_tx.clone();
                let tmp_claim_local_state_tx = claim_local_state_tx.clone();
                let tmp_claim_member_state_tx = claim_member_state_tx.clone();
                let tmp_skill_desc_tx = skill_desc_tx.clone();
                let tmp_claim_tech_state_tx = claim_tech_state_tx.clone();
                let tmp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
                let tmp_building_state_tx = building_state_tx.clone();
                let tmp_building_desc_tx = building_desc_tx.clone();
                let tmp_location_state_tx = location_state_tx.clone();
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
                let tmp_conf = config.clone();
                let tmp_global_app_state = global_app_state.clone();
                let tmp_remove_desc = remove_desc;
                let tmp_database = database.clone();

                tokio::spawn(async move {
                    metrics::gauge!(
                        "bitcraft_database_connected",
                        &[("region", tmp_database.clone())]
                    )
                    .set(0);

                    let result = connect_to_db_logic(
                        tmp_global_app_state,
                        &tmp_conf,
                        &tmp_database,
                        &tmp_remove_desc,
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
                        &tmp_location_state_tx,
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
                    );

                    if let Err(error) = result {
                        tracing::error!(
                            error = error.to_string(),
                            "Error creating connection to {tmp_database} on {}",
                            tmp_conf.spacetimedb_url()
                        )
                    };
                });

                remove_desc = true;
            });

        start_worker_mobile_entity_state(global_app_state.clone(), mobile_entity_state_rx);
        start_worker_player_state(
            global_app_state.clone(),
            player_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_player_username_state(
            global_app_state.clone(),
            player_username_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_experience_state(
            global_app_state.clone(),
            experience_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_inventory_state(
            global_app_state.clone(),
            inventory_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_vault_state_collectibles(
            global_app_state.clone(),
            vault_state_collectibles_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_item_desc(
            global_app_state.clone(),
            item_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_cargo_desc(
            global_app_state.clone(),
            cargo_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_deployable_state(
            global_app_state.clone(),
            deployable_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_claim_state(
            global_app_state.clone(),
            claim_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_claim_local_state(
            global_app_state.clone(),
            claim_local_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_claim_member_state(
            global_app_state.clone(),
            claim_member_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_skill_desc(
            global_app_state.clone(),
            skill_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_claim_tech_state(
            global_app_state.clone(),
            claim_tech_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_claim_tech_desc(
            global_app_state.clone(),
            claim_tech_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_building_state(
            global_app_state.clone(),
            building_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_building_desc(
            global_app_state.clone(),
            building_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_location_state(
            global_app_state.clone(),
            location_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_building_nickname_state(
            global_app_state.clone(),
            building_nickname_state_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_crafting_recipe_desc(
            global_app_state.clone(),
            crafting_recipe_desc_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_item_list_desc(
            global_app_state.clone(),
            item_list_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_traveler_task_desc(
            global_app_state.clone(),
            traveler_task_desc_rx,
            3000,
            Duration::from_millis(50),
        );
        start_worker_traveler_task_state(
            global_app_state.clone(),
            traveler_task_state_rx,
            6000,
            Duration::from_millis(50),
        );
        start_worker_trade_order_state(
            global_app_state.clone(),
            trade_order_state_rx,
            6000,
            Duration::from_millis(50),
        );

        start_worker_npc_desc(
            global_app_state.clone(),
            npc_desc_rx,
            3000,
            Duration::from_millis(50),
        );

        start_worker_buy_order_state(
            global_app_state.clone(),
            buy_order_state_rx,
            3000,
            Duration::from_millis(50),
        );

        start_worker_sell_order_state(
            global_app_state.clone(),
            sell_order_state_rx,
            3000,
            Duration::from_millis(50),
        );

        start_worker_collectible_desc(
            global_app_state.clone(),
            collectible_desc_rx,
            3000,
            Duration::from_millis(50),
        );

        start_worker_user_state(global_app_state.clone(), user_state_rx);
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
        database_name: Arc<String>,
    },
    Insert {
        event: __sdk::Event<Reducer>,
        new: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
    Update {
        event: __sdk::Event<Reducer>,
        old: T,
        new: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
    Remove {
        event: __sdk::Event<Reducer>,
        delete: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
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
}

impl WebSocketMessages {
    pub fn topics(&self) -> Option<Vec<(String, Option<i64>)>> {
        match self {
            WebSocketMessages::Experience {
                skill_name,
                user_id,
                ..
            } => Some(vec![
                (format!("experience:{skill_name}"), Some(*user_id)),
                ("experience".to_string(), Some(*user_id)),
            ]),
            WebSocketMessages::ClaimLocalState(claim_local_state) => Some(vec![(
                "claim_local_state".to_string(),
                Some(claim_local_state.entity_id),
            )]),
            WebSocketMessages::Level {
                user_id,
                skill_name,
                ..
            } => Some(vec![
                (format!("level:{skill_name}"), Some(*user_id)),
                ("level".to_string(), Some(*user_id)),
            ]),
            WebSocketMessages::PlayerMovedIntoClaim { user_id, .. } => Some(vec![(
                "player_moved_into_claim".to_string(),
                Some(*user_id),
            )]),
            WebSocketMessages::PlayerMovedOutOfClaim { user_id, .. } => Some(vec![(
                "player_moved_out_of_claim".to_string(),
                Some(*user_id),
            )]),
            WebSocketMessages::MovedOutOfClaim { claim_id, .. } => Some(vec![(
                "moved_out_of_claim".to_string(),
                Some(*claim_id as i64),
            )]),
            WebSocketMessages::MovedIntoClaim { claim_id, .. } => Some(vec![(
                "moved_into_claim".to_string(),
                Some(*claim_id as i64),
            )]),
            WebSocketMessages::PlayerState(player) => {
                Some(vec![("player_state".to_string(), Some(player.entity_id))])
            }
            WebSocketMessages::MobileEntityState(mobile_entity_state) => Some(vec![(
                "mobile_entity_state".to_string(),
                Some(mobile_entity_state.entity_id as i64),
            )]),
            // WebSocketMessages::ClaimDescriptionState(claim) => {
            //     Some(vec![("claim".to_string(), claim.entity_id)])
            // }
            WebSocketMessages::TotalExperience { user_id, .. } => {
                Some(vec![("total_experience".to_string(), Some(*user_id))])
            }
            WebSocketMessages::PlayerActionState(player_action_state) => Some(vec![(
                "player_action_state".to_string(),
                Some(player_action_state.entity_id as i64),
            )]),
            WebSocketMessages::PlayerActionStateChangeName(_, id) => Some(vec![(
                "player_action_state_change_name".to_string(),
                Some(*id as i64),
            )]),
            WebSocketMessages::ActionState(action_state) => Some(vec![(
                "action_state".to_string(),
                Some(action_state.owner_entity_id as i64),
            )]),
            WebSocketMessages::TravelerTaskState(traveler_task_state) => Some(vec![
                (
                    "traveler_task_state".to_string(),
                    Some(traveler_task_state.entity_id),
                ),
                (
                    "traveler_task_state:player".to_string(),
                    Some(traveler_task_state.player_entity_id),
                ),
            ]),
            WebSocketMessages::TravelerTaskStateDelete(traveler_task_state) => Some(vec![
                (
                    "traveler_task_state".to_string(),
                    Some(traveler_task_state.entity_id),
                ),
                (
                    "traveler_task_state:player".to_string(),
                    Some(traveler_task_state.player_entity_id),
                ),
            ]),
            WebSocketMessages::ListSubscribedTopics => None,
            WebSocketMessages::Subscribe { .. } => None,
            WebSocketMessages::SubscribedTopics(_) => None,
            WebSocketMessages::Unsubscribe { .. } => None,
            WebSocketMessages::Message(_) => None,
            WebSocketMessages::InsertSellOrder(auction_listing_state) => Some(vec![
                (
                    "insert_sell_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "insert_sell_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("insert_sell_order".to_string(), None),
            ]),
            WebSocketMessages::UpdateSellOrder(auction_listing_state) => Some(vec![
                (
                    "update_sell_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "update_sell_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("update_sell_order".to_string(), None),
            ]),
            WebSocketMessages::RemoveSellOrder(auction_listing_state) => Some(vec![
                (
                    "remove_sell_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "remove_sell_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("remove_sell_order".to_string(), None),
            ]),
            WebSocketMessages::InsertBuyOrder(auction_listing_state) => Some(vec![
                (
                    "update_buy_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "update_buy_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("update_buy_order".to_string(), None),
            ]),
            WebSocketMessages::UpdateBuyOrder(auction_listing_state) => Some(vec![
                (
                    "update_buy_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "update_buy_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("update_buy_order".to_string(), None),
            ]),
            WebSocketMessages::RemoveBuyOrder(auction_listing_state) => Some(vec![
                (
                    "remove_buy_order".to_string(),
                    Some(auction_listing_state.entity_id as i64),
                ),
                (
                    "remove_buy_order:item_id".to_string(),
                    Some(auction_listing_state.item_id as i64),
                ),
                ("remove_buy_order".to_string(), None),
            ]),
        }
    }
}
