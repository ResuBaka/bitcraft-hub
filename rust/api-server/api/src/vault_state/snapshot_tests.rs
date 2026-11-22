use super::*;
use futures::stream;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DbBackend, MockDatabase, QueryOrder, Statement,
};
use std::collections::BTreeMap;

fn vault(entity_id: u64, ids: impl IntoIterator<Item = i32>) -> VaultState {
    VaultState {
        entity_id,
        collectibles: ids
            .into_iter()
            .map(|id| VaultCollectible {
                id,
                activated: true,
                count: 3,
            })
            .collect(),
    }
}

#[tokio::test]
async fn incomplete_snapshot_or_failed_upsert_never_removes_stale_keys() {
    for interrupted in [true, false] {
        let key = |id: i64| BTreeMap::from([("0".to_owned(), sea_orm::Value::from(id))]);
        let existing = |id: i32| {
            BTreeMap::from([
                ("0".to_owned(), 1i64.into()),
                ("1".to_owned(), id.into()),
                ("2".to_owned(), true.into()),
                ("3".to_owned(), 3i32.into()),
            ])
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key(1), key(999)]])
            .append_query_results([vec![existing(7), existing(9)]]);
        // One full batch (including stale key (1,9)), then one more entity.
        let mut input: Vec<_> = (1..=ENTITIES as u64 + 1)
            .map(|id| Ok(vault(id, [7])))
            .collect();
        let db = if interrupted {
            db.append_exec_results([sea_orm::MockExecResult {
                last_insert_id: 0,
                rows_affected: 127,
            }])
        } else {
            db.append_exec_errors([DbErr::Custom("upsert failed".into())])
        }
        .into_connection();
        if interrupted {
            input.push(Err(DbErr::Custom("incomplete".into())));
        }
        let result = reconcile(&db, 7, 3000, stream::iter(input)).await;
        assert!(result.is_err());
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("INSERT INTO"));
        assert!(!log.contains("DELETE FROM"));
    }
}

async fn temp_db() -> DatabaseConnection {
    let mut opt = ConnectOptions::new(std::env::var("VAULT_TEST_DATABASE_URL").unwrap());
    // All statements (including the production reconciler's) resolve only
    // session-local tables; a reconnect cannot fall through to a real table.
    opt.max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .set_schema_search_path("pg_temp");
    let db = Database::connect(opt).await.unwrap();
    exec(&db,"CREATE TEMP TABLE vault_state_collectibles (entity_id bigint NOT NULL, id integer NOT NULL, activated boolean NOT NULL, count integer NOT NULL, region smallint NOT NULL, PRIMARY KEY(entity_id,id))").await;
    exec(&db, "CREATE INDEX ON vault_state_collectibles (region)").await;
    db
}
async fn exec(db: &DatabaseConnection, sql: &str) {
    db.execute_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires VAULT_TEST_DATABASE_URL; only uses temporary tables"]
async fn postgres_entity_snapshot_handles_moves_empty_removed_and_oversized_vaults() {
    let db = temp_db().await;
    exec(
        &db,
        "INSERT INTO vault_state_collectibles VALUES
        (1,8,false,1,7),(1,9,true,3,7),(1,10,true,3,7),(1,42,true,3,8),
        (2,1,true,3,7),(3,1,true,3,7),(4,5,false,1,8),
        (5,-200,true,3,7),(5,1,true,3,7),(5,65537,true,3,7),(5,100000,true,3,7)",
    )
    .await;
    let n = COLLECTIBLES as i32 + 2;
    let input = [
        vault(4, [5]),
        vault(2, []),
        vault(5, (0..n).rev().map(|n| n * 2)),
        vault(1, [9, 8]),
    ];
    reconcile(
        &db,
        7,
        3000,
        stream::iter(input.into_iter().map(Ok)),
    )
    .await
    .unwrap();
    let got = E::find()
        .order_by_asc(C::EntityId)
        .order_by_asc(C::Id)
        .all(&db)
        .await
        .unwrap();
    let mut expected = vec![(1, 8, 7), (1, 9, 7), (1, 42, 8), (4, 5, 7)];
    expected.extend((0..n).map(|n| (5, n * 2, 7)));
    assert!(
        got.iter()
            .map(|m| (m.entity_id, m.id, m.region))
            .eq(expected)
    );
    assert!(got.iter().all(|m| m.activated && m.count == 3));
    // A complete empty snapshot deletes only this region.
    reconcile(&db, 7, 3000, stream::empty())
        .await
        .unwrap();
    let rest = E::find().all(&db).await.unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].region, 8);
}
