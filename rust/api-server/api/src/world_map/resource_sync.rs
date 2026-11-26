//! Dedicated regional subscriptions for the resource projection. The main
//! location worker never sees these large snapshots or their live updates.
use super::resource_clusters;
use crate::{AppState, config::Config};
use anyhow::{Context, Result, ensure};
use entity::shared::Region;
use game_module::module_bindings::{
    DbConnection, LocationState, LocationStateTableAccess, ResourceState, ResourceStateTableAccess,
};
use parking_lot::Mutex;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
    sqlx::{self, Acquire, PgConnection},
};
use spacetimedb_sdk::{DbContext, Event, Table, TableWithPrimaryKey};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

const QUERIES: [&str; 2] = [
    "SELECT resource_state.* FROM resource_state JOIN location_state
     ON resource_state.entity_id = location_state.entity_id WHERE location_state.dimension = 1",
    "SELECT location_state.* FROM location_state JOIN resource_state
     ON location_state.entity_id = resource_state.entity_id WHERE location_state.dimension = 1",
];
const WRITE_BATCH: usize = 3_000;
const COPY_ROWS: usize = 16_384;
const COPY_HEADER: &[u8] = b"PGCOPY\n\xff\r\n\0\0\0\0\0\0\0\0\0";
const COPY_ROW_BYTES: usize = 2 + 4 * 4 + 8 + 3 * 4;
const FLUSH_INTERVAL: Duration = Duration::from_millis(150);
type Point = (i32, i32);

#[derive(Default)]
struct Snapshot {
    resources: Vec<ResourceState>,
    locations: Vec<LocationState>,
}

#[derive(Default)]
pub(super) struct Changes {
    pub(super) resources: HashMap<u64, Option<i32>>,
    pub(super) locations: HashMap<u64, Option<Point>>,
}

impl Changes {
    fn len(&self) -> usize {
        self.resources.len() + self.locations.len()
    }
}

#[derive(Default)]
struct Inbox {
    applied: bool,
    error: Option<String>,
    snapshot: Snapshot,
    changes: Changes,
}

fn listen(ctx: &DbConnection) -> Arc<Mutex<Inbox>> {
    let inbox = Arc::new(Mutex::new(Inbox::default()));
    let resources = ctx.db.resource_state();
    let target = inbox.clone();
    resources.on_initial_owned(move |_, rows| target.lock().snapshot.resources = rows);
    let target = inbox.clone();
    resources.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target
                .lock()
                .changes
                .resources
                .insert(row.entity_id, Some(row.resource_id));
        }
    });
    let target = inbox.clone();
    resources.on_update(move |_, old, row| {
        if old.resource_id != row.resource_id {
            target
                .lock()
                .changes
                .resources
                .insert(row.entity_id, Some(row.resource_id));
        }
    });
    let target = inbox.clone();
    resources.on_delete(move |_, row| {
        target.lock().changes.resources.insert(row.entity_id, None);
    });

    let locations = ctx.db.location_state();
    let target = inbox.clone();
    locations.on_initial_owned(move |_, rows| target.lock().snapshot.locations = rows);
    let target = inbox.clone();
    let position = |row: &LocationState| (row.dimension == 1).then_some((row.x, row.z));
    locations.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target
                .lock()
                .changes
                .locations
                .insert(row.entity_id, position(row));
        }
    });
    let target = inbox.clone();
    locations.on_update(move |_, old, row| {
        if position(old) != position(row) {
            target
                .lock()
                .changes
                .locations
                .insert(row.entity_id, position(row));
        }
    });
    let target = inbox.clone();
    locations.on_delete(move |_, row| {
        target.lock().changes.locations.insert(row.entity_id, None);
    });

    let applied = inbox.clone();
    let failed = inbox.clone();
    ctx.subscription_builder()
        .on_applied(move |_| applied.lock().applied = true)
        .on_error(move |_, error| failed.lock().error = Some(error.to_string()))
        .subscribe(QUERIES);
    inbox
}

async fn write_resources(
    txn: &DatabaseTransaction,
    region: Region,
    mut rows: impl Iterator<Item = (u64, Option<i32>)> + Send,
) -> Result<()> {
    let sql = "INSERT INTO atlas_resource_entry (region, entity_id, resource_id)
         SELECT $1, entity_id, CASE WHEN present THEN id END
         FROM unnest($2::bigint[], $3::integer[], $4::boolean[]) AS r(entity_id, id, present)
         ON CONFLICT (region, entity_id) DO UPDATE SET resource_id = EXCLUDED.resource_id";
    loop {
        let mut entities = Vec::new();
        let mut ids = Vec::new();
        let mut present = Vec::new();
        for (id, kind) in rows.by_ref().take(WRITE_BATCH) {
            entities.push(id as i64);
            ids.push(kind.unwrap_or_default());
            present.push(kind.is_some());
        }
        if entities.is_empty() {
            return Ok(());
        }
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [region.into(), entities.into(), ids.into(), present.into()],
        ))
        .await?;
    }
}

async fn write_locations(
    txn: &DatabaseTransaction,
    region: Region,
    mut rows: impl Iterator<Item = (u64, Option<Point>)> + Send,
) -> Result<()> {
    let sql =
        "INSERT INTO atlas_resource_entry (region, entity_id, x, z)
         SELECT $1, entity_id, CASE WHEN present THEN x END, CASE WHEN present THEN z END
         FROM unnest($2::bigint[], $3::integer[], $4::integer[], $5::boolean[]) AS l(entity_id, x, z, present)
         ON CONFLICT (region, entity_id) DO UPDATE SET x = EXCLUDED.x, z = EXCLUDED.z";
    loop {
        let mut entities = Vec::new();
        let mut xs = Vec::new();
        let mut zs = Vec::new();
        let mut present = Vec::new();
        for (id, position) in rows.by_ref().take(WRITE_BATCH) {
            let (x, z) = position.unwrap_or_default();
            entities.push(id as i64);
            xs.push(x);
            zs.push(z);
            present.push(position.is_some());
        }
        if entities.is_empty() {
            return Ok(());
        }
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [
                region.into(),
                entities.into(),
                xs.into(),
                zs.into(),
                present.into(),
            ],
        ))
        .await?;
    }
}

struct SnapshotTimings {
    sort_ms: u64,
    validate_ms: u64,
    setup_ms: u64,
    copy_ms: u64,
    stage_index_ms: u64,
    analyze_ms: u64,
    merge_ms: u64,
    merged_rows: u64,
    cluster_ms: u64,
    cleanup_ms: u64,
    commit_ms: u64,
    db_total_ms: u64,
}

fn prepare_snapshot(mut data: Snapshot) -> Result<(Snapshot, u64, u64)> {
    ensure!(
        data.resources.len() == data.locations.len(),
        "Resource subscription snapshot has mismatched projection lengths"
    );
    let started = Instant::now();
    // Sort in place by the signed PostgreSQL key. Zipping the projections
    // avoids allocating a large join map or writing the staging rows twice.
    data.resources
        .sort_unstable_by_key(|row| row.entity_id as i64);
    data.locations
        .sort_unstable_by_key(|row| row.entity_id as i64);
    let sort_ms = started.elapsed().as_millis() as u64;
    let started = Instant::now();
    let mut previous = None;
    for (resource, location) in data.resources.iter().zip(&data.locations) {
        ensure!(
            resource.entity_id == location.entity_id && location.dimension == 1,
            "Resource snapshot contains unmatched or non-overworld entities"
        );
        ensure!(
            previous != Some(resource.entity_id),
            "Resource snapshot contains duplicate entities"
        );
        previous = Some(resource.entity_id);
    }
    Ok((data, sort_ms, started.elapsed().as_millis() as u64))
}

fn encode_copy_row(buf: &mut Vec<u8>, resource: &ResourceState, location: &LocationState) {
    // Binary COPY fields each have a byte length followed by a big-endian value.
    buf.extend_from_slice(&4_i16.to_be_bytes());
    buf.extend_from_slice(&8_i32.to_be_bytes());
    buf.extend_from_slice(&(resource.entity_id as i64).to_be_bytes());
    for value in [resource.resource_id, location.x, location.z] {
        buf.extend_from_slice(&4_i32.to_be_bytes());
        buf.extend_from_slice(&value.to_be_bytes());
    }
}

async fn copy_snapshot(txn: &mut PgConnection, data: &Snapshot) -> Result<()> {
    let mut copy = txn
        .copy_in_raw(
            "COPY atlas_resource_stage (entity_id, resource_id, x, z) FROM STDIN (FORMAT BINARY)",
        )
        .await?;
    let result: sqlx::Result<()> = async {
        let mut buf = Vec::with_capacity(COPY_HEADER.len() + COPY_ROWS * COPY_ROW_BYTES + 2);
        buf.extend_from_slice(COPY_HEADER);
        for (resources, locations) in data
            .resources
            .chunks(COPY_ROWS)
            .zip(data.locations.chunks(COPY_ROWS))
        {
            for (resource, location) in resources.iter().zip(locations) {
                encode_copy_row(&mut buf, resource, location);
            }
            copy.send(buf.as_slice()).await?;
            buf.clear();
        }
        buf.extend_from_slice(&(-1_i16).to_be_bytes());
        copy.send(buf.as_slice()).await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        let _ = copy.abort("Atlas resource snapshot COPY failed").await;
        return Err(error.into());
    }
    let copied = copy.finish().await?;
    ensure!(
        copied == data.resources.len() as u64,
        "Resource snapshot COPY row count mismatch"
    );
    Ok(())
}

async fn replace_snapshot(
    db: &DatabaseConnection,
    region: Region,
    data: Snapshot,
) -> Result<SnapshotTimings> {
    let (data, sort_ms, validate_ms) = tokio::task::spawn_blocking(move || prepare_snapshot(data))
        .await
        .context("Resource snapshot preparation task failed")??;
    let started = Instant::now();
    let mut phase_started = started;
    let mut phase_ms = || {
        let now = Instant::now();
        let ms = now.duration_since(phase_started).as_millis() as u64;
        phase_started = now;
        ms
    };
    let mut conn = db.get_postgres_connection_pool().acquire().await?;
    // Cancellation may interrupt a COPY message mid-write. Keep that
    // connection out of the general request pool even on shutdown.
    conn.close_on_drop();
    let mut txn = conn.begin().await?;
    sqlx::query(
        "CREATE TEMP TABLE atlas_resource_stage (
            entity_id bigint NOT NULL, resource_id integer NOT NULL,
            x integer NOT NULL, z integer NOT NULL
        ) ON COMMIT DROP",
    )
    .execute(&mut *txn)
    .await?;
    let setup_ms = phase_ms();
    copy_snapshot(&mut txn, &data).await?;
    drop(data);
    let copy_ms = phase_ms();
    // Build after COPY so incoming rows don't pay per-row B-tree maintenance.
    sqlx::query("CREATE UNIQUE INDEX ON atlas_resource_stage (entity_id)")
        .execute(&mut *txn)
        .await?;
    let stage_index_ms = phase_ms();
    sqlx::query("ANALYZE atlas_resource_stage")
        .execute(&mut *txn)
        .await?;
    let analyze_ms = phase_ms();
    // Skip unchanged rows before ON CONFLICT so reconnects don't lock or
    // probe the unique index again for every saved node.
    let merged_rows = sqlx::query(
        "INSERT INTO atlas_resource_entry AS dst (region, entity_id, resource_id, x, z)
         SELECT $1, src.entity_id, src.resource_id, src.x, src.z
         FROM atlas_resource_stage src
         LEFT JOIN atlas_resource_entry old ON old.region = $1 AND old.entity_id = src.entity_id
         WHERE old.entity_id IS NULL
            OR (old.resource_id, old.x, old.z) IS DISTINCT FROM (src.resource_id, src.x, src.z)
         ON CONFLICT (region, entity_id) DO UPDATE
         SET resource_id = EXCLUDED.resource_id, x = EXCLUDED.x, z = EXCLUDED.z
         WHERE (dst.resource_id, dst.x, dst.z) IS DISTINCT FROM (EXCLUDED.resource_id, EXCLUDED.x, EXCLUDED.z)",
    ).bind(region).execute(&mut *txn).await?.rows_affected();
    let merge_ms = phase_ms();
    let deleted_rows = sqlx::query(
        "DELETE FROM atlas_resource_entry dst WHERE dst.region = $1 AND NOT EXISTS (
             SELECT 1 FROM atlas_resource_stage src WHERE src.entity_id = dst.entity_id
         )",
    )
    .bind(region)
    .execute(&mut *txn)
    .await?
    .rows_affected();
    let prune_ms = phase_ms();
    if merged_rows != 0
        || deleted_rows != 0
        || !sqlx::query_scalar::<_, bool>(
            "SELECT clusters_synced FROM atlas_resource_region WHERE region = $1",
        )
        .bind(region)
        .fetch_optional(&mut *txn)
        .await?
        .unwrap_or(false)
    {
        resource_clusters::rebuild(&mut txn, region).await?;
    }
    let cluster_ms = phase_ms();
    for sql in [
        "INSERT INTO atlas_resource_region (region, synced_at, subscription_synced, clusters_synced)
         VALUES ($1, clock_timestamp(), true, true)
         ON CONFLICT (region) DO UPDATE
         SET synced_at = EXCLUDED.synced_at, subscription_synced = true, clusters_synced = true",
        "DELETE FROM atlas_resource_node WHERE region = $1",
    ] {
        sqlx::query(sql).bind(region).execute(&mut *txn).await?;
    }
    let cleanup_ms = prune_ms + phase_ms();
    txn.commit().await?;
    let commit_ms = phase_ms();
    Ok(SnapshotTimings {
        sort_ms,
        validate_ms,
        setup_ms,
        copy_ms,
        stage_index_ms,
        analyze_ms,
        merge_ms,
        merged_rows,
        cluster_ms,
        cleanup_ms,
        commit_ms,
        db_total_ms: started.elapsed().as_millis() as u64,
    })
}

async fn flush(db: &DatabaseConnection, region: Region, changes: Changes) -> Result<()> {
    if changes.len() == 0 {
        return Ok(());
    }
    let mut removed: Vec<i64> = changes
        .resources
        .iter()
        .filter_map(|(&id, value)| value.is_none().then_some(id as i64))
        .chain(
            changes
                .locations
                .iter()
                .filter_map(|(&id, value)| value.is_none().then_some(id as i64)),
        )
        .collect();
    removed.sort_unstable();
    removed.dedup();
    let txn = db.begin().await?;
    let cluster_deltas = resource_clusters::deltas(&txn, region, &changes).await?;
    write_resources(&txn, region, changes.resources.into_iter()).await?;
    write_locations(&txn, region, changes.locations.into_iter()).await?;
    for ids in removed.chunks(WRITE_BATCH) {
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM atlas_resource_entry WHERE region = $1 AND entity_id = ANY($2::bigint[])
             AND resource_id IS NULL AND x IS NULL",
            [region.into(), ids.to_vec().into()],
        ))
        .await?;
    }
    resource_clusters::write(&txn, region, cluster_deltas).await?;
    txn.commit().await?;
    Ok(())
}

async fn drive(
    ctx: &DbConnection,
    db: &DatabaseConnection,
    region: Region,
    permit: OwnedSemaphorePermit,
) -> Result<()> {
    let started = Instant::now();
    let inbox = listen(ctx);
    let snapshot = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            ctx.advance_one_message_async().await?;
            let mut rows = inbox.lock();
            if let Some(error) = rows.error.take() {
                anyhow::bail!("Resource subscription failed: {error}");
            }
            // on_applied precedes the owned row callbacks. Advancing the
            // full message before reading the inbox also handles empty regions.
            if rows.applied {
                return Ok(std::mem::take(&mut rows.snapshot));
            }
        }
    })
    .await
    .context("Timed out waiting for resource subscription")??;
    let receive_ms = started.elapsed().as_millis() as u64;
    let count = snapshot.resources.len();
    let timings = replace_snapshot(db, region, snapshot).await?;
    tracing::info!(
        region,
        count,
        receive_ms,
        sort_ms = timings.sort_ms,
        validate_ms = timings.validate_ms,
        db_setup_ms = timings.setup_ms,
        copy_ms = timings.copy_ms,
        stage_index_ms = timings.stage_index_ms,
        analyze_ms = timings.analyze_ms,
        merge_ms = timings.merge_ms,
        merged_rows = timings.merged_rows,
        cluster_ms = timings.cluster_ms,
        cleanup_ms = timings.cleanup_ms,
        commit_ms = timings.commit_ms,
        db_total_ms = timings.db_total_ms,
        total_ms = started.elapsed().as_millis() as u64,
        "Loaded atlas resource subscription snapshot",
    );
    drop(permit);

    let mut tick = tokio::time::interval(FLUSH_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let flush_now = tokio::select! {
            biased;
            _ = tick.tick() => true,
            received = ctx.advance_one_message_async() => {
                received?;
                let mut rows = inbox.lock();
                if let Some(error) = rows.error.take() {
                    anyhow::bail!("Resource subscription failed: {error}");
                }
                rows.changes.len() >= WRITE_BATCH
            }
        };
        if flush_now {
            let changes = std::mem::take(&mut inbox.lock().changes);
            flush(db, region, changes).await?;
        }
    }
}

async fn run_region(
    config: &Config,
    db: &DatabaseConnection,
    region: Region,
    database: &str,
    permit: OwnedSemaphorePermit,
    stop: &CancellationToken,
) -> Result<()> {
    let token = config.spacetimedb.password.trim();
    let ctx = DbConnection::builder()
        .with_uri(config.spacetimedb_url(region))
        .with_database_name(database)
        .with_token((!token.is_empty()).then_some(token))
        .with_compression(config.spacetimedb.compression.into())
        .with_row_deduplication(true)
        .build()?;
    let result = tokio::select! {
        biased;
        _ = stop.cancelled() => Ok(()),
        result = drive(&ctx, db, region, permit) => result,
    };
    if ctx.is_active() && ctx.disconnect().is_ok() {
        // disconnect queues a mutation; drive it before reconnecting.
        let _ = tokio::time::timeout(Duration::from_secs(2), ctx.run_async()).await;
    }
    result
}

pub(crate) fn start(config: &Config, state: &AppState) {
    if !config.world_map.resource_sync_enabled || config.spacetimedb.skip_state_tables {
        return;
    }
    let gate = Arc::new(Semaphore::new(1));
    let settings = Arc::new(config.clone());
    for database in config.spacetimedb.regional_databases() {
        let Some(region) = database
            .rsplit('-')
            .next()
            .and_then(|id| id.parse::<Region>().ok())
        else {
            continue;
        };
        let database = database.clone();
        let settings = settings.clone();
        let db = state.conn.clone();
        let stop = state.shutdown.stop.clone();
        let gate = gate.clone();
        state.shutdown.spawn_connection(async move {
            loop {
                let permit = tokio::select! {
                    biased;
                    _ = stop.cancelled() => return,
                    permit = gate.clone().acquire_owned() => permit.expect("resource snapshot gate remains open"),
                };
                if let Err(err) = run_region(&settings, &db, region, &database, permit, &stop).await {
                    if !stop.is_cancelled() {
                        tracing::warn!(region, %err, "Atlas resource subscription ended; keeping saved locations until reconnect");
                    }
                }
                tokio::select! {
                    _ = stop.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(15)) => {},
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{MockDatabase, MockExecResult};

    fn sample_snapshot() -> Snapshot {
        Snapshot {
            resources: vec![ResourceState {
                entity_id: 100,
                resource_id: 12,
                direction_index: 0,
            }],
            locations: vec![LocationState {
                entity_id: 100,
                chunk_index: 3,
                x: 300,
                z: 400,
                dimension: 1,
            }],
        }
    }

    #[test]
    fn validates_projection_pairs_before_opening_a_database_transaction() {
        let mut snapshot = sample_snapshot();
        snapshot.resources.push(ResourceState {
            entity_id: u64::MAX,
            ..snapshot.resources[0].clone()
        });
        snapshot.locations.insert(
            0,
            LocationState {
                entity_id: u64::MAX,
                ..snapshot.locations[0].clone()
            },
        );
        let (snapshot, _, _) = prepare_snapshot(snapshot).unwrap();
        assert_eq!(snapshot.resources[0].entity_id, u64::MAX);
        assert_eq!(snapshot.locations[0].entity_id, u64::MAX);
        assert_eq!(
            snapshot.resources[1].entity_id,
            snapshot.locations[1].entity_id
        );

        let mut mismatch = sample_snapshot();
        mismatch.locations[0].entity_id = 101;
        assert!(prepare_snapshot(mismatch).is_err());
        let mut interior = sample_snapshot();
        interior.locations[0].dimension = 2;
        assert!(prepare_snapshot(interior).is_err());
        let mut duplicate = sample_snapshot();
        duplicate.resources.push(duplicate.resources[0].clone());
        duplicate.locations.push(duplicate.locations[0].clone());
        assert!(prepare_snapshot(duplicate).is_err());
    }

    #[tokio::test]
    #[ignore = "requires ATLAS_RESOURCE_TEST_URL; creates an isolated schema"]
    async fn copies_and_reconciles_snapshots_in_postgres() -> Result<()> {
        let url = std::env::var("ATLAS_RESOURCE_TEST_URL")?;
        let admin = sea_orm::Database::connect(&url).await?;
        let schema = format!("atlas_resource_test_{:016x}", rand::random::<u64>());
        admin
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await?;
        let result = async {
            let mut opts = sea_orm::ConnectOptions::new(url);
            opts.max_connections(1).set_schema_search_path(&schema);
            let db = sea_orm::Database::connect(opts).await?;
            db.execute_unprepared(
                "CREATE TABLE atlas_resource_entry (
                   region smallint NOT NULL, entity_id bigint NOT NULL, resource_id integer,
                   x integer, z integer, PRIMARY KEY (region, entity_id)
                 );
                 CREATE TABLE atlas_resource_region (region smallint PRIMARY KEY, synced_at timestamptz, subscription_synced boolean, clusters_synced boolean);
                 CREATE TABLE atlas_resource_cluster (
                   region smallint NOT NULL, level integer NOT NULL, resource_id integer NOT NULL,
                    bucket_x integer NOT NULL, bucket_z integer NOT NULL, node_count integer NOT NULL,
                    PRIMARY KEY (region, level, resource_id, bucket_x, bucket_z)
                 );
                   CREATE TABLE atlas_resource_node (region smallint, resource_id integer, x integer, z integer);
                   INSERT INTO atlas_resource_node (region) VALUES (7), (8);
                  INSERT INTO atlas_resource_region VALUES (8, now(), true, true);
                  INSERT INTO atlas_resource_cluster VALUES (8, 0, 12, 6, 8, 1000);
                  INSERT INTO atlas_resource_entry VALUES (7, 99, 5, 1, 2), (8, 99, 5, 1, 2);",
            ).await?;
            use migration::{Migrator, MigratorTrait, SchemaManager};
            Migrator::migrations()
                .into_iter()
                .find(|m| m.name() == "m20261126_000005_atlas_resource_cluster_positions")
                .unwrap()
                .up(&SchemaManager::new(&db))
                .await?;
            let make_snapshot = || {
                let mut snapshot = sample_snapshot();
                let resource = snapshot.resources.pop().unwrap();
                let location = snapshot.locations.pop().unwrap();
                for id in 100..100 + COPY_ROWS as u64 + 1 {
                    snapshot.resources.push(ResourceState { entity_id: id, ..resource.clone() });
                    snapshot.locations.push(LocationState { entity_id: id, ..location.clone() });
                }
                snapshot.locations.reverse();
                snapshot
            };
            assert_eq!(replace_snapshot(&db, 7, make_snapshot()).await?.merged_rows, COPY_ROWS as u64 + 1);
            assert_eq!(replace_snapshot(&db, 7, make_snapshot()).await?.merged_rows, 0);
            let mut changed = make_snapshot();
            changed.locations[0].x = -15;
            assert_eq!(replace_snapshot(&db, 7, changed).await?.merged_rows, 1);
            let pool = db.get_postgres_connection_pool();
            let base_count = |x: i32| {
                sqlx::query_scalar::<_, i32>("SELECT node_count FROM atlas_resource_cluster
                    WHERE region=7 AND level=0 AND resource_id=12 AND bucket_x=$1 AND bucket_z=8")
                    .bind(x).fetch_optional(pool)
            };
            assert_eq!(base_count(6).await?, Some(COPY_ROWS as i32));
            flush(&db, 7, Changes {
                resources: HashMap::from([(100, None)]),
                 locations: HashMap::from([(100, None), (101, Some((48, 400))), (102, Some((301, 401)))]),
            }).await?;
            assert_eq!(base_count(6).await?, Some(COPY_ROWS as i32 - 2));
             assert_eq!(base_count(1).await?, Some(1));
             assert_eq!(sqlx::query_as::<_, (i64, i64)>("SELECT sum_x2, sum_z FROM atlas_resource_cluster
                 WHERE region=7 AND level=0 AND resource_id=12 AND bucket_x=6 AND bucket_z=8")
                 .fetch_one(pool).await?, ((COPY_ROWS as i64 - 2) * 600 + 3, (COPY_ROWS as i64 - 2) * 400 + 1));
            assert_eq!(sqlx::query_scalar::<_, i32>("SELECT node_count FROM atlas_resource_cluster
                 WHERE region=7 AND level=8 AND resource_id=12").fetch_one(pool).await?, COPY_ROWS as i32 - 1);
            {
                use axum::body::{Body, to_bytes};
                use axum::http::{Request, StatusCode};
                use tower::ServiceExt;

                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                let response = super::super::resources::routes()
                    .with_state(crate::AppState::new(db.clone(), tx, Default::default()))
                    .oneshot(Request::builder()
                        .uri("/world-map/resource-clusters?minX=6&maxX=6&minZ=8&maxZ=8&level=0&regions=7,8&ids=12")
                        .body(Body::empty())?)
                    .await?;
                assert_eq!(response.status(), StatusCode::OK);
                let json: serde_json::Value =
                    serde_json::from_slice(&to_bytes(response.into_body(), 4096).await?)?;
                let n = COPY_ROWS - 2;
                assert_eq!(json["partial"], true);
                assert_eq!(json["clusters"][0][2], n);
                let row = &json["clusters"][0];
                assert!((row[4].as_f64().unwrap() - (600.0 + 3.0 / n as f64)).abs() < 1e-9);
                assert!((row[5].as_f64().unwrap() - (400.0 + 1.0 / n as f64)).abs() < 1e-9);
            }
            assert_eq!(sqlx::query_scalar::<_, i32>("SELECT x FROM atlas_resource_entry WHERE region=7 AND entity_id=$1")
                .bind(100 + COPY_ROWS as i64).fetch_one(pool).await?, -15);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM atlas_resource_entry WHERE region=7")
                .fetch_one(pool).await?, COPY_ROWS as i64);
            assert_eq!(replace_snapshot(&db, 7, Snapshot::default()).await?.merged_rows, 0);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM atlas_resource_entry WHERE region=7")
                .fetch_one(pool).await?, 0);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM atlas_resource_entry WHERE region=8")
                .fetch_one(pool).await?, 1);
             assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM atlas_resource_node WHERE region=8")
                 .fetch_one(pool).await?, 1);
            {
                use axum::{body::{Body, to_bytes}, http::{Request, StatusCode}};
                use tower::ServiceExt;

                db.execute_unprepared(
                    "INSERT INTO atlas_resource_entry
                     SELECT 7, x*100+z, 12, x, z FROM generate_series(1, 100) x
                     CROSS JOIN generate_series(1, 40) z;
                     INSERT INTO atlas_resource_entry VALUES (7, 20000, 12, 450, 20);",
                ).await?;
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                let app = super::super::resources::routes()
                    .with_state(crate::AppState::new(db.clone(), tx, Default::default()));
                for (bounds, truncated, count) in [
                    ("minX=0&maxX=500&minZ=0&maxZ=50", true, 3_000),
                    ("minX=440&maxX=460&minZ=10&maxZ=30", false, 1),
                ] {
                    let response = app.clone().oneshot(Request::builder()
                        .uri(format!("/world-map/resources?{bounds}&centerX=450&centerZ=20&regions=7&ids=12"))
                        .body(Body::empty())?)
                        .await?;
                    assert_eq!(response.status(), StatusCode::OK);
                    let json: serde_json::Value =
                        serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await?)?;
                    assert_eq!(json["truncated"], truncated);
                    assert_eq!(json["nodes"].as_array().unwrap().len(), count);
                    assert_eq!(json["nodes"][0], serde_json::json!([450, 20, 12]));
                }
            }
            Ok::<_, anyhow::Error>(())
        }.await;
        admin
            .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
            .await?;
        result
    }

    #[tokio::test]
    async fn batches_spawn_move_and_despawn_projections_in_one_transaction() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new(),
            ])
            .append_exec_results((0..5).map(|_| MockExecResult::default()))
            .into_connection();
        flush(
            &db,
            7,
            Changes {
                resources: HashMap::from([(100, Some(12)), (101, None)]),
                locations: HashMap::from([(100, Some((2, 3))), (101, None), (102, Some((5, 6)))]),
            },
        )
        .await
        .unwrap();
        let log = format!("{:?}", db.into_transaction_log());
        assert!(
            log.contains("UPDATE SET resource_id = EXCLUDED.resource_id"),
            "{log}"
        );
        assert!(
            log.contains("UPDATE SET x = EXCLUDED.x, z = EXCLUDED.z"),
            "{log}"
        );
        assert!(
            log.contains("AND resource_id IS NULL AND x IS NULL"),
            "{log}"
        );
        assert!(log.contains("COMMIT"), "{log}");
    }
}
