use super::*;
use entity::inventory::{Column, Entity};
use sea_orm::{ColumnTrait, ConnectOptions, Database, EntityTrait, IntoActiveModel, QueryFilter};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

#[derive(Deserialize)]
struct Capture {
    seed: Vec<Model>,
    steps: Vec<Flush>,
}

#[derive(Deserialize)]
struct Flush {
    rows: Vec<Model>,
    inserted: Vec<i64>,
}

fn cpu_ns() -> u64 {
    use rustix::time::{ClockId, clock_gettime};
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

#[tokio::test]
#[ignore = "private WAL-validated live flushes; requires disposable PostgreSQL"]
async fn compare_live_inventory_writers() {
    let capture: Capture = serde_json::from_reader(std::io::BufReader::new(
        std::fs::File::open(std::env::var("INVENTORY_LIVE_FIXTURE").unwrap()).unwrap(),
    ))
    .unwrap();
    let cap = crate::config::DatabaseConfig::default().inventory_snapshot_write_bytes;
    // Baseline: former live insert_many path, updating all non-key columns.
    let conflict = sea_orm::sea_query::OnConflict::column(Column::EntityId)
        .update_columns([
            Column::Pockets,
            Column::InventoryIndex,
            Column::CargoIndex,
            Column::OwnerEntityId,
            Column::PlayerOwnerEntityId,
            Column::Region,
        ])
        .to_owned();
    let mut expected: BTreeMap<_, _> = capture.seed.iter().map(|r| (r.entity_id, r)).collect();
    let mut deletes = Vec::new();
    let mut splits = 0;
    for batch in &capture.steps {
        let inserted: HashSet<_> = batch.inserted.iter().copied().collect();
        let mut remove = Vec::new();
        assert_eq!(
            batch
                .rows
                .iter()
                .map(|r| r.entity_id)
                .collect::<HashSet<_>>()
                .len(),
            batch.rows.len()
        );
        for row in &batch.rows {
            let exists = expected.insert(row.entity_id, row).is_some();
            if inserted.contains(&row.entity_id) {
                if exists {
                    remove.push(row.entity_id);
                }
            } else {
                assert!(exists, "missing pre-image for a captured conflict");
            }
        }
        let mut remaining = &batch.rows[..];
        while !remaining.is_empty() {
            remaining = &remaining[next_write(remaining, cap).0..];
            splits += usize::from(!remaining.is_empty());
        }
        deletes.push(remove);
    }
    let sizes: Vec<_> = capture.steps.iter().map(|s| s.rows.len()).collect();
    let inserts: usize = capture.steps.iter().map(|s| s.inserted.len()).sum();
    println!(
        "\nmix {}",
        serde_json::json!({"seed":capture.seed.len(), "sizes":sizes,
        "inserts":inserts, "conflicts":sizes.iter().sum::<usize>()-inserts,
        "extra_unnest_statements":splits, "byte_cap":cap.get()})
    );
    let schema = format!("inventory_live_{}", rand::random::<u64>());
    let mut opts = ConnectOptions::new(std::env::var("INVENTORY_LIVE_TEST_URL").unwrap());
    opts.max_connections(1).set_schema_search_path(&schema);
    let db = Database::connect(opts).await.unwrap();
    db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE inventory (entity_id bigint PRIMARY KEY, pockets jsonb NOT NULL,
        inventory_index integer NOT NULL, cargo_index integer NOT NULL, owner_entity_id bigint NOT NULL,
        player_owner_entity_id bigint NOT NULL, region smallint NOT NULL);
        CREATE INDEX inv_owner_1 ON inventory(owner_entity_id);
        CREATE INDEX inv_owner_2 ON inventory(owner_entity_id);
        CREATE INDEX inv_player_owner ON inventory(player_owner_entity_id);
        CREATE INDEX inv_region ON inventory(region)").await.unwrap();
    for block in 0..=10 {
        for fixed in [block % 2 != 0, block % 2 == 0] {
            db.execute_unprepared("TRUNCATE inventory").await.unwrap();
            write_changes(&db, &mut capture.seed.clone(), cap)
                .await
                .unwrap();
            db.execute_unprepared("ANALYZE inventory").await.unwrap();
            let mut times = Vec::new();
            let mut cpu = 0;
            for (batch, remove) in capture.steps.iter().zip(&deletes) {
                if !remove.is_empty() {
                    let result = Entity::delete_many()
                        .filter(Column::EntityId.is_in(remove.clone()))
                        .exec(&db)
                        .await
                        .unwrap();
                    assert_eq!(result.rows_affected, remove.len() as u64);
                }
                let rows = batch.rows.clone(); // same owned models + WAL order for both
                let t = cpu_ns();
                let start = Instant::now();
                if fixed {
                    write_live(&db, rows, cap).await.unwrap();
                } else {
                    let active: Vec<_> = rows
                        .into_iter()
                        .map(IntoActiveModel::into_active_model)
                        .collect();
                    Entity::insert_many(active)
                        .on_conflict(conflict.clone())
                        .exec(&db)
                        .await
                        .unwrap();
                }
                let ns = start.elapsed().as_nanos() as u64;
                cpu += cpu_ns() - t;
                times.push(ns);
                if block == 0 {
                    let stored = Entity::find()
                        .filter(Column::EntityId.is_in(batch.rows.iter().map(|r| r.entity_id)))
                        .all(&db)
                        .await
                        .unwrap();
                    let stored: BTreeMap<_, _> =
                        stored.into_iter().map(|r| (r.entity_id, r)).collect();
                    assert_eq!(stored.len(), batch.rows.len());
                    for row in &batch.rows {
                        assert_eq!(stored.get(&row.entity_id), Some(row));
                    }
                }
            }
            let stored = Entity::find().all(&db).await.unwrap();
            assert_eq!(stored.len(), expected.len());
            for r in &stored {
                assert_eq!(expected[&r.entity_id], r);
            }
            if block != 0 {
                println!(
                    "sample {}",
                    serde_json::json!({"block": block, "writer": if fixed {"unnest"} else {"values"},
                    "ns":times,"cpu_ns":cpu})
                );
            }
        }
    }
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
