//! Global empire mirror. Callbacks collect one SDK message; the connection
//! driver persists it before advancing, so reconnects cannot race older writes.
use super::initial_sync::reconcile_global;
use entity::empire::*;
use game_global_module::module_bindings::*;
use parking_lot::Mutex;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, Iterable,
    PrimaryKeyToColumn, QueryFilter, TryGetableMany, Value, sea_query::OnConflict,
};
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::{collections::HashMap, hash::Hash, sync::Arc};

const BATCH_SIZE: usize = 1_000;

async fn persist<E, K>(
    db: &DatabaseConnection,
    rows: HashMap<K, Option<E::Model>>,
    snapshot: bool,
    pk: E::Column,
    key: impl Fn(&E::Model) -> K + Send + Sync,
) -> Result<(), DbErr>
where
    E: EntityTrait,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: Copy + TryGetableMany + Into<Value> + Eq + Hash + Send + Sync,
{
    if rows.is_empty() && !snapshot {
        return Ok(());
    }
    let conflict = OnConflict::column(pk)
        .update_columns(E::Column::iter().filter(|c| E::PrimaryKey::from_column(*c).is_none()))
        .to_owned();
    let filter = |keys: &[K]| Condition::all().add(pk.is_in(keys.iter().copied()));
    if snapshot {
        // Also reconcile an empty successful snapshot: everything previously
        // mirrored for this table was removed while we were disconnected.
        return reconcile_global::<E, _, _, _, _, _>(
            db,
            &conflict,
            BATCH_SIZE,
            rows.into_values().flatten(),
            key,
            filter,
            |_, _| {},
        )
        .await;
    }
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for (key, row) in rows {
        match row {
            Some(row) => upserts.push(row.into_active_model()),
            None => deletes.push(key),
        }
    }
    let mut upserts = upserts.into_iter();
    while upserts.len() > 0 {
        E::insert_many(upserts.by_ref().take(BATCH_SIZE))
            .on_conflict(conflict.clone())
            .exec(db)
            .await?;
    }
    for batch in deletes.chunks(BATCH_SIZE) {
        E::delete_many().filter(filter(batch)).exec(db).await?;
    }
    Ok(())
}

// Keep subscription SQL, callbacks, primary-key handling, and snapshot
// completeness tied to the same table list (including non-entity_id keys).
macro_rules! empire_tables {
    ($($table:ident($pk:ident: $key:ty, $column:ident)),+ $(,)?) => {
        pub(super) const QUERIES: &[&str] = &[$(concat!("SELECT * FROM ", stringify!($table))),+];

        #[derive(Default)]
        pub(super) struct Changes {
            $($table: HashMap<$key, Option<$table::Model>>),+
        }

        impl Changes {
            pub(super) fn listen(ctx: &DbConnection) -> Arc<Mutex<Self>> {
                let changes = Arc::new(Mutex::new(Self::default()));
                $({
                    let table = ctx.db.$table();
                    let target = changes.clone();
                    table.on_initial(move |_, rows| {
                        target.lock().$table.extend(rows.iter().map(|row| {
                            (row.$pk as $key, Some(row.into()))
                        }));
                    });
                    let target = changes.clone();
                    table.on_insert(move |ctx, row| {
                        if !matches!(ctx.event, Event::SubscribeApplied) {
                            target.lock().$table.insert(row.$pk as $key, Some(row.into()));
                        }
                    });
                    let target = changes.clone();
                    table.on_update(move |_, _, row| {
                        target.lock().$table.insert(row.$pk as $key, Some(row.into()));
                    });
                    let target = changes.clone();
                    table.on_delete(move |_, row| {
                        target.lock().$table.insert(row.$pk as $key, None);
                    });
                })+
                changes
            }

            pub(super) async fn persist(self, db: &DatabaseConnection, snapshot: bool) -> anyhow::Result<()> {
                use anyhow::Context;
                $(persist::<$table::Entity, _>(
                    db, self.$table, snapshot, $table::Column::$column, |r| r.$pk,
                ).await.context(stringify!($table))?;)+
                Ok(())
            }
        }
    };
}

empire_tables!(
    parameters_desc(version: i32, Version),
    empire_state(entity_id: i64, EntityId),
    empire_player_data_state(entity_id: i64, EntityId),
    empire_rank_state(entity_id: i64, EntityId),
    empire_settlement_state(building_entity_id: i64, BuildingEntityId),
    empire_node_state(entity_id: i64, EntityId),
    empire_chunk_state(chunk_index: i64, ChunkIndex),
    empire_node_siege_state(entity_id: i64, EntityId),
    empire_emblem_state(entity_id: i64, EntityId),
    empire_directive_state(entity_id: i64, EntityId),
    empire_foundry_state(entity_id: i64, EntityId),
    empire_siege_engine_state(entity_id: i64, EntityId),
    empire_notification_state(entity_id: i64, EntityId),
    empire_rank_desc(rank: i32, Rank),
    empire_color_desc(id: i32, Id),
    empire_icon_desc(id: i32, Id),
    empire_supplies_desc(cargo_id: i32, CargoId),
    empire_territory_desc(id: i16, Id),
    empire_notification_desc(id: i32, Id),
);

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};

    #[tokio::test]
    #[ignore = "requires EMPIRE_TEST_DATABASE_URL; uses an isolated PostgreSQL schema"]
    async fn mirror_snapshot_live_changes_and_empty_reconnect() -> anyhow::Result<()> {
        let schema = format!("empire_test_{}", rand::random::<u64>());
        let mut options = ConnectOptions::new(std::env::var("EMPIRE_TEST_DATABASE_URL")?);
        options.max_connections(1).set_schema_search_path(&schema);
        let db = Database::connect(options).await?;
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await?;
        let migration = migration::Migrator::migrations()
            .into_iter()
            .find(|m| m.name() == "m20261124_000001_empire")
            .unwrap();
        let manager = migration::SchemaManager::new(&db);
        migration.up(&manager).await?;
        let parameters = migration::Migrator::migrations()
            .into_iter()
            .find(|m| m.name() == "m20261124_000003_empire_decay_parameters")
            .unwrap();
        parameters.up(&manager).await?;

        let chunk = |id, owner| {
            empire_chunk_state::Model::from(&EmpireChunkState {
                chunk_index: id,
                watchtower_entity_id: 12,
                empire_entity_id: owner,
            })
        };
        let mut initial = Changes::default();
        initial.empire_chunk_state.insert(8, Some(chunk(8, 0)));
        initial.empire_chunk_state.insert(9, Some(chunk(9, 17)));
        let rank = empire_rank_desc::Model::from(&EmpireRankDesc {
            rank: 3,
            title: "Knight".into(),
            max_count: None,
            permissions: vec![true, false],
        });
        initial.empire_rank_desc.insert(3, Some(rank.clone()));
        initial.persist(&db, true).await?;
        assert_eq!(
            empire_rank_desc::Entity::find_by_id(3).one(&db).await?,
            Some(rank)
        );

        let mut live = Changes::default();
        live.empire_chunk_state.insert(8, Some(chunk(8, 42)));
        live.empire_chunk_state.insert(9, None);
        live.empire_chunk_state.insert(10, Some(chunk(10, 42)));
        live.persist(&db, false).await?;
        assert_eq!(
            empire_chunk_state::Entity::find_by_id(8).one(&db).await?,
            Some(chunk(8, 42))
        );
        assert!(
            empire_chunk_state::Entity::find_by_id(9)
                .one(&db)
                .await?
                .is_none()
        );
        assert_eq!(
            empire_chunk_state::Entity::find_by_id(10).one(&db).await?,
            Some(chunk(10, 42))
        );

        let mut reconnect = Changes::default();
        reconnect.empire_chunk_state.insert(8, Some(chunk(8, 0)));
        reconnect.persist(&db, true).await?;
        assert_eq!(
            empire_chunk_state::Entity::find().all(&db).await?,
            [chunk(8, 0)]
        );
        assert!(empire_rank_desc::Entity::find().all(&db).await?.is_empty());
        parameters.down(&manager).await?;
        migration.down(&manager).await?;
        db.execute_unprepared(&format!("DROP SCHEMA {schema}"))
            .await?;
        Ok(())
    }
}
