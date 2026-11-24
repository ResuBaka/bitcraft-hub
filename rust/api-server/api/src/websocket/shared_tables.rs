//! Rows available on the global module. Regional-only configurations use the
//! same workers; region 0 denotes the global module for static data.
use super::{
    SpacetimeUpdateMessages as Message, initial_batches, send_worker_message,
    worker_channel::UnboundedSender,
};
use crate::AppState;
use entity::shared::Region;
use game_global_module::module_bindings as global;
use game_global_module::module_bindings::*;
use game_module::module_bindings as game;
use parking_lot::Mutex;
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) const GLOBAL: Region = 0;

// The two SDK modules give distinct Rust types to BitCraft's shared wire
// definitions. Decode through BSATN at this boundary, keeping existing workers
// and API models identical for global and regional-only deployments.
fn shared_row<
    G: spacetimedb_sdk::__codegen::__sats::ser::Serialize,
    R: for<'a> spacetimedb_sdk::__codegen::__sats::de::Deserialize<'a>,
>(
    row: &G,
) -> anyhow::Result<R> {
    use spacetimedb_sdk::__codegen::__sats::bsatn;
    let bytes = bsatn::to_vec(row)?;
    let mut input = bytes.as_slice();
    let result = bsatn::from_reader(&mut input)?;
    anyhow::ensure!(input.is_empty(), "shared row has trailing fields");
    Ok(result)
}

pub(super) struct Snapshot {
    finish: Vec<Box<dyn FnOnce() + Send>>,
    error: Arc<Mutex<Option<anyhow::Error>>>,
}

impl Snapshot {
    pub(super) fn check(&self) -> anyhow::Result<()> {
        self.error.lock().take().map_or(Ok(()), Err)
    }
    pub(super) fn finish(&mut self) {
        for finish in self.finish.drain(..) {
            finish();
        }
    }
}

// Bind the SQL, worker sender type and callbacks to the same table list.
macro_rules! shared_tables {
    ($($table:ident: $row:ident => $scope:ident),+ $(,)?) => {
        #[derive(Clone)]
        pub(super) struct Workers {
            $(pub(super) $table: UnboundedSender<Message<game::$row>>,)+
        }

        pub(super) const TABLES: &[&str] = &[$(stringify!($table)),+];

        impl Workers {
            pub(super) fn listen_regional(&self, ctx: &game::DbConnection, database: &str, region: Region, descriptions: bool) {
                use game_module::module_bindings::*;
                use super::{ListenerMetrics, ListenerRow, SpacetimeUpdateMessages, WorkerSendMetrics};
                $(if descriptions || !stringify!($table).ends_with("_desc") {
                    let tx = &self.$table;
                    super::setup_spacetime_db_listeners!(ctx, $table, tx, $row, database, region, stringify!($table));
                })+
            }

            pub(super) fn listen(&self, ctx: &global::DbConnection, regions: &[Region], state: &AppState) -> Snapshot {
                let mut snapshot = Snapshot { finish: Vec::new(), error: Default::default() };
                $({
                    let table = ctx.db.$table();
                    let tx = self.$table.clone();
                    let name = stringify!($table);
                    let regions = regions.to_vec();
                    let initial = Arc::new(Mutex::new(BTreeMap::<Region, Vec<game::$row>>::new()));
                    let target = initial.clone();
                    let error = snapshot.error.clone();
                    table.on_initial(move |_, batch| {
                        let mut batches = target.lock();
                        for row in batch {
                            let region = row_region!($scope, row);
                            match shared_row(row) {
                                Ok(row) => batches.entry(region).or_default().push(row),
                                Err(e) => { *error.lock() = Some(e.context(name)); return; }
                            }
                        }
                    });
                    snapshot.finish.push(Box::new(move || {
                        let mut batches = std::mem::take(&mut *initial.lock());
                        // Region-scoped workers reconcile a whole region at a
                        // time, including a previously populated, now empty one.
                        for region in snapshot_regions!($scope, regions) {
                            batches.entry(region).or_default();
                        }
                        for (region, rows) in batches {
                            initial_batches::send_initial(name, &tx, rows, region, 0);
                        }
                    }));
                    let tx = self.$table.clone();
                    let error = snapshot.error.clone();
                    table.on_insert(move |ctx, row| {
                        if !matches!(ctx.event, Event::SubscribeApplied) {
                            match shared_row(row) {
                                Ok(new) => send_worker_message(name, &tx, Message::Insert {event: None, database_name: row_region!($scope, row), new}),
                                Err(e) => *error.lock() = Some(e.context(name)),
                            }
                        }
                    });
                    let tx = self.$table.clone();
                    let error = snapshot.error.clone();
                    table.on_update(move |_, old, row| {
                        match shared_row(old).and_then(|old| Ok((old, shared_row(row)?))) {
                            Ok((old, new)) => send_worker_message(name, &tx, Message::Update {event: None, database_name: row_region!($scope, row), old, new}),
                            Err(e) => *error.lock() = Some(e.context(name)),
                        }
                    });
                    let tx = self.$table.clone();
                    let error = snapshot.error.clone();
                    table.on_delete(move |_, row| {
                        match shared_row(row) {
                            Ok(delete) => send_worker_message(name, &tx, Message::Remove {event: None, database_name: row_region!($scope, row), delete}),
                            Err(e) => *error.lock() = Some(e.context(name)),
                        }
                    });
                })+
                listen_metadata(ctx, state, &mut snapshot);
                snapshot
            }
        }

        #[cfg(test)]
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        #[ignore = "requires BITCRAFT_TEST_URI and BITCRAFT_TEST_GLOBAL; reads a live global snapshot"]
        async fn live_shared_snapshot() -> anyhow::Result<()> {
            use spacetimedb_sdk::DbContext;
            use super::worker_channel::unbounded_channel;
            $(let mut $table = unbounded_channel();)+
            let workers = Workers { $($table: $table.0.clone(),)+ };
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let state = AppState::new(Default::default(), tx, Default::default());
            let ctx = global::DbConnection::builder()
                .with_uri(std::env::var("BITCRAFT_TEST_URI")?)
                .with_database_name(std::env::var("BITCRAFT_TEST_GLOBAL")?).build()?;
            let mut snapshot = workers.listen(&ctx, &[7], &state);
            let outcome = Arc::new(Mutex::new(None));
            let applied = outcome.clone();
            let failed = outcome.clone();
            ctx.subscription_builder()
                .on_applied(move |_| *applied.lock() = Some(Ok(())))
                .on_error(move |_, err| *failed.lock() = Some(Err(err)))
                .subscribe(TABLES.iter().chain(METADATA).map(|t| format!("SELECT * FROM {t}")).collect::<Vec<_>>());
            tokio::time::timeout(std::time::Duration::from_secs(60), async {
                loop {
                    ctx.advance_one_message_async().await?;
                    if let Some(result) = outcome.lock().take() { break result; }
                }
            }).await??;
            snapshot.check()?;
            snapshot.finish();
            $(assert!(matches!($table.1.try_recv(), Ok(Message::Initial { .. })), "{}", stringify!($table));)+
            assert!(state.player_stats.tool_powers().is_some());
            ctx.disconnect()?;
            Ok(())
        }
    }
}

macro_rules! row_region {
    (global, $row:expr) => {
        GLOBAL
    };
    // game::game_state::create_entity stores the allocating region in the
    // top byte. Claims and buildings are stationary; players are not.
    (stationary, $row:expr) => {
        ($row.entity_id >> 56) as Region
    };
}
macro_rules! snapshot_regions {
    (global, $regions:expr) => {{
        let _ = $regions;
        [GLOBAL]
    }};
    (stationary, $regions:expr) => {
        $regions
    };
}

shared_tables!(
    item_desc: ItemDesc => global,
    item_list_desc: ItemListDesc => global,
    cargo_desc: CargoDesc => global,
    crafting_recipe_desc: CraftingRecipeDesc => global,
    skill_desc: SkillDesc => global,
    building_desc: BuildingDesc => global,
    collectible_desc: CollectibleDesc => global,
    claim_tech_desc: ClaimTechDesc => global,
    resource_desc: ResourceDesc => global,
    extraction_recipe_desc: ExtractionRecipeDesc => global,
    traveler_task_desc: TravelerTaskDesc => global,
    npc_desc: NpcDesc => global,
    interior_network_desc: InteriorNetworkDesc => global,
    claim_state: ClaimState => stationary,
    building_nickname_state: BuildingNicknameState => stationary,
    player_housing_state: PlayerHousingState => global,
);

pub(super) const METADATA: &[&str] = &[
    "equipment_desc",
    "tool_desc",
    "tool_type_desc",
    "buff_desc",
    "buff_type_desc",
    "food_desc",
];

pub(super) fn owns(table: &str) -> bool {
    TABLES.contains(&table) || METADATA.contains(&table) || table == "player_username_state"
}

macro_rules! metadata_listeners {
    ($ctx:expr, $state:expr, $source:expr, $snapshot:expr, $convert:path) => {{
        let (ctx, state, source, snapshot) = ($ctx, $state, $source, $snapshot);
        state.player_stats.begin_food(source);
        state.player_stats.begin_tools(source);
        state.player_stats.clear_buff_descriptions();
        state.player_equipment.begin_global_metadata();

        macro_rules! metadata {
            ($table:ident, $row:ty, $key:ident, $apply:expr) => {{
                let table = ctx.db.$table();
                let target = state.clone();
                let error = snapshot.error.clone();
                let apply = move |row: &$row, delete: bool| {
                    let result = if delete {
                        Ok(None)
                    } else {
                        $convert(row).map(Some)
                    };
                    match result {
                        Ok(value) => ($apply)(&target, row.$key, value),
                        Err(e) => *error.lock() = Some(e.context(stringify!($table))),
                    }
                };
                let initial = apply.clone();
                table.on_initial(move |_, rows| {
                    for row in rows {
                        initial(row, false);
                    }
                });
                let insert = apply.clone();
                let target = state.clone();
                table.on_insert(move |ctx, row| {
                    if !matches!(ctx.event, Event::SubscribeApplied) {
                        insert(row, false);
                        super::player_stats::changed(&target, None);
                    }
                });
                let update = apply.clone();
                let target = state.clone();
                table.on_update(move |_, _, row| {
                    update(row, false);
                    super::player_stats::changed(&target, None);
                });
                let target = state.clone();
                table.on_delete(move |_, row| {
                    apply(row, true);
                    super::player_stats::changed(&target, None);
                });
            }};
        }
        metadata!(buff_desc, BuffDesc, id, |s: &AppState, key, row| s
            .player_stats
            .buff_desc(key, row));
        metadata!(
            buff_type_desc,
            BuffTypeDesc,
            id,
            |s: &AppState, key, row| s.player_stats.buff_type(key, row)
        );
        metadata!(food_desc, FoodDesc, item_id, |s: &AppState, key, row| s
            .player_stats
            .food(source, key, row));
        metadata!(
            tool_desc,
            ToolDesc,
            id,
            |s: &AppState, key, row: Option<game::ToolDesc>| {
                s.player_stats.update_tools(source, |index| {
                    if let Some(row) = row {
                        index.descriptions.insert(key, row);
                    } else {
                        index.descriptions.remove(&key);
                    }
                });
            }
        );
        metadata!(
            tool_type_desc,
            ToolTypeDesc,
            id,
            |s: &AppState, key, row| {
                s.player_stats.update_tools(source, |index| {
                    if let Some(row) = row {
                        index.types.insert(key, row);
                    } else {
                        index.types.remove(&key);
                    }
                });
            }
        );
        metadata!(equipment_desc, EquipmentDesc, item_id, |s: &AppState,
                                                           key,
                                                           row: Option<
            game::EquipmentDesc,
        >| {
            s.player_equipment
                .global_item_stats(key, row.map(|desc| desc.stats));
        });
        let state = state.clone();
        snapshot.finish.push(Box::new(move || {
            state.player_stats.finish_food(source);
            state
                .player_stats
                .update_tools(source, |index| index.ready = true);
            super::player_stats::changed(&state, None);
            super::player_stats::character_changed(&state, None, None);
        }));
    }};
}

fn listen_metadata(ctx: &global::DbConnection, state: &AppState, snapshot: &mut Snapshot) {
    metadata_listeners!(ctx, state, GLOBAL, snapshot, shared_row);
}

pub(super) fn regional_metadata(
    ctx: &game::DbConnection,
    state: &AppState,
    region: Region,
) -> Snapshot {
    use game_module::module_bindings::*;
    fn copy<T: Clone>(row: &T) -> anyhow::Result<T> {
        Ok(row.clone())
    }
    let mut snapshot = Snapshot {
        finish: Vec::new(),
        error: Default::default(),
    };
    metadata_listeners!(ctx, state, region, &mut snapshot, copy);
    snapshot
}

#[cfg(test)]
#[tokio::test]
#[ignore = "requires SHARED_SYNC_TEST_URL; uses PostgreSQL temporary tables"]
async fn reconnect_prunes_postgres_through_workers() -> anyhow::Result<()> {
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
    use std::time::Duration;
    let mut options = ConnectOptions::new(std::env::var("SHARED_SYNC_TEST_URL")?);
    options.max_connections(1).set_schema_search_path("pg_temp");
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "
        CREATE TEMP TABLE resource_desc (id integer PRIMARY KEY);
        CREATE TEMP TABLE extraction_recipe_desc (id integer PRIMARY KEY);
        CREATE TEMP TABLE interior_network_desc (building_id integer PRIMARY KEY);
        CREATE TEMP TABLE player_housing_state (entity_id bigint PRIMARY KEY, region_index integer);
        INSERT INTO resource_desc VALUES (1);
        INSERT INTO extraction_recipe_desc VALUES (2);
        INSERT INTO interior_network_desc VALUES (3);
        INSERT INTO player_housing_state VALUES (4,7), (5,8);
    ",
    )
    .await?;
    for source in [7, GLOBAL] {
        let (out, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        macro_rules! empty_snapshot {
            ($worker:ident, $source:expr) => {{
                let (tx, rx) = super::worker_channel::shutdown_channel(&state.shutdown);
                super::$worker(state.clone(), rx, 10, Duration::from_secs(1));
                tx.send(Message::Initial {
                    data: vec![],
                    batches: None,
                    database_name: $source,
                })
                .unwrap();
            }};
        }
        empty_snapshot!(start_worker_resource_desc, GLOBAL);
        empty_snapshot!(start_worker_extraction_recipe_desc, GLOBAL);
        empty_snapshot!(start_worker_interior_network_desc, GLOBAL);
        empty_snapshot!(start_worker_player_housing_state, source);
        state.shutdown.drain().await;
        let result = db.query_one_raw(Statement::from_string(DbBackend::Postgres, "
            SELECT (SELECT count(*) FROM resource_desc) + (SELECT count(*) FROM extraction_recipe_desc)
                + (SELECT count(*) FROM interior_network_desc) AS descriptions,
                ARRAY(SELECT entity_id FROM player_housing_state) AS houses"
        )).await?.unwrap();
        assert_eq!(result.try_get::<i64>("", "descriptions")?, 0);
        assert_eq!(
            result.try_get::<Vec<i64>>("", "houses")?,
            if source == 7 { vec![5] } else { vec![] }
        );
    }
    Ok(())
}
