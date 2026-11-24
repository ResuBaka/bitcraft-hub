//! Mirror current node health, not a hit history. Only the write batch lives
//! in API memory; subscribers bootstrap from Postgres and receive committed HP.
use super::{
    BroadcastTopics, OutboundWebSocketMessages, SpacetimeUpdateMessages as Msg,
    WorkerReceiveMetrics,
    batched_worker::collect_db_batch,
    initial_sync,
    worker_channel::{UnboundedSender, shutdown_channel},
};
use crate::AppState;
use entity::resource_health_state::{Column, Entity, Model};
use game_module::module_bindings::ResourceHealthState;
use sea_orm::{DbErr, EntityTrait, IntoActiveModel, QueryFilter, sea_query::OnConflict};
use std::{collections::HashMap, time::Duration};

const BATCH: usize = 3000;
type Pending = HashMap<i64, Option<Model>>;

fn row(region: entity::shared::Region, value: ResourceHealthState) -> Model {
    Model {
        entity_id: value.entity_id as i64,
        region,
        health: value.health,
    }
}

fn notify(state: &AppState, id: Option<i64>, health: Option<i32>) {
    let _ = state.tx.send_if_interested(
        BroadcastTopics::new([("resource_health".into(), id)]),
        || OutboundWebSocketMessages::ResourceHealthChanged {
            target_id: id.map(|id| id.to_string()),
            health,
        },
    );
}

async fn flush(
    state: &AppState,
    conflict: &OnConflict,
    pending: &mut Pending,
) -> Result<(), DbErr> {
    let mut writes = Vec::new();
    let mut deletes = Vec::new();
    let mut updates = Vec::new();
    for (id, value) in pending.drain() {
        match value {
            Some(model) => {
                updates.push((id, model.health));
                writes.push(model.into_active_model());
            }
            None => deletes.push(id),
        }
    }
    if !writes.is_empty() {
        Entity::insert_many(writes)
            .on_conflict(conflict.clone())
            .exec(&state.conn)
            .await?;
        for (id, hp) in updates {
            notify(state, Some(id), Some(hp));
        }
    }
    if !deletes.is_empty() {
        Entity::delete_many()
            .filter(initial_sync::bigint_keys_filter(Column::EntityId, &deletes))
            .exec(&state.conn)
            .await?;
        for id in deletes {
            notify(state, Some(id), None);
        }
    }
    Ok(())
}

async fn persist(
    state: &AppState,
    conflict: &OnConflict,
    pending: &mut Pending,
    message: Msg<ResourceHealthState>,
) -> Result<(), DbErr> {
    match message {
        Msg::Initial {
            database_name: region,
            data,
            ..
        } => {
            flush(state, conflict, pending).await?;
            initial_sync::reconcile::<Entity, _, _, _, _, _>(
                &state.conn,
                Column::Region,
                region,
                conflict,
                BATCH,
                data.into_iter().map(|v| row(region, v)),
                |r| r.entity_id,
                |keys| initial_sync::bigint_keys_filter(Column::EntityId, keys),
                |_, _| {},
            )
            .await?;
            // Reconnect can change or remove a watched node without a live hit.
            notify(state, None, None);
        }
        Msg::Insert {
            new, database_name, ..
        }
        | Msg::Update {
            new, database_name, ..
        } => {
            pending.insert(new.entity_id as i64, Some(row(database_name, new)));
        }
        Msg::Remove { delete, .. } => {
            pending.insert(delete.entity_id as i64, None);
        }
    }
    Ok(())
}

pub(super) fn start_worker(
    state: AppState,
    delay: Duration,
) -> UnboundedSender<Msg<ResourceHealthState>> {
    let (tx, mut rx) = shutdown_channel(&state.shutdown);
    state.shutdown.clone().spawn_worker(async move {
        let metrics = WorkerReceiveMetrics::new("resource_health_state");
        let conflict = OnConflict::column(Column::EntityId)
            .update_columns([Column::Region, Column::Health])
            .to_owned();
        let mut batch = Vec::new();
        let mut pending = Pending::new();
        while collect_db_batch(&mut rx, &mut batch, BATCH, delay).await != 0 {
            metrics.record(batch.len());
            for msg in batch.drain(..) {
                if let Err(error) = persist(&state, &conflict, &mut pending, msg).await {
                    tracing::error!(%error, "Could not reconcile resource health");
                }
            }
            if let Err(error) = flush(&state, &conflict, &mut pending).await {
                tracing::error!(%error, "Could not persist resource health");
            }
        }
    });
    tx
}

#[cfg(test)]
#[tokio::test]
#[ignore = "requires RESOURCE_HEALTH_TEST_URL; uses PostgreSQL temporary tables"]
async fn mirrors_health_and_reconciles_despawned_nodes() -> anyhow::Result<()> {
    use migration::{Migrator, MigratorTrait, SchemaManager};
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};
    let mut opts = ConnectOptions::new(std::env::var("RESOURCE_HEALTH_TEST_URL")?);
    opts.max_connections(1).set_schema_search_path("pg_temp");
    let db = Database::connect(opts).await?;
    db.execute_unprepared("CREATE TEMP TABLE resource_desc (id integer PRIMARY KEY)")
        .await?;
    let schema = SchemaManager::new(&db);
    let migration = Migrator::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261124_000004_resource_health")
        .unwrap();
    migration.up(&schema).await?;
    db.execute_unprepared("INSERT INTO resource_health_state VALUES (1,14,60), (2,7,100)")
        .await?;
    let (out, mut events) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(db.clone(), out, Default::default());
    const ID: u64 = 9007199254740993;
    let _watcher = state
        .subscribe_test_client(&[("resource_health", Some(ID as i64))])
        .await;
    let tx = start_worker(state.clone(), Duration::from_millis(1));
    let hp = |health| ResourceHealthState {
        entity_id: ID,
        health,
    };
    let snapshot = |region, data| Msg::Initial {
        database_name: region,
        data,
        batches: None,
    };
    tx.send(snapshot(14, vec![hp(100)])).unwrap();
    tx.send(Msg::Update {
        database_name: 14,
        old: hp(100),
        new: hp(42),
        event: None,
    })
    .unwrap();
    // A snapshot is an ordering boundary for pending live updates.
    tx.send(snapshot(
        7,
        vec![ResourceHealthState {
            entity_id: 2,
            health: 100,
        }],
    ))
    .unwrap();
    tx.send(Msg::Remove {
        database_name: 14,
        delete: hp(42),
        event: None,
    })
    .unwrap();
    state.shutdown.drain().await;
    for expected in [Some(42), None] {
        let OutboundWebSocketMessages::ResourceHealthChanged { target_id, health } =
            events.try_recv()?
        else {
            panic!("expected a targeted HP update");
        };
        assert_eq!(target_id, Some(ID.to_string()));
        assert_eq!(health, expected);
    }
    assert!(events.try_recv().is_err());
    assert_eq!(
        Entity::find().all(&db).await?,
        vec![Model {
            entity_id: 2,
            region: 7,
            health: 100
        }]
    );
    migration.down(&schema).await?;
    Ok(())
}
