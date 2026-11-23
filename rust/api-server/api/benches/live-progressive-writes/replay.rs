use super::*;
use entity::progressive_action_state::{Column, Entity, Model};
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use serde::Deserialize;
use std::time::Instant;

#[derive(Deserialize)]
struct Capture {
    seed: Vec<Model>,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Step {
    Upsert(Vec<Model>),
    Delete(Vec<i64>),
}

fn client_cpu_ns() -> u64 {
    use rustix::time::{ClockId, clock_gettime};
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

#[tokio::test]
#[ignore = "requires LIVE_WRITER_CAPTURE and LIVE_WRITER_TEST_URL (disposable PostgreSQL)"]
async fn compare_progressive_writers() {
    let capture: Capture = serde_json::from_reader(
        std::fs::File::open(std::env::var("LIVE_WRITER_CAPTURE").unwrap()).unwrap(),
    )
    .unwrap();
    let schema = format!("live_writer_{}", rand::random::<u64>());
    let mut opts = ConnectOptions::new(std::env::var("LIVE_WRITER_TEST_URL").unwrap());
    opts.max_connections(1).set_schema_search_path(&schema);
    let db = Database::connect(opts).await.unwrap();
    db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE progressive_action_state (
        entity_id bigint PRIMARY KEY, building_entity_id bigint NOT NULL, function_type integer NOT NULL,
        progress integer NOT NULL, recipe_id integer NOT NULL, craft_count integer NOT NULL,
        last_crit_outcome integer NOT NULL, owner_entity_id bigint NOT NULL, lock_expiration jsonb NOT NULL,
        preparation boolean NOT NULL, region smallint NOT NULL);
        CREATE INDEX ON progressive_action_state(building_entity_id);
        CREATE INDEX ON progressive_action_state(owner_entity_id)").await.unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(db.clone(), tx, Default::default());
    let conflict = live_upsert_conflict();
    let mut expected: HashMap<_, _> = capture.seed.iter().map(|r| (r.entity_id, r)).collect();
    let mut inserts = 0;
    let mut updates = 0;
    for step in &capture.steps {
        match step {
            Step::Upsert(batch) => {
                for r in batch {
                    if expected.insert(r.entity_id, r).is_some() {
                        updates += 1;
                    } else {
                        inserts += 1;
                    }
                }
            }
            Step::Delete(ids) => {
                for id in ids {
                    assert!(expected.remove(id).is_some());
                }
            }
        }
    }
    let sizes: Vec<_> = capture
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::Upsert(rows) => Some(rows.len()),
            Step::Delete(_) => None,
        })
        .collect();
    println!(
        "\nmix {}",
        serde_json::json!({
            "seed": capture.seed.len(), "inserts": inserts, "conflicts": updates, "sizes": sizes
        })
    );
    let hasher = std::hash::RandomState::new();
    for block in 0..=12 {
        for fixed in [block % 2 != 0, block % 2 == 0] {
            db.execute_unprepared("TRUNCATE progressive_action_state")
                .await
                .unwrap();
            for batch in capture.seed.chunks(1000) {
                db.execute_raw(snapshot_write::progressive_actions(batch))
                    .await
                    .unwrap();
            }
            db.execute_unprepared("ANALYZE progressive_action_state")
                .await
                .unwrap();
            let mut pending = PendingActions {
                upserts: HashMap::with_hasher(hasher.clone()),
                ..Default::default()
            };
            let mut times = Vec::new();
            let mut cpu = 0;
            for step in &capture.steps {
                match step {
                    Step::Upsert(batch) => {
                        for r in batch {
                            pending.upsert(r.clone());
                        }
                        assert_eq!(pending.upserts.len(), batch.len());
                        let cpu_start = client_cpu_ns();
                        let start = Instant::now();
                        let writer = if fixed {
                            Writer::Fixed(snapshot_write::progressive_actions)
                        } else {
                            Writer::Orm(&conflict)
                        };
                        assert!(pending.flush_with_writer(&state, writer).await);
                        let ns = start.elapsed().as_nanos() as u64;
                        cpu += client_cpu_ns() - cpu_start;
                        times.push(ns);
                        if block == 0 {
                            let rows = Entity::find()
                                .filter(Column::EntityId.is_in(batch.iter().map(|r| r.entity_id)))
                                .all(&db)
                                .await
                                .unwrap();
                            let by_id: HashMap<_, _> =
                                rows.iter().map(|r| (r.entity_id, r)).collect();
                            for r in batch {
                                assert_eq!(by_id.get(&r.entity_id).copied(), Some(r));
                            }
                        }
                    }
                    Step::Delete(ids) => {
                        let n = Entity::delete_many()
                            .filter(Column::EntityId.is_in(ids.clone()))
                            .exec(&db)
                            .await
                            .unwrap()
                            .rows_affected;
                        assert_eq!(n, ids.len() as u64);
                    }
                }
            }
            let stored = Entity::find().all(&db).await.unwrap();
            assert_eq!(stored.len(), expected.len());
            for r in stored {
                assert_eq!(expected.get(&r.entity_id), Some(&&r));
            }
            assert!(!times.is_empty());
            if block != 0 {
                let writer = if fixed { "unnest" } else { "values" };
                println!(
                    "sample {}",
                    serde_json::json!({
                        "block": block, "writer": writer, "ns": times, "cpu_ns": cpu
                    })
                );
            }
        }
    }
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
