use super::{
    SpacetimeUpdateMessages as Msg, WorkerReceiveMetrics, initial_sync,
    worker_channel::{UnboundedSender, shutdown_channel},
};
use crate::AppState;
use entity::{placeables::*, shared::Region};
use game_module::module_bindings::{
    GrowthTimer, PlaceableDesc, PlaceableInteractionDesc, PlaceableState,
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
}

impl Workers {
    pub(super) fn new(app: &AppState) -> Self {
        use placeable_desc as d;
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
    migration.down(&schema).await?;
    Ok(())
}
