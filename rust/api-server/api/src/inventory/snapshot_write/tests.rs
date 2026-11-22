use super::*;
use entity::inventory::{Column, Entity, ItemStack, ItemType, Pocket};
use sea_orm::{ConnectOptions, Database, DbBackend, EntityTrait, QueryOrder, Statement};
use std::io::{BufRead, BufReader};

fn row(id: i64, pockets: Vec<Pocket>) -> Model {
    Model {
        entity_id: id,
        pockets,
        inventory_index: 1,
        cargo_index: 2,
        owner_entity_id: 3,
        player_owner_entity_id: 4,
        region: 5,
    }
}

#[test]
fn partial_writes_respect_the_byte_boundary_and_keep_oversize_rows() {
    let full = Pocket {
        volume: i32::MIN,
        locked: false,
        contents: Some(ItemStack {
            item_id: i32::MIN,
            quantity: i32::MIN,
            durability: Some(i32::MIN),
            item_type: ItemType::Cargo,
        }),
    };
    let empty = Pocket {
        contents: None,
        ..full.clone()
    };
    let rows = [
        row(1, vec![empty]),
        row(2, vec![full.clone()]),
        row(3, vec![full; 100]),
        row(4, vec![]),
    ];
    for row in &rows {
        assert!(serde_json::to_vec(&row.pockets).unwrap().len() + 64 <= estimated_bytes(row));
    }
    let exact = estimated_bytes(&rows[0]) + estimated_bytes(&rows[1]);
    let cap = NonZeroUsize::new(exact).unwrap();
    assert_eq!(next_write(&rows, cap), (2, exact));
    assert_eq!(
        next_write(&rows, NonZeroUsize::new(exact - 1).unwrap()).0,
        1
    );
    assert_eq!(next_write(&rows[2..], cap), (1, estimated_bytes(&rows[2])));
    assert_eq!(next_write(&rows[3..], cap).0, 1);
}

/// Writer-only replay. Use an isolated Postgres instance for comparable WAL
/// deltas; the test creates a private schema and uses ordinary logged tables.
#[tokio::test]
#[ignore = "requires INVENTORY_WRITER_TEST_URL and INVENTORY_WRITER_FIXTURE (JSONL, sorted by entity_id)"]
async fn replay_snapshot_writes_on_postgres() {
    let schema = format!("inventory_bench_{}", rand::random::<u64>());
    let mut opts = ConnectOptions::new(std::env::var("INVENTORY_WRITER_TEST_URL").unwrap());
    opts.max_connections(1).set_schema_search_path(&schema);
    let db = Database::connect(opts).await.unwrap();
    db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE inventory (entity_id bigint PRIMARY KEY, pockets jsonb NOT NULL, inventory_index integer NOT NULL, cargo_index integer NOT NULL, owner_entity_id bigint NOT NULL, player_owner_entity_id bigint NOT NULL, region smallint NOT NULL)").await.unwrap();
    let cap = std::env::var("INVENTORY_WRITER_TEST_BYTES").map_or_else(
        |_| crate::config::DatabaseConfig::default().inventory_snapshot_write_bytes,
        |s| s.parse().unwrap(),
    );
    let fixture = std::env::var("INVENTORY_WRITER_FIXTURE").unwrap();
    let open = || {
        BufReader::new(std::fs::File::open(&fixture).unwrap())
            .lines()
            .map(|l| serde_json::from_str::<Model>(&l.unwrap()).unwrap())
    };
    let mutate = |r: &mut Model| {
        r.inventory_index ^= 1;
        r.cargo_index ^= 1;
        r.owner_entity_id ^= 1;
        r.player_owner_entity_id ^= 1;
        r.region ^= 1;
        if let Some(p) = r.pockets.first_mut() {
            p.locked = !p.locked;
        }
    };
    let wal_position = || async {
        db.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(), '0/0')::bigint",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap()
    };
    for update in [false, true] {
        let mut input = open();
        let mut elapsed = std::time::Duration::ZERO;
        let mut written = 0;
        let wal = wal_position().await;
        loop {
            let mut rows: Vec<_> = input.by_ref().take(3_000).collect();
            if rows.is_empty() {
                break;
            }
            if update {
                rows.iter_mut().for_each(mutate);
            }
            written += rows.len();
            let start = std::time::Instant::now();
            write_changes(&db, &mut rows, cap).await.unwrap();
            elapsed += start.elapsed();
        }
        assert!(written > 0);
        eprintln!(
            "inventory_writer update={update} rows={written} write_ms={} wal_bytes={}",
            elapsed.as_millis(),
            wal_position().await - wal
        );
    }
    // Stream the durable rows, checking JSON payloads and every updated column.
    use futures::TryStreamExt;
    let mut stored = Entity::find()
        .order_by_asc(Column::EntityId)
        .into_model::<crate::inventory::db_row::InventoryDbRow>()
        .stream(&db)
        .await
        .unwrap();
    for mut expected in open() {
        mutate(&mut expected);
        assert_eq!(stored.try_next().await.unwrap().unwrap().0, expected);
    }
    assert!(stored.try_next().await.unwrap().is_none());
    drop(stored);
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
