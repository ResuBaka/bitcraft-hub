use super::{
    SpacetimeUpdateMessages as Msg, WorkerReceiveMetrics, initial_sync,
    worker_channel::{UnboundedSender, shutdown_channel},
};
use crate::AppState;
use entity::{placeables::*, shared::Region};
use game_module::module_bindings::{
    GrowthTimer, PlaceableDesc, PlaceableGrowthDesc, PlaceableGrowthOutcomeV2,
    PlaceableInteractionDesc, PlaceableState,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, IntoActiveModel, Iterable, PrimaryKeyToColumn,
    QueryFilter, TryGetableMany, Value, sea_query::OnConflict,
};
use std::hash::Hash;

/// These low-volume tables have complete, unchunked initial snapshots. Keep
/// one ordered writer per table across reconnects and discard stale rows only
/// in the scope covered by that snapshot (global descriptions or one region).
fn mirror<R, E, K>(
    state: &AppState,
    pk: E::Column,
    scope: Option<E::Column>,
    project: fn(Region, R) -> E::Model,
    key: fn(&E::Model) -> K,
) -> UnboundedSender<Msg<R>>
where
    R: Send + 'static,
    E: EntityTrait,
    E::ActiveModel: Send,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: Copy + Eq + Hash + TryGetableMany + Into<Value> + Send + Sync + 'static,
{
    let (tx, mut rx) = shutdown_channel(&state.shutdown);
    let db = state.conn.clone();
    state.shutdown.spawn_worker(async move {
        let table = E::default();
        let metrics = WorkerReceiveMetrics::new(table.table_name());
        let conflict = OnConflict::column(pk)
            .update_columns(E::Column::iter().filter(|col| E::PrimaryKey::from_column(*col).is_none()))
            .to_owned();
        let filter = |ids: &[K]| Condition::all().add(pk.is_in(ids.iter().copied()));
        while let Some(msg) = rx.recv().await {
            metrics.record(1);
            let result = match msg {
                Msg::Initial { data, database_name: region, .. } => {
                    let rows = data.into_iter().map(|r| project(region, r));
                    if let Some(scope) = scope {
                        initial_sync::reconcile::<E, _, _, _, _, _>(&db, scope, region, &conflict, 1000, rows, key, filter, |_,_| {}).await
                    } else {
                        initial_sync::reconcile_global::<E, _, _, _, _, _>(&db, &conflict, 1000, rows, key, filter, |_,_| {}).await
                    }
                }
                Msg::Insert { new, database_name, .. } | Msg::Update { new, database_name, .. } => {
                    E::insert(project(database_name, new).into_active_model()).on_conflict(conflict.clone()).exec(&db).await.map(|_| ())
                }
                Msg::Remove { delete, database_name, .. } => {
                    E::delete_many().filter(pk.eq(key(&project(database_name, delete)))).exec(&db).await.map(|_| ())
                }
            };
            if let Err(error) = result {
                tracing::error!(%error, table = table.table_name(), "Could not mirror placeable data");
            }
        }
    });
    tx
}

#[derive(Clone)]
pub(super) struct Workers {
    pub state: UnboundedSender<Msg<PlaceableState>>,
    pub timer: UnboundedSender<Msg<GrowthTimer>>,
    pub desc: UnboundedSender<Msg<PlaceableDesc>>,
    pub interactions: UnboundedSender<Msg<PlaceableInteractionDesc>>,
    pub growth: UnboundedSender<Msg<PlaceableGrowthDesc>>,
}

fn outcomes(rows: Option<Vec<PlaceableGrowthOutcomeV2>>) -> Vec<i32> {
    // The game picks from positive-weight outcomes; zero is also a real
    // outcome (despawn), not an alias for one of the surviving placeables.
    let mut ids: Vec<_> = rows
        .into_iter()
        .flatten()
        .filter(|r| r.probability > 0.0)
        .map(|r| r.placeable_id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

impl Workers {
    pub(super) fn new(app: &AppState) -> Self {
        use placeable_desc as d;
        use placeable_growth_desc as g;
        use placeable_growth_timer as t;
        use placeable_interaction_desc as i;
        use placeable_state as p;
        Self {
            state: mirror::<_, p::Entity, _>(
                app,
                p::Column::EntityId,
                Some(p::Column::Region),
                |region, row: PlaceableState| p::Model {
                    region,
                    entity_id: row.entity_id as i64,
                    owner_entity_id: row.owner_entity_id as i64,
                    placeable_id: row.placeable_id,
                },
                |r| r.entity_id,
            ),
            timer: mirror::<_, t::Entity, _>(
                app,
                t::Column::EntityId,
                Some(t::Column::Region),
                |region, row: GrowthTimer| t::Model {
                    region,
                    entity_id: row.entity_id as i64,
                    ends_at_ms: match row.scheduled_at {
                        spacetimedb_sdk::ScheduleAt::Time(time) => {
                            Some(time.to_micros_since_unix_epoch() / 1000)
                        }
                        _ => None,
                    },
                },
                |r| r.entity_id,
            ),
            desc: mirror::<_, d::Entity, _>(
                app,
                d::Column::Id,
                None,
                |_, row: PlaceableDesc| d::Model {
                    id: row.id,
                    name: row.name,
                    tier: row.tier,
                    tag: row.tag,
                },
                |r| r.id,
            ),
            interactions: mirror::<_, i::Entity, _>(
                app,
                i::Column::Id,
                None,
                |_, row: PlaceableInteractionDesc| i::Model {
                    id: row.id,
                    placeable_id: row.placeable_id,
                    verb_phrase: row.verb_phrase,
                    next_ids: outcomes(row.on_destroy_outcomes),
                    // Item and cargo IDs belong to different namespaces.
                    reward_keys: row
                        .output_item_stacks
                        .into_iter()
                        .chain(row.on_destroy_yield.into_iter().flatten())
                        .filter(|stack| stack.quantity > 0)
                        .map(|stack| {
                            (i64::from(stack.item_id) << 1)
                                | i64::from(matches!(
                                    stack.item_type,
                                    game_module::module_bindings::ItemType::Cargo
                                ))
                        })
                        .collect(),
                },
                |r| r.id,
            ),
            growth: mirror::<_, g::Entity, _>(
                app,
                g::Column::Id,
                None,
                |_, row: PlaceableGrowthDesc| g::Model {
                    id: row.id,
                    placeable_id: row.placeable_id,
                    next_ids: outcomes(row.outcomes_v_2),
                },
                |r| r.id,
            ),
        }
    }
}

#[cfg(test)]
#[tokio::test]
#[ignore = "requires PLACEABLE_TEST_URL; all tables are session-local"]
async fn owned_placeables_follow_stage_changes_and_reconcile() -> anyhow::Result<()> {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use migration::{Migrator, MigratorTrait, SchemaManager};
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};
    use tower::ServiceExt;

    let mut opts = ConnectOptions::new(std::env::var("PLACEABLE_TEST_URL")?);
    opts.max_connections(1).set_schema_search_path("pg_temp");
    let db = Database::connect(opts).await?;
    db.execute_unprepared("CREATE TEMP TABLE init_temp_schema(id int)")
        .await?;
    let schema = SchemaManager::new(&db);
    let migration = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261124_000005_placeables")
        .unwrap();
    migration.up(&schema).await?;
    db.execute_unprepared(
        "
        INSERT INTO placeable_desc VALUES (1,'Set trap',2),(2,'Sprung trap',2);
        INSERT INTO placeable_interaction_desc VALUES (9,2,'Collect');
        INSERT INTO placeable_state VALUES (10,7,42,1),(11,8,42,2),(12,8,43,1);
        INSERT INTO placeable_growth_timer VALUES (10,7,100);
    ",
    )
    .await?;
    let state = AppState::new(
        db.clone(),
        tokio::sync::mpsc::unbounded_channel().0,
        Default::default(),
    );
    let workers = Workers::new(&state);
    const ID: u64 = 9007199254740993;
    let node = |id| PlaceableState {
        entity_id: id,
        owner_entity_id: 42,
        placeable_id: 1,
        direction_index: 0,
    };
    workers
        .state
        .send(Msg::Initial {
            database_name: 7,
            data: vec![node(ID), node(19)],
            batches: None,
        })
        .unwrap();
    workers
        .state
        .send(Msg::Remove {
            database_name: 7,
            delete: node(19),
            event: None,
        })
        .unwrap();
    workers
        .timer
        .send(Msg::Initial {
            database_name: 7,
            data: vec![GrowthTimer {
                scheduled_id: 3,
                entity_id: ID,
                growth_recipe_id: 1,
                scheduled_at: spacetimedb_sdk::ScheduleAt::Time(
                    spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(1000000),
                ),
            }],
            batches: None,
        })
        .unwrap();
    state.shutdown.drain().await;

    let app = crate::player_state::get_routes().with_state(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bitcraft/players/42/placeables")
                .header("Accept", "application/json")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let rows: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await?)?;
    assert_eq!(
        rows,
        serde_json::json!([
            {"id": ID.to_string(), "owner_id":"42","name":"Set trap","tier":2,"growing":true,"ends_at_ms":1000,"interactions":[]},
            {"id":"11","owner_id":"42","name":"Sprung trap","tier":2,"growing":false,"ends_at_ms":null,"interactions":["Collect"]}
        ])
    );
    let tags = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261124_000006_placeable_tag")
        .unwrap();
    tags.up(&schema).await?;
    db.execute_unprepared("
        INSERT INTO placeable_desc VALUES (3,'Growing Wispweave',1,'Farmed Crop'),(4,'Rice Plant',2,'Rice Farming');
        INSERT INTO placeable_interaction_desc VALUES (20,4,'Harvest');
        INSERT INTO placeable_state VALUES (30,7,42,3),(31,7,42,3),(32,8,43,3),(33,8,42,4);
        INSERT INTO placeable_growth_timer VALUES (30,7,3000),(31,7,2000),(32,8,1000);
    ").await?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/bitcraft/players/42/farming")
                .header("Accept", "application/json")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let crops: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await?)?;
    assert_eq!(
        crops,
        serde_json::json!([
            {"id":"3:true","owner_id":"42","name":"Growing Wispweave","tier":1,"growing":true,"ends_at_ms":2000,"interactions":[],"quantity":2},
            {"id":"4:false","owner_id":"42","name":"Rice Plant","tier":2,"growing":false,"ends_at_ms":null,"interactions":["Harvest"],"quantity":1}
        ])
    );
    let outcomes = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261124_000007_placeable_outcomes")
        .unwrap();
    outcomes.up(&schema).await?;
    db.execute_unprepared("
        INSERT INTO placeable_desc VALUES (5,'Sown Wispweave',1,'Farmed Crop'),(6,'Ripe Wispweave',1,'Farmed Crop'),
            (7,'Uncertain seed',1,'Farmed Crop');
        INSERT INTO placeable_state VALUES (34,7,42,5),(35,7,42,6),(36,7,42,7);
        INSERT INTO placeable_growth_desc VALUES (1,1,ARRAY[2]),(3,3,ARRAY[6]),(7,7,ARRAY[4,6]);
        INSERT INTO placeable_interaction_desc VALUES (21,5,'Water',ARRAY[3]),(22,6,'Harvest',ARRAY[]::integer[]);
        UPDATE placeable_interaction_desc SET next_ids=ARRAY[1] WHERE id=9;
        UPDATE placeable_interaction_desc SET reward_keys=ARRAY[500]::bigint[] WHERE id=9;
        INSERT INTO placeable_interaction_desc VALUES (23,1,'Collect','{}',ARRAY[100]::bigint[]);
    ").await?;
    for farming in [false, true] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/bitcraft/players/42/placeable-summary?farming={farming}"
                    ))
                    .header("Accept", "application/json")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let groups: Vec<serde_json::Value> =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await?)?;
        let states = |outcome| {
            groups
                .iter()
                .filter(|r| r["outcome_id"].as_i64() == Some(outcome))
                .map(|r| {
                    (
                        r["stage_id"].as_i64().unwrap(),
                        r["quantity"].as_i64().unwrap(),
                        r["ready"].as_bool().unwrap(),
                        r["ends_at_ms"].as_i64(),
                    )
                })
                .collect::<Vec<_>>()
        };
        // Ready, timed (first of two), and needs water are one crop outcome.
        assert_eq!(
            states(6),
            vec![
                (6, 1, true, None),
                (3, 2, false, Some(2000)),
                (5, 1, false, None)
            ]
        );
        assert_eq!(
            states(7),
            vec![(7, 1, false, None)],
            "don't predict a random crop"
        );
        assert_eq!(
            states(2),
            if farming {
                vec![]
            } else {
                vec![(2, 1, true, None), (1, 1, false, Some(1000))]
            }
        );
    }
    outcomes.down(&schema).await?;
    tags.down(&schema).await?;
    migration.down(&schema).await?;
    Ok(())
}
