use crate::AppState;
use crate::config::Config;
use crate::leaderboard::experience_to_level;
use chrono::{DateTime};
use entity::inventory_changelog::TypeOfChange;
use entity::{
    cargo_desc, claim_local_state, claim_member_state, claim_state, claim_tech_state,
    crafting_recipe, deployable_state, item_desc, item_list_desc, mobile_entity_state,
    vault_state_collectibles,
};
#[allow(unused_imports)]
use entity::{raw_event_data, skill_desc};
use game_module::module_bindings::*;
use kanal::{AsyncReceiver, Sender};
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait, TryIntoModel, sea_query, Set,NotSet};
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{
    Compression, DbContext, Error, Event, Identity, Table, TableWithPrimaryKey, Timestamp,
    credentials,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Instant;
use tokio::time::{Duration, sleep};
use ts_rs::TS;

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

macro_rules! setup_spacetime_db_listeners {
    ($ctx:expr, $db_table_method:ident, $tx_channel:ident, $state_type:ty, $database_name_expr:expr) => {
        let table_name_str = stringify!($db_table_method);
        let database_name_runtime_string = $database_name_expr.to_string();
        let database_name_arc: Arc<String> = Arc::new($database_name_expr.to_string());

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

fn connect_to_db_logic(
    config: &Config,
    database: &str,
    remove_desc: &bool,
    mobile_entity_state_tx: &Sender<SpacetimeUpdateMessages<MobileEntityState>>,
    player_state_tx: &Sender<SpacetimeUpdateMessages<PlayerState>>,
    player_username_state_tx: &Sender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    experience_state_tx: &Sender<SpacetimeUpdateMessages<ExperienceState>>,
    inventory_state_tx: &Sender<SpacetimeUpdateMessages<InventoryState>>,
    item_desc_tx: &Sender<SpacetimeUpdateMessages<ItemDesc>>,
    cargo_desc_tx: &Sender<SpacetimeUpdateMessages<CargoDesc>>,
    vault_state_collectibles_tx: &Sender<SpacetimeUpdateMessages<VaultState>>,
    deployable_state_tx: &Sender<SpacetimeUpdateMessages<DeployableState>>,
    claim_state_tx: &Sender<SpacetimeUpdateMessages<ClaimState>>,
    claim_local_state_tx: &Sender<SpacetimeUpdateMessages<ClaimLocalState>>,
    claim_member_state_tx: &Sender<SpacetimeUpdateMessages<ClaimMemberState>>,
    skill_desc_tx: &Sender<SpacetimeUpdateMessages<SkillDesc>>,
    claim_tech_state_tx: &Sender<SpacetimeUpdateMessages<ClaimTechState>>,
    claim_tech_desc_tx: &Sender<SpacetimeUpdateMessages<ClaimTechDesc>>,
    building_state_tx: &Sender<SpacetimeUpdateMessages<BuildingState>>,
    building_desc_tx: &Sender<SpacetimeUpdateMessages<BuildingDesc>>,
    location_state_tx: &Sender<SpacetimeUpdateMessages<LocationState>>,
    building_nickname_state_tx: &Sender<SpacetimeUpdateMessages<BuildingNicknameState>>,
    crafting_recipe_desc_tx: &Sender<SpacetimeUpdateMessages<CraftingRecipeDesc>>,
    item_list_desc_tx: &Sender<SpacetimeUpdateMessages<ItemListDesc>>,
    user_state_tx: &Sender<SpacetimeUpdateMessages<UserState>>,
) {
    let ctx = connect_to_db(database, config.spacetimedb_url().as_ref());

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

    setup_spacetime_db_listeners!(ctx, user_state, user_state_tx, UserState, database);

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
        // "collectible_desc",
        "claim_tech_desc",
        // "claim_description_state", -> claim_state
        // "location_state where dimension = 1", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN player_state ps ON player_state.entity_id = location_state.entity_id", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN building_state ps ON building_state.entity_id = building_state.entity_id", // This currently takes to much cpu to run
        // "select location_state.* from location_state JOIN deployable_state ps ON deployable_state.entity_id = deployable_state.entity_id", // This currently takes to much cpu to run
        "inventory_state",
    ];

    ctx.subscription_builder()
        .on_applied(move |_ctx: &SubscriptionEventContext| {})
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
}

pub fn start_websocket_bitcraft_logic(config: Config, global_app_state: Arc<AppState>) {
    tokio::spawn(async move {
        let (mobile_entity_state_tx, mobile_entity_state_rx) = kanal::unbounded_async();

        let (user_state_tx, user_state_rx) = kanal::unbounded_async();

        let (player_state_tx, player_state_rx) = kanal::unbounded_async();

        let (player_username_state_tx, player_username_state_rx) = kanal::unbounded_async();

        let (experience_state_tx, experience_state_rx) = kanal::unbounded_async();

        let (inventory_state_tx, inventory_state_rx) = kanal::unbounded_async();

        let (item_desc_tx, item_desc_rx) = kanal::unbounded_async();
        let (item_list_desc_tx, item_list_desc_rx) = kanal::unbounded_async();

        let (cargo_desc_tx, cargo_desc_rx) = kanal::unbounded_async();

        let (vault_state_collectibles_tx, vault_state_collectibles_rx) = kanal::unbounded_async();

        let (deployable_state_tx, deployable_state_rx) = kanal::unbounded_async();

        let (claim_state_tx, claim_state_rx) = kanal::unbounded_async();

        let (claim_local_state_tx, claim_local_state_rx) = kanal::unbounded_async();

        let (claim_member_state_tx, claim_member_state_rx) = kanal::unbounded_async();

        let (skill_desc_tx, skill_desc_rx) = kanal::unbounded_async();

        let (claim_tech_state_tx, claim_tech_state_rx) = kanal::unbounded_async();
        let (claim_tech_desc_tx, claim_tech_desc_rx) = kanal::unbounded_async();
        let (building_state_tx, building_state_rx) = kanal::unbounded_async();
        let (building_desc_tx, building_desc_rx) = kanal::unbounded_async();
        let (location_state_tx, location_state_rx) = kanal::unbounded_async();
        let (building_nickname_state_tx, building_nickname_state_rx) = kanal::unbounded_async();

        let (crafting_recipe_desc_tx, crafting_recipe_desc_desc_rx) = kanal::unbounded_async();

        let mut remove_desc = false;

        config.spacetimedb.databases.iter().for_each(|database| {
            connect_to_db_logic(
                &config,
                database,
                &remove_desc,
                &mobile_entity_state_tx.clone_sync(),
                &player_state_tx.clone_sync(),
                &player_username_state_tx.clone_sync(),
                &experience_state_tx.clone_sync(),
                &inventory_state_tx.clone_sync(),
                &item_desc_tx.clone_sync(),
                &cargo_desc_tx.clone_sync(),
                &vault_state_collectibles_tx.clone_sync(),
                &deployable_state_tx.clone_sync(),
                &claim_state_tx.clone_sync(),
                &claim_local_state_tx.clone_sync(),
                &claim_member_state_tx.clone_sync(),
                &skill_desc_tx.clone_sync(),
                &claim_tech_state_tx.clone_sync(),
                &claim_tech_desc_tx.clone_sync(),
                &building_state_tx.clone_sync(),
                &building_desc_tx.clone_sync(),
                &location_state_tx.clone_sync(),
                &building_nickname_state_tx.clone_sync(),
                &crafting_recipe_desc_tx.clone_sync(),
                &item_list_desc_tx.clone_sync(),
                &user_state_tx.clone_sync(),
            );

            remove_desc = true;
        });
        start_worker_mobile_entity_state(global_app_state.clone(), mobile_entity_state_rx);
        start_worker_player_state(
            global_app_state.clone(),
            player_state_rx,
            1000,
            Duration::from_millis(50),
        );
        start_worker_player_username_state(
            global_app_state.clone(),
            player_username_state_rx,
            1000,
            Duration::from_millis(50),
        );
        start_worker_experience_state(
            global_app_state.clone(),
            experience_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_inventory_state(
            global_app_state.clone(),
            inventory_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_vault_state_collectibles(
            global_app_state.clone(),
            vault_state_collectibles_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_item_desc(
            global_app_state.clone(),
            item_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_cargo_desc(
            global_app_state.clone(),
            cargo_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_deployable_state(
            global_app_state.clone(),
            deployable_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_claim_state(
            global_app_state.clone(),
            claim_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_claim_local_state(
            global_app_state.clone(),
            claim_local_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_claim_member_state(
            global_app_state.clone(),
            claim_member_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_skill_desc(
            global_app_state.clone(),
            skill_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_claim_tech_state(
            global_app_state.clone(),
            claim_tech_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_claim_tech_desc(
            global_app_state.clone(),
            claim_tech_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_building_state(
            global_app_state.clone(),
            building_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_building_desc(
            global_app_state.clone(),
            building_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_location_state(
            global_app_state.clone(),
            location_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_building_nickname_state(
            global_app_state.clone(),
            building_nickname_state_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_crafting_recipe_desc(
            global_app_state.clone(),
            crafting_recipe_desc_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_item_list_desc(
            global_app_state.clone(),
            item_list_desc_rx,
            2000,
            Duration::from_millis(50),
        );
        start_worker_user_state(global_app_state.clone(), user_state_rx)
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
enum SpacetimeUpdateMessages<T> {
    Insert {
        new: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
    Update {
        old: T,
        new: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
    Remove {
        delete: T,
        database_name: Arc<String>,
        caller_identity: Option<Identity>,
        reducer: Option<Reducer>,
        timestamp: Option<Timestamp>,
    },
}

fn start_worker_mobile_entity_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Insert { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                SpacetimeUpdateMessages::Update { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
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

fn start_worker_user_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<UserState>>,
) {
    tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Insert { new, .. } => {
                    global_app_state
                        .user_state
                        .insert(new.identity, new.entity_id);
                }
                SpacetimeUpdateMessages::Update { new, .. } => {
                    global_app_state
                        .user_state
                        .insert(new.identity, new.entity_id);
                }
                SpacetimeUpdateMessages::Remove { delete, .. } => {
                    global_app_state.user_state.remove(&delete.identity);
                }
            }
        }
    });
}

fn start_worker_item_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ItemDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(item_desc::Column::Id)
            .update_columns([
                item_desc::Column::Name,
                item_desc::Column::Description,
                item_desc::Column::Volume,
                item_desc::Column::Durability,
                item_desc::Column::ConvertToOnDurabilityZero,
                item_desc::Column::SecondaryKnowledgeId,
                item_desc::Column::ModelAssetName,
                item_desc::Column::IconAssetName,
                item_desc::Column::Tier,
                item_desc::Column::Tag,
                item_desc::Column::Rarity,
                item_desc::Column::CompendiumEntry,
                item_desc::Column::ItemListId,
            ])
            .to_owned();

        let mut currently_known_item_desc = ::entity::item_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                if currently_known_item_desc.contains_key(&model.id) {
                                    let value = currently_known_item_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                if currently_known_item_desc.contains_key(&model.id) {
                                    let value = currently_known_item_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::item_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.item_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ItemDesc = id, error = error.to_string(), "Could not delete ItemDesc");
                                }

                                tracing::debug!("ItemDesc::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::item_desc::Entity::insert_many(messages.clone())
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

fn start_worker_cargo_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<CargoDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
            .update_columns([
                cargo_desc::Column::Name,
                cargo_desc::Column::Description,
                cargo_desc::Column::Volume,
                cargo_desc::Column::SecondaryKnowledgeId,
                cargo_desc::Column::ModelAssetName,
                cargo_desc::Column::IconAssetName,
                cargo_desc::Column::CarriedModelAssetName,
                cargo_desc::Column::PickUpAnimationStart,
                cargo_desc::Column::PickUpAnimationEnd,
                cargo_desc::Column::DropAnimationStart,
                cargo_desc::Column::DropAnimationEnd,
                cargo_desc::Column::PickUpTime,
                cargo_desc::Column::PlaceTime,
                cargo_desc::Column::AnimatorState,
                cargo_desc::Column::MovementModifier,
                cargo_desc::Column::BlocksPath,
                cargo_desc::Column::OnDestroyYieldCargos,
                cargo_desc::Column::DespawnTime,
                cargo_desc::Column::Tier,
                cargo_desc::Column::Tag,
                cargo_desc::Column::Rarity,
                cargo_desc::Column::NotPickupable,
            ])
            .to_owned();

        let mut currently_known_cargo_desc = ::entity::cargo_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                if currently_known_cargo_desc.contains_key(&model.id) {
                                    let value = currently_known_cargo_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_cargo_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                if currently_known_cargo_desc.contains_key(&model.id) {
                                    let value = currently_known_cargo_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_cargo_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::cargo_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.cargo_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CargoDesc = id, error = error.to_string(), "Could not delete CargoDesc");
                                }

                                tracing::debug!("CargoDesc::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::cargo_desc::Entity::insert_many(messages.clone())
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

fn start_worker_player_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<PlayerState>>,
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

        let mut currently_known_player_state = ::entity::player_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let mut ids = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::player_state::Model = new.into();

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).increment(1);

                                ids.push(model.entity_id);
                                if currently_known_player_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, old, .. } => {
                                let model: ::entity::player_state::Model = new.into();


                                if model.signed_in != old.signed_in {
                                    metrics::gauge!("players_current_state", &[
                                        ("online", model.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).increment(1);
                                    metrics::gauge!("players_current_state", &[
                                        ("online", old.signed_in.to_string()),
                                        ("region", database_name.to_string())
                                    ]).decrement(1);
                                }

                                ids.push(model.entity_id);
                                if currently_known_player_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::player_state::Model = delete.into();
                                let id = model.entity_id;

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                metrics::gauge!("players_current_state", &[
                                    ("online", model.signed_in.to_string()),
                                    ("region", database_name.to_string())
                                ]).decrement(1);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerState = id, error = error.to_string(), "Could not delete PlayerState");
                                }

                                tracing::debug!("PlayerState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
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
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
                .update_columns([::entity::player_username_state::Column::Username])
                .to_owned();

        let mut currently_known_player_username_state =
            ::entity::player_username_state::Entity::find()
                .all(&global_app_state.conn)
                .await
                .map_or(vec![], |aa| aa)
                .into_iter()
                .map(|value| (value.entity_id, value))
                .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let mut ids = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();

                                ids.push(model.entity_id);

                                if currently_known_player_username_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_username_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_username_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();
                                ids.push(model.entity_id);
                                if currently_known_player_username_state.contains_key(&model.entity_id) {
                                    let value = currently_known_player_username_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_player_username_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_username_state::Model = delete.into();
                                let id = model.entity_id;

                                if ids.contains(&id) {
                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerUsernameState = id, error = error.to_string(), "Could not delete PlayerUsernameState");
                                }

                                tracing::debug!("PlayerUsernameState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::player_username_state::Entity::insert_many(messages.clone())
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
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ExperienceState>>,
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

        let mut currently_known_experience_state = ::entity::experience_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let id = new.entity_id;
                                new.experience_stacks.iter().for_each(|es| {
                                    let model = ::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    };

                                    if currently_known_experience_state.contains_key(&model.entity_id) {
                                        let value = currently_known_experience_state.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_experience_state.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, old, .. } => {
                                let id = new.entity_id;

                                let mut new_level_vec = vec![];

                                new.experience_stacks.iter().for_each(|es| {
                                    new_level_vec.push((
                                        es.clone(),
                                        experience_to_level(es.quantity as i64),
                                    ));

                                    let model = ::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    };

                                    if currently_known_experience_state.contains_key(&model.entity_id) {
                                        let value = currently_known_experience_state.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_experience_state.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                });
                                old.experience_stacks.iter().for_each(|es| {
                                    let old_level =
                                        experience_to_level(es.quantity as i64);

                                    let new_level = new_level_vec.iter().find(|new_level| new_level.0.skill_id.eq(&es.skill_id));
                                    let skill_name = global_app_state.skill_desc.get(&(es.skill_id as i64));

                                    if let Some(skill_name) = skill_name {
                                        if let Some(new_level) = new_level {
                                            if old_level != new_level.1 {

                                                    global_app_state.tx.send(WebSocketMessages::Level {
                                                        level: new_level.1 as u64,
                                                        skill_name: skill_name.to_owned().name,
                                                        user_id: id as i64,
                                                    })
                                                    .expect("TODO: panic message");
                                                }

                                            if new_level.0.quantity > es.quantity {
                                                global_app_state.tx.send(WebSocketMessages::Experience {
                                                    level: new_level.1 as u64,
                                                    experience: new_level.0.quantity as u64,
                                                    rank: 0,
                                                    skill_name: skill_name.to_owned().name,
                                                    user_id: id as i64,
                                                })
                                                .expect("TODO: panic message");
                                            }
                                        }
                                    }
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let id = delete.entity_id as i64;
                                let vec_es = delete.experience_stacks.iter().map(|es| {
                                    if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                        messages.remove(index);
                                    }

                                    ::entity::experience_state::Model {
                                        entity_id: id,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    }
                                }).collect::<Vec<_>>();

                                for es in vec_es {
                                    if let Err(error) = es.delete(&global_app_state.conn).await {
                                        tracing::error!(ExperienceState = id, error = error.to_string(), "Could not delete ExperienceState");
                                    }
                                }
                                tracing::debug!("ExperienceState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::experience_state::Entity::insert_many(messages.clone())
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

fn start_worker_inventory_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<InventoryState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::inventory::Column::EntityId)
            .update_columns([
                ::entity::inventory::Column::Pockets,
                ::entity::inventory::Column::InventoryIndex,
                ::entity::inventory::Column::CargoIndex,
                ::entity::inventory::Column::OwnerEntityId,
                ::entity::inventory::Column::PlayerOwnerEntityId,
            ])
            .to_owned();
        let on_conflict_changelog =
            sea_query::OnConflict::column(::entity::inventory_changelog::Column::Id)
                .update_columns([
                    ::entity::inventory_changelog::Column::EntityId,
                    ::entity::inventory_changelog::Column::UserId,
                    ::entity::inventory_changelog::Column::PocketNumber,
                    ::entity::inventory_changelog::Column::OldItemId,
                    ::entity::inventory_changelog::Column::OldItemType,
                    ::entity::inventory_changelog::Column::OldItemQuantity,
                    ::entity::inventory_changelog::Column::NewItemId,
                    ::entity::inventory_changelog::Column::NewItemType,
                    ::entity::inventory_changelog::Column::NewItemQuantity,
                    ::entity::inventory_changelog::Column::TypeOfChange,
                    ::entity::inventory_changelog::Column::Timestamp,
                ])
                .to_owned();
        let mut currently_known_inventory = ::entity::inventory::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let mut messages_changed = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::inventory::Model = new.into();

                                global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                if currently_known_inventory.contains_key(&model.entity_id) {
                                    let value = currently_known_inventory.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_inventory.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, old, caller_identity, timestamp, .. } => {
                                let new_model = new.clone();
                                let model: ::entity::inventory::Model = new.into();
                                global_app_state.inventory_state.insert(model.entity_id, model.clone());
                                if currently_known_inventory.contains_key(&model.entity_id) {
                                    let value = currently_known_inventory.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_inventory.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }


                                if let Some(caller_identity) = caller_identity {
                                    let mut user_id = None;
                                        if let Some(entity_id) = global_app_state.user_state.get(&caller_identity) {
                                         user_id = Some(entity_id.clone() as i64)
                                    };
                                    for (pocket_index, new_pocket) in new_model.pockets.iter().enumerate() {
                                        let old_pocket = &old.pockets[pocket_index];

                                        let mut new_item_id = None;
                                        let mut new_item_type = None;
                                        let mut new_item_quantity = None;
                                        if let Some(old_contents) = new_pocket.contents.clone() {
                                            new_item_id = Some(old_contents.item_id);
                                            new_item_type = Some(old_contents.item_type.into());
                                            new_item_quantity = Some(old_contents.quantity);
                                        }


                                        let mut old_item_id = None;
                                        let mut old_item_type = None;
                                        let mut old_item_quantity = None;
                                        if let Some(old_contents) = old_pocket.contents.clone() {
                                            old_item_id = Some(old_contents.item_id);
                                            old_item_type = Some(old_contents.item_type.into());
                                            old_item_quantity = Some(old_contents.quantity);
                                        }

                                        if new_item_id == old_item_id  && new_item_type == old_item_type && new_item_quantity == old_item_quantity {
                                            continue
                                        }
                                        let type_of_change: TypeOfChange;
                                        if new_item_id == None && old_item_id != None {
                                            type_of_change = TypeOfChange::Remove;
                                        }else if new_item_id != None && old_item_id == None {
                                            type_of_change = TypeOfChange::Add;
                                        }else {
                                            if old_item_id != new_item_id {
                                                type_of_change = TypeOfChange::AddAndRemove;
                                            }else {
                                                type_of_change = TypeOfChange::Update;
                                            }
                                        }
                                        messages_changed.push(::entity::inventory_changelog::ActiveModel {
                                            id: NotSet,
                                            entity_id: Set(new_model.entity_id as i64),
                                            user_id: Set(user_id),
                                            pocket_number: Set(pocket_index as i32),
                                            old_item_id: Set(old_item_id),
                                            old_item_type: Set(old_item_type),
                                            old_item_quantity: Set(old_item_quantity),
                                            new_item_id: Set(new_item_id),
                                            new_item_type: Set(new_item_type),
                                            new_item_quantity: Set(new_item_quantity),
                                            type_of_change: Set(type_of_change),
                                            timestamp: Set(DateTime::from_timestamp_micros(timestamp.unwrap().to_micros_since_unix_epoch()).unwrap())
                                        })
                                    }
                                }

                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::inventory::Model = delete.into();
                                let id = model.entity_id;

                                global_app_state.inventory_state.remove(&model.entity_id);
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(InventoryState = id, error = error.to_string(), "Could not delete InventoryState");
                                }

                                tracing::debug!("InventoryState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::inventory::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;
                // Your batch processing logic here
            }

            if !messages_changed.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ =
                    ::entity::inventory_changelog::Entity::insert_many(messages_changed.clone())
                        .on_conflict(on_conflict_changelog.clone())
                        .exec(&global_app_state.conn)
                        .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_changed.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_deployable_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<DeployableState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(deployable_state::Column::EntityId)
            .update_columns([
                deployable_state::Column::OwnerId,
                deployable_state::Column::ClaimEntityId,
                deployable_state::Column::Direction,
                deployable_state::Column::DeployableDescriptionId,
                deployable_state::Column::Nickname,
                deployable_state::Column::Hidden,
            ])
            .to_owned();

        let mut currently_known_deployable_state = ::entity::deployable_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::deployable_state::Model = new.into();

                                if currently_known_deployable_state.contains_key(&model.entity_id) {
                                    let value = currently_known_deployable_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_deployable_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::deployable_state::Model = new.into();
                                if currently_known_deployable_state.contains_key(&model.entity_id) {
                                    let value = currently_known_deployable_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_deployable_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::deployable_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(DeployableState = id, error = error.to_string(), "Could not delete DeployableState");
                                }

                                tracing::debug!("DeployableState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::deployable_state::Entity::insert_many(messages.clone())
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

fn start_worker_claim_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_state::Column::EntityId)
            .update_columns([
                claim_state::Column::OwnerPlayerEntityId,
                claim_state::Column::OwnerBuildingEntityId,
                claim_state::Column::Name,
                claim_state::Column::Neutral,
            ])
            .to_owned();

        let mut currently_known_claim_state = ::entity::claim_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::claim_state::Model = new.into();

                                if currently_known_claim_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_state::Model = new.into();
                                if currently_known_claim_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimState = id, error = error.to_string(), "Could not delete ClaimState");
                                }

                                tracing::debug!("ClaimState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_state::Entity::insert_many(messages.clone())
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

fn start_worker_claim_local_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimLocalState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_local_state::Column::EntityId)
            .update_columns([
                claim_local_state::Column::Supplies,
                claim_local_state::Column::BuildingMaintenance,
                claim_local_state::Column::NumTiles,
                claim_local_state::Column::NumTileNeighbors,
                claim_local_state::Column::Location,
                claim_local_state::Column::Treasury,
                claim_local_state::Column::XpGainedSinceLastCoinMinting,
                claim_local_state::Column::SuppliesPurchaseThreshold,
                claim_local_state::Column::SuppliesPurchasePrice,
                claim_local_state::Column::BuildingDescriptionId,
            ])
            .to_owned();

        let mut currently_known_claim_local_state = ::entity::claim_local_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let org_id = new.entity_id;
                                let model: ::entity::claim_local_state::Model = new.into();
                                global_app_state.claim_local_state.insert(org_id, model.clone());

                                if currently_known_claim_local_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_local_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.clone().into_active_model());
                                    } else {
                                        currently_known_claim_local_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.clone().into_active_model());
                                }

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let org_id = new.entity_id;
                                let model: ::entity::claim_local_state::Model = new.into();

                                global_app_state.claim_local_state.insert(org_id, model.clone());

                                if currently_known_claim_local_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_local_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.clone().into_active_model());
                                    } else {
                                        currently_known_claim_local_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.clone().into_active_model());
                                }

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_local_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                global_app_state.claim_local_state.remove(&(model.entity_id as u64));

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimLocalState = id, error = error.to_string(), "Could not delete ClaimLocalState");
                                }

                                tracing::debug!("ClaimLocalState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_local_state::Entity::insert_many(
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

fn start_worker_claim_member_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimMemberState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_member_state::Column::EntityId)
            .update_columns([
                claim_member_state::Column::ClaimEntityId,
                claim_member_state::Column::PlayerEntityId,
                claim_member_state::Column::UserName,
                claim_member_state::Column::InventoryPermission,
                claim_member_state::Column::BuildPermission,
                claim_member_state::Column::OfficerPermission,
                claim_member_state::Column::CoOwnerPermission,
            ])
            .to_owned();

        let mut currently_known_claim_member_state = ::entity::claim_member_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_member_state::Model = new.into();

                                global_app_state
                                    .add_claim_member(model.clone());

                                if currently_known_claim_member_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_member_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_member_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_member_state::Model = new.into();

                                global_app_state
                                    .add_claim_member(model.clone());

                                if currently_known_claim_member_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_member_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_member_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_member_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }


                                global_app_state.remove_claim_member(model.clone());

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimMemberState = id, error = error.to_string(), "Could not delete ClaimMemberState");
                                }

                                tracing::debug!("ClaimMemberState::Remove");
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
                tracing::debug!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_member_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| {
                            global_app_state
                                .add_claim_member(value.clone().try_into_model().unwrap());
                            value.clone().into_active_model()
                        })
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

fn start_worker_skill_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<SkillDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
            .update_columns([
                skill_desc::Column::Name,
                skill_desc::Column::Description,
                skill_desc::Column::IconAssetName,
                skill_desc::Column::Title,
                skill_desc::Column::SkillCategory,
                skill_desc::Column::Skill,
            ])
            .to_owned();

        let mut currently_known_skill_desc = ::entity::skill_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::skill_desc::Model = new.into();

                                global_app_state.skill_desc.insert(model.id, model.clone());
                                if currently_known_skill_desc.contains_key(&model.id) {
                                    let value = currently_known_skill_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_skill_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::skill_desc::Model = new.into();
                                global_app_state.skill_desc.insert(model.id, model.clone());
                                if currently_known_skill_desc.contains_key(&model.id) {
                                    let value = currently_known_skill_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_skill_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::skill_desc::Model = delete.into();
                                let id = model.id;

                                global_app_state.skill_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(SkillDesc = id, error = error.to_string(), "Could not delete SkillDesc");
                                }

                                tracing::debug!("SkillDesc::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::skill_desc::Entity::insert_many(messages.clone())
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

fn start_worker_vault_state_collectibles(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<VaultState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            vault_state_collectibles::Column::EntityId,
            vault_state_collectibles::Column::Id,
        ])
        .update_columns([
            vault_state_collectibles::Column::Activated,
            vault_state_collectibles::Column::Count,
        ])
        .to_owned();

        let mut currently_known_vault_state_collectibles =
            ::entity::vault_state_collectibles::Entity::find()
                .all(&global_app_state.conn)
                .await
                .map_or(vec![], |aa| aa)
                .into_iter()
                .map(|value| (value.entity_id, value))
                .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles();

                                for model in models {
                                    if currently_known_vault_state_collectibles.contains_key(&model.entity_id) {
                                        let value = currently_known_vault_state_collectibles.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_vault_state_collectibles.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles();
                                for model in models {
                                    if currently_known_vault_state_collectibles.contains_key(&model.entity_id) {
                                        let value = currently_known_vault_state_collectibles.get(&model.entity_id).unwrap();

                                        if &model != value {
                                            messages.push(model.into_active_model());
                                        } else {
                                            currently_known_vault_state_collectibles.remove(&model.entity_id);
                                        }
                                    } else {
                                        messages.push(model.into_active_model());
                                    }
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = delete.into();
                                let models= raw_model.to_model_collectibles();
                                for model in models {

                                    let id = model.entity_id;

                                    if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }

                                    if let Err(error) = model.delete(&global_app_state.conn).await {
                                        tracing::error!(VaultState = id, error = error.to_string(), "Could not delete VaultState");
                                    }
                                }

                                tracing::debug!("VaultState::Remove");
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::vault_state_collectibles::Entity::insert_many(messages.clone())
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

fn start_worker_claim_tech_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimTechState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([claim_tech_state::Column::EntityId])
            .update_columns([
                claim_tech_state::Column::Learned,
                claim_tech_state::Column::Researching,
                claim_tech_state::Column::StartTimestamp,
                claim_tech_state::Column::ScheduledId,
            ])
            .to_owned();

        let mut currently_known_claim_tech_state = ::entity::claim_tech_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_tech_state::Model = new.into();

                                if currently_known_claim_tech_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_tech_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_state::Model = new.into();
                                if currently_known_claim_tech_state.contains_key(&model.entity_id) {
                                    let value = currently_known_claim_tech_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechState = id, error = error.to_string(), "Could not delete ClaimTechState");
                                }

                                tracing::debug!("ClaimTechState::Remove");
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
                tracing::debug!(
                    "ClaimTechState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_state::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechState: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_tech_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ClaimTechDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([::entity::claim_tech_desc::Column::Id])
            .update_columns([
                ::entity::claim_tech_desc::Column::Description,
                ::entity::claim_tech_desc::Column::Tier,
                ::entity::claim_tech_desc::Column::SuppliesCost,
                ::entity::claim_tech_desc::Column::ResearchTime,
                ::entity::claim_tech_desc::Column::Requirements,
                ::entity::claim_tech_desc::Column::Input,
                ::entity::claim_tech_desc::Column::Members,
                ::entity::claim_tech_desc::Column::Area,
                ::entity::claim_tech_desc::Column::Supplies,
                ::entity::claim_tech_desc::Column::XpToMintHexCoin,
            ])
            .to_owned();

        let mut currently_known_claim_tech_desc = ::entity::claim_tech_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();

                                if currently_known_claim_tech_desc.contains_key(&model.id) {
                                    let value = currently_known_claim_tech_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();
                                if currently_known_claim_tech_desc.contains_key(&model.id) {
                                    let value = currently_known_claim_tech_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_claim_tech_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechDesc = id, error = error.to_string(), "Could not delete ClaimTechDesc");
                                }

                                tracing::debug!("ClaimTechDesc::Remove");
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
                tracing::debug!(
                    "ClaimTechDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechDesc: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_building_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_state::Column::EntityId])
                .update_columns([
                    ::entity::building_state::Column::ClaimEntityId,
                    ::entity::building_state::Column::DirectionIndex,
                    ::entity::building_state::Column::BuildingDescriptionId,
                    ::entity::building_state::Column::ConstructedByPlayerEntityId,
                ])
                .to_owned();

        let mut currently_known_building_state = ::entity::building_state::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = HashMap::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_state::Model = new.into();

                                if currently_known_building_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.insert(model.entity_id, model);
                                    } else {
                                        currently_known_building_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.insert(model.entity_id, model);
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_state::Model = new.into();

                                if currently_known_building_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.insert(model.entity_id, model);
                                    } else {
                                        currently_known_building_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.insert(model.entity_id, model);
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_state::Model = delete.into();
                                let id = model.entity_id;

                                messages.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingState = id, error = error.to_string(), "Could not delete BuildingState");
                                }

                                tracing::debug!("BuildingState::Remove");
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
                tracing::debug!(
                    "BuildingState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::building_state::Entity::insert_many(
                    messages
                        .values()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting BuildingState: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_building_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([::entity::building_desc::Column::Id])
            .update_columns([
                ::entity::building_desc::Column::Functions,
                ::entity::building_desc::Column::Name,
                ::entity::building_desc::Column::Description,
                ::entity::building_desc::Column::RestedBuffDuration,
                ::entity::building_desc::Column::LightRadius,
                ::entity::building_desc::Column::ModelAssetName,
                ::entity::building_desc::Column::IconAssetName,
                ::entity::building_desc::Column::Unenterable,
                ::entity::building_desc::Column::Wilderness,
                ::entity::building_desc::Column::Footprint,
                ::entity::building_desc::Column::MaxHealth,
                ::entity::building_desc::Column::IgnoreDamage,
                ::entity::building_desc::Column::DefenseLevel,
                ::entity::building_desc::Column::Decay,
                ::entity::building_desc::Column::Maintenance,
                ::entity::building_desc::Column::BuildPermission,
                ::entity::building_desc::Column::InteractPermission,
                ::entity::building_desc::Column::HasAction,
                ::entity::building_desc::Column::ShowInCompendium,
                ::entity::building_desc::Column::IsRuins,
                ::entity::building_desc::Column::NotDeconstructible,
            ])
            .to_owned();

        let mut currently_known_building_desc = ::entity::building_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();

                                global_app_state.building_desc.insert(model.id, model.clone());
                                if currently_known_building_desc.contains_key(&model.id) {
                                    let value = currently_known_building_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_desc::Model = new.into();
                               global_app_state.building_desc.insert(model.id, model.clone());
                                if currently_known_building_desc.contains_key(&model.id) {
                                    let value = currently_known_building_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                global_app_state.building_desc.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingDesc = id, error = error.to_string(), "Could not delete BuildingDesc");
                                }

                                tracing::debug!("BuildingDesc::Remove");
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
                tracing::debug!(
                    "BuildingDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::building_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!("Error inserting BuildingDesc: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_building_nickname_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<BuildingNicknameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::columns([::entity::building_nickname_state::Column::EntityId])
                .update_columns([::entity::building_nickname_state::Column::Nickname])
                .to_owned();

        let mut currently_known_building_nickname_state =
            ::entity::building_nickname_state::Entity::find()
                .all(&global_app_state.conn)
                .await
                .map_or(vec![], |aa| aa)
                .into_iter()
                .map(|value| (value.entity_id, value))
                .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::building_nickname_state::Model = new.into();

                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if currently_known_building_nickname_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_nickname_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_nickname_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::building_nickname_state::Model = new.into();
                                global_app_state.building_nickname_state.insert(model.entity_id, model.clone());
                                if currently_known_building_nickname_state.contains_key(&model.entity_id) {
                                    let value = currently_known_building_nickname_state.get(&model.entity_id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_building_nickname_state.remove(&model.entity_id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::building_nickname_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.building_nickname_state.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(BuildingNicknameState = id, error = error.to_string(), "Could not delete BuildingNicknameState");
                                }

                                tracing::debug!("BuildingNicknameState::Remove");
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
                tracing::debug!(
                    "BuildingNicknameState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert =
                    ::entity::building_nickname_state::Entity::insert_many(messages.clone())
                        .on_conflict(on_conflict.clone())
                        .exec(&global_app_state.conn)
                        .await;

                if insert.is_err() {
                    tracing::error!(
                        "Error inserting BuildingNicknameState: {}",
                        insert.unwrap_err()
                    )
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_crafting_recipe_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<CraftingRecipeDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
            .update_columns([
                crafting_recipe::Column::Name,
                crafting_recipe::Column::TimeRequirement,
                crafting_recipe::Column::StaminaRequirement,
                crafting_recipe::Column::ToolDurabilityLost,
                crafting_recipe::Column::BuildingRequirement,
                crafting_recipe::Column::LevelRequirements,
                crafting_recipe::Column::ToolRequirements,
                crafting_recipe::Column::ConsumedItemStacks,
                crafting_recipe::Column::DiscoveryTriggers,
                crafting_recipe::Column::RequiredKnowledges,
                crafting_recipe::Column::RequiredClaimTechId,
                crafting_recipe::Column::FullDiscoveryScore,
                crafting_recipe::Column::ExperiencePerProgress,
                crafting_recipe::Column::AllowUseHands,
                crafting_recipe::Column::CraftedItemStacks,
                crafting_recipe::Column::IsPassive,
                crafting_recipe::Column::ActionsRequired,
                crafting_recipe::Column::ToolMeshIndex,
                crafting_recipe::Column::RecipePerformanceId,
            ])
            .to_owned();

        let mut currently_known_crafting_recipe = ::entity::crafting_recipe::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();

                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                if currently_known_crafting_recipe.contains_key(&model.id) {
                                    let value = currently_known_crafting_recipe.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_crafting_recipe.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();
                                global_app_state.crafting_recipe_desc.insert(model.id, model.clone());
                                if currently_known_crafting_recipe.contains_key(&model.id) {
                                    let value = currently_known_crafting_recipe.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_crafting_recipe.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::crafting_recipe::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                global_app_state.crafting_recipe_desc.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CraftingRecipeDesc = id, error = error.to_string(), "Could not delete BuildingNicknameState");
                                }

                                tracing::debug!("CraftingRecipeDesc::Remove");
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
                tracing::debug!(
                    "CraftingRecipeDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::crafting_recipe::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!(
                        "Error inserting CraftingRecipeDesc: {}",
                        insert.unwrap_err()
                    )
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_item_list_desc(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<ItemListDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(item_list_desc::Column::Id)
            .update_columns([
                item_list_desc::Column::Name,
                item_list_desc::Column::Possibilities,
            ])
            .to_owned();

        let mut currently_known_item_list_desc = ::entity::item_list_desc::Entity::find()
            .all(&global_app_state.conn)
            .await
            .map_or(vec![], |aa| aa)
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::item_list_desc::Model = new.into();

                                global_app_state.item_list_desc.insert(model.id, model.clone());
                                if currently_known_item_list_desc.contains_key(&model.id) {
                                    let value = currently_known_item_list_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_list_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::item_list_desc::Model = new.into();
                                global_app_state.item_list_desc.insert(model.id, model.clone());
                                if currently_known_item_list_desc.contains_key(&model.id) {
                                    let value = currently_known_item_list_desc.get(&model.id).unwrap();

                                    if &model != value {
                                        messages.push(model.into_active_model());
                                    } else {
                                        currently_known_item_list_desc.remove(&model.id);
                                    }
                                } else {
                                    messages.push(model.into_active_model());
                                }

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::item_list_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }

                                global_app_state.item_list_desc.remove(&id);

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ItemListDesc = id, error = error.to_string(), "Could not delete BuildingNicknameState");
                                }

                                tracing::debug!("ItemListDesc::Remove");
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
                tracing::debug!(
                    "ItemListDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::item_list_desc::Entity::insert_many(messages.clone())
                    .on_conflict(on_conflict.clone())
                    .exec(&global_app_state.conn)
                    .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ItemListDesc: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_location_state(
    global_app_state: Arc<AppState>,
    rx: AsyncReceiver<SpacetimeUpdateMessages<LocationState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        // let on_conflict = sea_query::OnConflict::columns([::entity::building_desc::Column::Id])
        //     .update_columns([
        //         ::entity::building_desc::Column::Functions,
        //         ::entity::building_desc::Column::Name,
        //         ::entity::building_desc::Column::Description,
        //         ::entity::building_desc::Column::RestedBuffDuration,
        //         ::entity::building_desc::Column::LightRadius,
        //         ::entity::building_desc::Column::ModelAssetName,
        //         ::entity::building_desc::Column::IconAssetName,
        //         ::entity::building_desc::Column::Unenterable,
        //         ::entity::building_desc::Column::Wilderness,
        //         ::entity::building_desc::Column::Footprint,
        //         ::entity::building_desc::Column::MaxHealth,
        //         ::entity::building_desc::Column::IgnoreDamage,
        //         ::entity::building_desc::Column::DefenseLevel,
        //         ::entity::building_desc::Column::Decay,
        //         ::entity::building_desc::Column::Maintenance,
        //         ::entity::building_desc::Column::BuildPermission,
        //         ::entity::building_desc::Column::InteractPermission,
        //         ::entity::building_desc::Column::HasAction,
        //         ::entity::building_desc::Column::ShowInCompendium,
        //         ::entity::building_desc::Column::IsRuins,
        //         ::entity::building_desc::Column::NotDeconstructible,
        //     ])
        //     .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::location::Model = new.into();

                                messages.push(model.clone());
                                global_app_state.location_state.insert(model.entity_id, model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::location::Model = new.into();
                                // messages.push(model.clone());
                               global_app_state.location_state.insert(model.entity_id, model);

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::location::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.location_state.remove(&id);

                                // if let Err(error) = model.delete(&global_app_state.conn).await {
                                //     tracing::error!(LocationState = id, error = error.to_string(), "Could not delete LocationState");
                                // }

                                tracing::debug!("LocationState::Remove");
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
                tracing::debug!(
                    "LocationState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                // let insert = ::entity::building_desc::Entity::insert_many(
                //     messages
                //         .iter()
                //         .map(|value| value.clone().into_active_model())
                //         .collect::<Vec<_>>(),
                // )
                // .on_conflict(on_conflict.clone())
                // .exec(&global_app_state.conn)
                // .await;
                //
                // if insert.is_err() {
                //     tracing::error!("Error inserting BuildingDesc: {}", insert.unwrap_err())
                // }
                // Your batch processing logic here

                messages.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
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
    // ClaimDescriptionState(entity::claim_description_state::Model),
    ClaimLocalState(entity::claim_local_state::Model),
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
            WebSocketMessages::ClaimLocalState(claim_local_state) => Some(vec![(
                "claim_local_state".to_string(),
                claim_local_state.entity_id,
            )]),
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
