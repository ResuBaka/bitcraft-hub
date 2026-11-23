use super::*;
use crate::claims::claim_location::ClaimAtLocation;
use entity::experience_state::{Column, Entity, Model};
use sea_orm::{ConnectOptions, Database, DbErr, IntoActiveModel};
use std::{collections::BTreeMap, io::BufRead, time::Instant};

type Event = (
    chrono::DateTime<chrono::Utc>,
    i64,
    i32,
    i32,
    i32,
    i16,
    String,
    Option<i64>,
);

#[derive(serde::Deserialize)]
struct Batch {
    state: Vec<Model>,
    history: Vec<Event>,
}

fn observe(
    (recorded_at, player_id, skill_id, previous_experience, experience, region, status, claim): Event,
) -> ObservedClaimExperience {
    let location = match (status.as_str(), claim) {
        ("claim", Some(id)) => ClaimAtLocation::Claim(id as u64),
        ("unclaimed", None) => ClaimAtLocation::Unclaimed,
        ("unknown", None) => ClaimAtLocation::Unknown,
        _ => panic!("invalid fixture location"),
    };
    ObservedClaimExperience {
        recorded_at,
        player_id,
        skill_id,
        previous_experience,
        experience,
        region,
        location,
    }
}

async fn schema() -> (DatabaseConnection, String) {
    let name = format!("xp_write_{}", rand::random::<u64>());
    let mut opts = ConnectOptions::new(std::env::var("XP_WRITE_TEST_URL").unwrap());
    opts.max_connections(1)
        .set_schema_search_path(format!("{name},public"));
    let db = Database::connect(opts).await.unwrap();
    let actual = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS db",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "db")
        .unwrap();
    assert_eq!(
        actual, "xp_replay_scratch",
        "requires a disposable database"
    );
    db.execute_unprepared(&format!("CREATE SCHEMA {name}"))
        .await
        .unwrap();
    db.execute_unprepared(include_str!("schema.sql"))
        .await
        .unwrap();
    // Same four continuous aggregate definitions; no background refresh/retention jobs.
    for (suffix, bucket) in [("30m", "30 minutes"), ("weekly", "1 week")] {
        for loc in [false, true] {
            let (prefix, extra, gain, filter, groups) = if loc {
                (
                    "location_",
                    ",region,location_status,claim_id",
                    "COALESCE(attributed_experience,experience::bigint-previous_experience::bigint)",
                    if suffix == "weekly" {
                        "location_status IS NOT NULL AND NOT legacy_claim_week_archived AND (attributed_experience IS NOT NULL OR previous_experience IS NOT NULL)"
                    } else {
                        "location_status IS NOT NULL AND (attributed_experience IS NOT NULL OR previous_experience IS NOT NULL)"
                    },
                    "1,2,3,4,5,6",
                )
            } else {
                (
                    "",
                    "",
                    "experience::bigint-previous_experience::bigint",
                    "previous_experience IS NOT NULL",
                    "1,2,3",
                )
            };
            db.execute_unprepared(&format!("CREATE MATERIALIZED VIEW xp_{prefix}{suffix}
                WITH (timescaledb.continuous, timescaledb.materialized_only=false) AS SELECT
                time_bucket(interval '{bucket}', recorded_at) AS bucket,player_id,skill_id{extra},sum({gain})::bigint AS gained
                FROM player_experience_history WHERE {filter} GROUP BY {groups} WITH NO DATA")).await.unwrap();
        }
    }
    (db, name)
}

async fn cleanup(db: &DatabaseConnection, name: &str) {
    db.execute_unprepared(&format!("DROP SCHEMA {name} CASCADE"))
        .await
        .unwrap();
}

// Former writer: active models + ORM upsert (including RETURNING), then
// generated VALUES placeholders and eight scalar parameters per event.
async fn values(
    db: &DatabaseConnection,
    conflict: &sea_query::OnConflict,
    rows: Vec<Model>,
    history: Vec<ObservedClaimExperience>,
) -> Result<(), DbErr> {
    let active: Vec<_> = rows
        .into_iter()
        .map(IntoActiveModel::into_active_model)
        .collect();
    let txn = db.begin().await?;
    if !active.is_empty() {
        Entity::insert_many(active)
            .on_conflict(conflict.clone())
            .exec(&txn)
            .await?;
    }
    for chunk in history.chunks(500) {
        let mut binds = Vec::<sea_orm::Value>::with_capacity(chunk.len() * 8);
        let mut rows = Vec::with_capacity(chunk.len());
        for e in chunk {
            let (status, claim) = match e.location {
                ClaimAtLocation::Claim(id) => ("claim", Some(id as i64)),
                ClaimAtLocation::Unclaimed => ("unclaimed", None),
                ClaimAtLocation::Unknown => ("unknown", None),
            };
            let n = binds.len();
            rows.push(format!(
                "(${},${},${},${},${},${},${},${})",
                n + 1,
                n + 2,
                n + 3,
                n + 4,
                n + 5,
                n + 6,
                n + 7,
                n + 8
            ));
            binds.extend([
                e.recorded_at.into(),
                e.player_id.into(),
                e.skill_id.into(),
                e.previous_experience.into(),
                e.experience.into(),
                e.region.into(),
                status.into(),
                claim.into(),
            ]);
        }
        let sql = format!(
            "INSERT INTO player_experience_history (recorded_at, player_id, skill_id, previous_experience, experience, region, location_status, claim_id) VALUES {}",
            rows.join(",")
        );
        txn.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            binds,
        ))
        .await?;
    }
    txn.commit().await
}

async fn history(db: &DatabaseConnection) -> Vec<Event> {
    let mut rows: Vec<Event> = db.query_all_raw(Statement::from_string(DbBackend::Postgres,
        "SELECT recorded_at,player_id,skill_id,previous_experience,experience,region,location_status,claim_id
         FROM player_experience_history WHERE location_status IS NOT NULL")).await.unwrap().into_iter().map(|r|
        (r.try_get_by_index(0).unwrap(),r.try_get_by_index(1).unwrap(),r.try_get_by_index(2).unwrap(),r.try_get_by_index(3).unwrap(),
         r.try_get_by_index(4).unwrap(),r.try_get_by_index(5).unwrap(),r.try_get_by_index(6).unwrap(),r.try_get_by_index(7).unwrap())).collect();
    rows.sort_unstable();
    rows
}

async fn trigger_count(db: &DatabaseConnection) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT count(*) FROM player_experience_history WHERE location_status IS NULL",
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index(0)
    .unwrap()
}

#[tokio::test]
#[ignore = "requires a disposable TimescaleDB database"]
async fn live_xp_write_rolls_back_state_trigger_and_events() {
    let (db, name) = schema().await;
    db.execute_unprepared("ALTER TABLE player_experience_history ADD CHECK (experience >= 0)")
        .await
        .unwrap();
    let old = Model {
        entity_id: 1,
        skill_id: 2,
        experience: 3,
        region: 4,
    };
    let event = |id, xp, loc| ObservedClaimExperience {
        recorded_at: chrono::Utc::now(),
        player_id: id,
        skill_id: 2,
        previous_experience: 0,
        experience: xp,
        region: 4,
        location: loc,
    };
    let events = vec![
        event(1, 1, ClaimAtLocation::Claim(0)),
        event(1, 2, ClaimAtLocation::Unknown),
        event(1, 3, ClaimAtLocation::Unclaimed),
    ];
    insert_multiple_experience_state(&db, vec![old.clone()], events)
        .await
        .unwrap();
    let before = history(&db).await;
    assert_eq!(before.len(), 3);
    assert!(before.iter().any(|e| e.6 == "claim" && e.7 == Some(0)));
    let changed = Model {
        experience: 4,
        region: 8,
        ..old.clone()
    };
    let inserted = Model {
        entity_id: 5,
        ..changed.clone()
    };
    let result = insert_multiple_experience_state(
        &db,
        vec![changed, inserted],
        vec![
            event(1, 4, ClaimAtLocation::Unclaimed),
            event(5, -1, ClaimAtLocation::Unknown),
        ],
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("check constraint"));
    assert_eq!(Entity::find().all(&db).await.unwrap(), [old]);
    assert_eq!(history(&db).await, before);
    assert_eq!(trigger_count(&db).await, 1);
    cleanup(&db, &name).await;
}

fn thread_ns() -> u64 {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

#[tokio::test]
#[ignore = "private live XP fixture and disposable TimescaleDB required"]
async fn compare_live_xp_writes() {
    let file = std::fs::File::open(std::env::var("XP_WRITE_FIXTURE").unwrap()).unwrap();
    let batches: Vec<Batch> = std::io::BufReader::new(file)
        .lines()
        .map(|s| serde_json::from_str(&s.unwrap()).unwrap())
        .collect();
    let mut seed = BTreeMap::new();
    let mut final_state = BTreeMap::new();
    let mut expected = Vec::new();
    let mut input = Vec::new();
    for batch in batches {
        let mut latest = BTreeMap::new();
        for e in &batch.history {
            seed.entry((e.1, e.2)).or_insert_with(|| Model {
                entity_id: e.1,
                skill_id: e.2,
                experience: e.3,
                region: e.5,
            });
            latest.insert((e.1, e.2), e.4);
        }
        assert_eq!(latest.len(), batch.state.len());
        for r in &batch.state {
            assert_eq!(
                latest.remove(&(r.entity_id, r.skill_id)),
                Some(r.experience)
            );
            final_state.insert((r.entity_id, r.skill_id), r.clone());
        }
        expected.extend(batch.history.iter().cloned());
        input.push((
            batch.state,
            batch.history.into_iter().map(observe).collect::<Vec<_>>(),
        ));
    }
    expected.sort_unstable();
    let sizes: Vec<_> = input.iter().map(|(s, _)| s.len()).collect();
    let conflicts = sizes.iter().sum::<usize>();
    println!(
        "\nmix {}",
        serde_json::json!({"sizes":sizes,"conflicts":conflicts,"inserts":0,"seed":seed.len(),
        "history_sizes":input.iter().map(|(_,h)| h.len()).collect::<Vec<_>>() })
    );
    let seed: Vec<_> = seed.into_values().collect();
    let conflict = sea_query::OnConflict::columns([Column::EntityId, Column::SkillId])
        .update_columns([Column::Experience, Column::Region])
        .to_owned();
    for block in 0..=10 {
        for fixed in [block % 2 != 0, block % 2 == 0] {
            let (db, name) = schema().await;
            db.execute_unprepared(
                "ALTER TABLE experience_state DISABLE TRIGGER experience_history_record",
            )
            .await
            .unwrap();
            db.execute_raw(snapshot_write::experience(&seed))
                .await
                .unwrap();
            db.execute_unprepared("ALTER TABLE experience_state ENABLE TRIGGER experience_history_record; ANALYZE experience_state").await.unwrap();
            let mut times = Vec::new();
            let mut cpu = 0;
            for (rows, events) in &input {
                let (rows, events) = (rows.clone(), events.clone());
                let t = thread_ns();
                let start = Instant::now();
                if fixed {
                    insert_multiple_experience_state(&db, rows, events)
                        .await
                        .unwrap();
                } else {
                    values(&db, &conflict, rows, events).await.unwrap();
                }
                times.push(start.elapsed().as_nanos() as u64);
                cpu += thread_ns() - t;
            }
            let stored = Entity::find().all(&db).await.unwrap();
            assert_eq!(stored.len(), final_state.len());
            for r in &stored {
                assert_eq!(r, &final_state[&(r.entity_id, r.skill_id)]);
            }
            assert_eq!(trigger_count(&db).await, conflicts as i64);
            assert!(
                history(&db).await == expected,
                "history differs from captured multiset"
            );
            cleanup(&db, &name).await;
            if block != 0 {
                println!(
                    "sample {}",
                    serde_json::json!({"writer":if fixed {"unnest"} else {"values"}, "block":block, "cpu_ns":cpu, "ns":times})
                );
            }
        }
    }
}
