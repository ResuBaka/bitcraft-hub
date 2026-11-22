//! PostgreSQL correctness and writer-only benchmark. Run each table in its
//! own process under Heaptrack. The baseline uses its previous SeaORM writer.
use super::*;
use entity::{passive_craft_state as craft, vault_state_collectibles as vault};
use futures::TryStreamExt;
use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, Iterable, PrimaryKeyToColumn, QueryOrder,
};
use std::time::{Duration, Instant};

const ROWS: i32 = 12_019;

#[derive(Clone, Copy, PartialEq)]
enum Pass {
    Insert,
    Update,
    StoredAfterUpdate,
}

async fn wal(db: &DatabaseConnection) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(), '0/0')::bigint",
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index(0)
    .unwrap()
}

async fn replay<E: EntityTrait>(
    table: &str,
    ddl: &str,
    conflict: OnConflict,
    legacy_returning: bool,
    batch_rows: usize,
    model: fn(i32, Pass) -> E::Model,
    writer: Writer<'_, E>,
) where
    E::Model: IntoActiveModel<E::ActiveModel> + PartialEq,
{
    let legacy = std::env::var("SNAPSHOT_WRITER_BASELINE").as_deref() == Ok("1");
    let schema = format!("snapshot_writer_{}", rand::random::<u64>());
    let mut opts = ConnectOptions::new(std::env::var("SNAPSHOT_WRITER_TEST_URL").unwrap());
    opts.max_connections(1).set_schema_search_path(&schema);
    let db = Database::connect(opts).await.unwrap();
    db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    db.execute_unprepared(ddl).await.unwrap();
    for update in [false, true] {
        let start_wal = wal(&db).await;
        let mut elapsed = Duration::ZERO;
        for first in (0..ROWS).step_by(batch_rows) {
            let mut batch: Vec<_> = (first..ROWS.min(first + batch_rows as i32))
                .map(|i| model(i, if update { Pass::Update } else { Pass::Insert }))
                .collect();
            let started = Instant::now();
            if legacy {
                let insert =
                    E::insert_many(batch.into_iter().map(IntoActiveModel::into_active_model))
                        .on_conflict(conflict.clone());
                if legacy_returning {
                    insert.exec(&db).await.unwrap();
                } else {
                    insert.exec_without_returning(&db).await.unwrap();
                }
            } else {
                writer.write(&db, &mut batch).await.unwrap();
            }
            elapsed += started.elapsed();
        }
        eprintln!(
            "snapshot_write table={table} baseline={legacy} update={update} rows={ROWS} write_us={} wal_bytes={}",
            elapsed.as_micros(),
            wal(&db).await - start_wal
        );
        let mut query = E::find();
        for key in E::PrimaryKey::iter() {
            query = query.order_by_asc(key.into_column());
        }
        let mut stored = query.stream(&db).await.unwrap();
        for i in 0..ROWS {
            assert_eq!(
                stored.try_next().await.unwrap().unwrap(),
                model(
                    i,
                    if update {
                        Pass::StoredAfterUpdate
                    } else {
                        Pass::Insert
                    }
                )
            );
        }
        assert!(stored.try_next().await.unwrap().is_none());
        drop(stored);
        // The typed Rust model can round JSON numbers back to f32. Compare
        // this canonical database digest across variants to catch differences
        // an application-level equality check cannot see.
        let digest: String = db.query_one_raw(Statement::from_string(DbBackend::Postgres,
            format!("SELECT md5(string_agg(h, '' ORDER BY h)) FROM (SELECT md5(to_jsonb(r)::text) AS h FROM {table} r) rows")))
            .await.unwrap().unwrap().try_get_by_index(0).unwrap();
        eprintln!("snapshot_digest table={table} update={update} digest={digest}");
    }
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL; uses a private logged schema"]
async fn passive_craft_rows() {
    use craft::Column::*;
    let ddl = "CREATE TABLE passive_craft_state(
        entity_id bigint PRIMARY KEY, owner_entity_id bigint NOT NULL, recipe_id integer NOT NULL,
        building_entity_id bigint NOT NULL, timestamp timestamptz NOT NULL, status smallint NOT NULL,
        slot bigint, region smallint NOT NULL)";
    replay::<craft::Entity>(
        "passive_craft_state",
        ddl,
        OnConflict::column(EntityId)
            .update_columns([
                OwnerEntityId,
                RecipeId,
                BuildingEntityId,
                Timestamp,
                Status,
                Slot,
                Region,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let update = pass != Pass::Insert;
            let v = i + i32::from(update);
            craft::Model {
                entity_id: i.into(),
                owner_entity_id: (1i64 << 40) + i64::from(v),
                recipe_id: v,
                building_entity_id: -(i64::from(v) + 1),
                timestamp: chrono::DateTime::from_timestamp_micros(
                    1_790_000_000_123_456 + i64::from(v),
                )
                .unwrap()
                .into(),
                status: (v % 3) as i16,
                slot: match v % 3 {
                    0 => None,
                    1 => Some(0),
                    _ => Some(i64::from(v)),
                },
                region: if update { 8 } else { 7 },
            }
        },
        Writer::Fixed(passive_crafts),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL; uses a private logged schema"]
async fn vault_collectible_rows() {
    use vault::Column::*;
    let ddl = "CREATE TABLE vault_state_collectibles(entity_id bigint NOT NULL, id integer NOT NULL,
        activated boolean NOT NULL, count integer NOT NULL, region smallint NOT NULL, PRIMARY KEY(entity_id,id))";
    replay::<vault::Entity>(
        "vault_state_collectibles",
        ddl,
        OnConflict::columns([EntityId, Id])
            .update_columns([Activated, Count, Region])
            .to_owned(),
        false,
        3000,
        |i, pass| vault::Model {
            entity_id: (1i64 << 40) + i64::from(i / 48),
            id: i % 48 - 24,
            activated: (i % 2 == 0) ^ (pass != Pass::Insert),
            count: i + i32::from(pass != Pass::Insert),
            region: if pass != Pass::Insert { 8 } else { 7 },
        },
        Writer::Fixed(vault_collectibles),
    )
    .await;
}

mod regional;
