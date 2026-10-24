use entity::inventory::{Model, Pocket};
use sea_orm::{DbErr, FromQueryResult, QueryResult, TryGetableFromJson};
use serde::Deserialize;

// SeaORM's Vec<T: TryGetableFromJson> decoder materializes serde_json::Value
// before decoding each element. A single JSON value containing the entire
// typed vector instead uses SQLx's Json<Self> decoder directly on PostgreSQL.
#[derive(Deserialize)]
#[serde(transparent)]
struct PocketsJson(Vec<Pocket>);

impl TryGetableFromJson for PocketsJson {}

/// Read-only projection for snapshot comparison. SQL, stored JSON, entity write
/// encoding, and the public model all stay unchanged.
pub(super) struct InventoryDbRow(pub Model);

impl FromQueryResult for InventoryDbRow {
    fn from_query_result(row: &QueryResult, pre: &str) -> Result<Self, DbErr> {
        Ok(Self(Model {
            entity_id: row.try_get(pre, "entity_id")?,
            pockets: row.try_get::<PocketsJson>(pre, "pockets")?.0,
            inventory_index: row.try_get(pre, "inventory_index")?,
            cargo_index: row.try_get(pre, "cargo_index")?,
            owner_entity_id: row.try_get(pre, "owner_entity_id")?,
            player_owner_entity_id: row.try_get(pre, "player_owner_entity_id")?,
            region: row.try_get(pre, "region")?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    const FIXTURE: &str = r#"[
        {"volume":20,"contents":null,"locked":false},
        {"volume":30,"contents":{"item_id":7,"quantity":5,"item_type":"Item","durability":100},"locked":true},
        {"volume":40,"contents":{"item_id":8,"quantity":2,"item_type":"Cargo","durability":null},"locked":false}
    ]"#;

    fn legacy_decode(json: &str) -> Result<Vec<Pocket>, sea_orm::TryGetError> {
        let value = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|error| DbErr::Json(error.to_string()))?;
        // Use SeaORM's actual old element-by-element conversion, including its
        // output-vector growth policy, rather than approximating the decoder.
        <Pocket as TryGetableFromJson>::from_json_vec(value)
    }

    #[test]
    fn direct_decode_matches_legacy_and_preserves_stored_json() {
        for json in ["[]", FIXTURE] {
            let direct: PocketsJson = serde_json::from_str(json).unwrap();
            assert_eq!(direct.0, legacy_decode(json).unwrap());
            assert_eq!(
                serde_json::to_value(&direct.0).unwrap(),
                serde_json::from_str::<serde_json::Value>(json).unwrap()
            );
        }
        // Unknown fields remain ignored, as in the entity's serde definitions.
        let json = r#"[{"volume":1,"contents":null,"locked":true,"extra":{"nested":[1,2]}}]"#;
        assert_eq!(
            serde_json::from_str::<PocketsJson>(json).unwrap().0,
            legacy_decode(json).unwrap()
        );
    }

    #[test]
    fn malformed_pockets_remain_errors() {
        for json in [
            "null",
            "{}",
            "[null]",
            "[{}]",
            r#"[{"volume":2147483648,"contents":null,"locked":false}]"#,
            r#"[{"volume":1,"contents":{"item_id":1,"quantity":2,"item_type":"Invalid"},"locked":false}]"#,
            r#"[{"volume":1,"contents":null,"locked":"false"}]"#,
        ] {
            assert!(legacy_decode(json).is_err(), "{json}");
            assert!(serde_json::from_str::<PocketsJson>(json).is_err(), "{json}");
        }
    }

    #[tokio::test]
    #[ignore = "requires INVENTORY_TEST_DATABASE_URL; runs read-only PostgreSQL SELECT fixtures"]
    async fn postgres_direct_decode_matches_entity_and_propagates_errors() {
        let db = Database::connect(std::env::var("INVENTORY_TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        for json in ["[]", FIXTURE, "null", "{}", "[{}]"] {
            let row = db.query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT 123::bigint AS entity_id, $1::jsonb AS pockets, 2::integer AS inventory_index, 3::integer AS cargo_index, 456::bigint AS owner_entity_id, 789::bigint AS player_owner_entity_id, 4::smallint AS region",
                [serde_json::from_str::<serde_json::Value>(json).unwrap().into()],
            )).await.unwrap().unwrap();
            let legacy = Model::from_query_result(&row, "");
            let direct = InventoryDbRow::from_query_result(&row, "");
            match (legacy, direct) {
                (Ok(legacy), Ok(direct)) => assert_eq!(legacy, direct.0),
                (Err(_), Err(_)) => {}
                _ => panic!("decoder mismatch for {json}"),
            }
        }
        // SQL NULL and missing columns must also fail, not yield an empty snapshot.
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT 123::bigint AS entity_id, NULL::jsonb AS pockets".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(InventoryDbRow::from_query_result(&row, "").is_err());
        assert!(InventoryDbRow::from_query_result(&row, "missing_").is_err());
        db.close().await.unwrap();
    }

    fn profile_decode(decode: impl Fn(&str) -> Vec<Pocket>) {
        let fixture = legacy_decode(FIXTURE).unwrap();
        let pockets = fixture.iter().cycle().take(96).cloned().collect::<Vec<_>>();
        let json = serde_json::to_string(&pockets).unwrap();
        for _ in 0..10_000 {
            let decoded = decode(std::hint::black_box(&json));
            assert_eq!(decoded, pockets);
            std::hint::black_box(decoded);
        }
    }

    #[test]
    #[ignore = "allocation profiling workload: run this test alone under Heaptrack"]
    fn profile_legacy_json_decoding() {
        profile_decode(|json| legacy_decode(json).unwrap());
    }

    #[test]
    #[ignore = "allocation profiling workload: run this test alone under Heaptrack"]
    fn profile_direct_json_decoding() {
        profile_decode(|json| serde_json::from_str::<PocketsJson>(json).unwrap().0);
    }
}
