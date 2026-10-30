//! Bounded database comparisons for complete regional and global snapshots.
use entity::shared::Region;
use futures::{Stream, TryStreamExt, stream};
use migration::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, Iterable,
    PrimaryKeyToColumn, QueryFilter, QuerySelect, TryGetableMany,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// PostgreSQL array membership keeps the parameter count and SQL shape fixed.
pub(crate) fn bigint_keys_filter<C: ColumnTrait>(column: C, keys: &[i64]) -> Condition {
    Condition::all().add(migration::sea_query::Expr::cust_with_exprs(
        "$1 = ANY($2::bigint[])",
        [
            migration::sea_query::Expr::col(column.as_column_ref()),
            migration::sea_query::Expr::val(sea_orm::Value::from(keys.to_vec())),
        ],
    ))
}

/// Cache state is independent of database state: even a database-unchanged row
/// may be missing/stale in the cache. Clone only when the cache needs a new value.
pub(crate) fn update_cache_if_changed<K: Eq + Hash, M: Clone + PartialEq>(
    cache: &dashmap::DashMap<K, M>,
    key: K,
    model: &M,
) {
    use dashmap::mapref::entry::Entry;
    match cache.entry(key) {
        Entry::Occupied(mut entry) => {
            if entry.get() != model {
                entry.insert(model.clone());
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(model.clone());
        }
    }
}

/// Global description tables have no region column. Only complete table
/// snapshots may use this entry point; regional state must use `reconcile`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn reconcile_global<E, K, I, Key, Filter, Observe>(
    conn: &DatabaseConnection,
    on_conflict: &OnConflict,
    batch_size: usize,
    incoming: I,
    key: Key,
    filter: Filter,
    observe: Observe,
) -> Result<(), DbErr>
where
    E: EntityTrait,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: TryGetableMany + Eq + Hash + Clone + Send + Sync,
    I: Iterator<Item = E::Model> + Send,
    Key: Fn(&E::Model) -> K + Send + Sync,
    Filter: Fn(&[K]) -> Condition + Send + Sync,
    Observe: FnMut(&E::Model, bool) + Send,
{
    reconcile_scoped::<E, _, _, _, _, _>(
        conn,
        Condition::all(),
        on_conflict,
        batch_size,
        stream::iter(incoming.map(Ok)),
        key,
        filter,
        observe,
    )
    .await
}

/// Flush older live writes/deletes before comparing a newer complete snapshot.
/// Otherwise a buffered live write can overwrite the snapshot after reconciliation.
pub(crate) async fn flush_before_snapshot<E: EntityTrait, K>(
    conn: &DatabaseConnection,
    on_conflict: &OnConflict,
    messages: &mut Vec<E::ActiveModel>,
    deletes: &mut Vec<K>,
    filter: impl Fn(&[K]) -> Condition,
) -> Result<(), DbErr> {
    if !messages.is_empty() {
        E::insert_many(messages.drain(..))
            .on_conflict(on_conflict.clone())
            .exec(conn)
            .await?;
    }
    for keys in deletes.chunks(1_000) {
        E::delete_many().filter(filter(keys)).exec(conn).await?;
    }
    deletes.clear();
    Ok(())
}

/// Retain only primary keys for stale-row detection and one batch of full rows.
/// An error aborts reconciliation before stale rows are deleted. `incoming` must
/// represent a complete region, not an individual page of a subscription response.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn reconcile<E, K, I, Key, Filter, Observe>(
    conn: &DatabaseConnection,
    region_column: E::Column,
    region: Region,
    on_conflict: &OnConflict,
    batch_size: usize,
    incoming: I,
    key: Key,
    filter: Filter,
    observe: Observe,
) -> Result<(), DbErr>
where
    E: EntityTrait,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: TryGetableMany + Eq + Hash + Clone + Send + Sync,
    I: Iterator<Item = E::Model> + Send,
    Key: Fn(&E::Model) -> K + Send + Sync,
    Filter: Fn(&[K]) -> Condition + Send + Sync,
    Observe: FnMut(&E::Model, bool) + Send,
{
    reconcile_scoped::<E, _, _, _, _, _>(
        conn,
        Condition::all().add(region_column.eq(region)),
        on_conflict,
        batch_size,
        stream::iter(incoming.map(Ok)),
        key,
        filter,
        observe,
    )
    .await
}

/// Streaming snapshot variant. Upstream errors abort before stale-row deletion;
/// successful earlier batches may already have been persisted.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn reconcile_stream<E, K, I, Key, Filter, Observe>(
    conn: &DatabaseConnection,
    region_column: E::Column,
    region: Region,
    on_conflict: &OnConflict,
    batch_size: usize,
    incoming: I,
    key: Key,
    filter: Filter,
    observe: Observe,
) -> Result<(), DbErr>
where
    E: EntityTrait,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: TryGetableMany + Eq + Hash + Clone + Send + Sync,
    I: Stream<Item = Result<E::Model, DbErr>> + Send,
    Key: Fn(&E::Model) -> K + Send + Sync,
    Filter: Fn(&[K]) -> Condition + Send + Sync,
    Observe: FnMut(&E::Model, bool) + Send,
{
    reconcile_scoped::<E, _, _, _, _, _>(
        conn,
        Condition::all().add(region_column.eq(region)),
        on_conflict,
        batch_size,
        incoming,
        key,
        filter,
        observe,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn reconcile_scoped<E, K, I, Key, Filter, Observe>(
    conn: &DatabaseConnection,
    scope: Condition,
    on_conflict: &OnConflict,
    batch_size: usize,
    incoming: I,
    key: Key,
    filter: Filter,
    mut observe: Observe,
) -> Result<(), DbErr>
where
    E: EntityTrait,
    E::Model: PartialEq + IntoActiveModel<E::ActiveModel>,
    K: TryGetableMany + Eq + Hash + Clone + Send + Sync,
    I: Stream<Item = Result<E::Model, DbErr>> + Send,
    Key: Fn(&E::Model) -> K + Send + Sync,
    Filter: Fn(&[K]) -> Condition + Send + Sync,
    Observe: FnMut(&E::Model, bool) + Send,
{
    futures::pin_mut!(incoming);
    let mut remaining = HashSet::new();
    {
        let key_query = E::find()
            .select_only()
            .columns(E::PrimaryKey::iter().map(|column| column.into_column()))
            .filter(scope.clone())
            .into_tuple::<K>();
        let mut keys = key_query.stream(conn).await?;
        while let Some(key) = keys.try_next().await? {
            remaining.insert(key);
        }
        // Release the stream's pooled connection before comparison queries or
        // writes. Only the key set survives, not a full Vec of database rows.
        // Any stream/decode error returns before incoming is polled or deleted.
    }

    // Scratch capacity belongs to this snapshot, not to the worker. Draining
    // moves owned rows into comparisons/writes without reallocating each batch.
    let batch_size = batch_size.max(1);
    let mut batch = Vec::new();
    let mut existing_keys = Vec::new();
    let mut existing = HashMap::new();
    let mut changes = Vec::new();
    loop {
        while batch.len() < batch_size {
            let Some(model) = incoming.try_next().await? else {
                break;
            };
            batch.push(model);
        }
        if batch.is_empty() {
            break;
        }
        existing_keys.clear();
        existing_keys.extend(batch.iter().map(&key).filter(|key| remaining.contains(key)));
        if !existing_keys.is_empty() {
            let query = E::find()
                .filter(scope.clone())
                .filter(filter(&existing_keys));
            let mut rows = query.stream(conn).await?;
            while let Some(model) = rows.try_next().await? {
                existing.insert(key(&model), model);
            }
            // Return the stream's connection before any write. A partial read
            // or decode error exits without observing this batch or deleting.
        }
        for model in batch.drain(..) {
            let model_key = key(&model);
            remaining.remove(&model_key);
            let changed = existing.remove(&model_key).as_ref() != Some(&model);
            observe(&model, changed);
            if changed {
                changes.push(model.into_active_model());
            }
        }
        // Avoid holding full existing rows while awaiting a database write.
        existing.clear();
        if !changes.is_empty() {
            E::insert_many(changes.drain(..))
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;
        }
    }

    let mut remaining = remaining.into_iter();
    loop {
        let keys = remaining.by_ref().take(1_000).collect::<Vec<_>>();
        if keys.is_empty() {
            break;
        }
        E::delete_many()
            .filter(scope.clone())
            .filter(filter(&keys))
            .exec(conn)
            .await?;
    }
    Ok(())
}

/// Match exact composite keys, not the cross product of the two ID lists.
pub(crate) fn composite_filter<C: ColumnTrait>(
    entity_column: C,
    component_column: C,
    keys: &[(i64, i32)],
) -> Condition {
    // One scratch allocation instead of a hash table and one Vec per entity.
    // Sorting our copy leaves the caller's reconciliation key buffer untouched.
    let mut keys = keys.to_vec();
    keys.sort_unstable();
    let mut condition = Condition::any();
    let mut start = 0;
    while start < keys.len() {
        let entity = keys[start].0;
        let mut end = start + 1;
        while end < keys.len() && keys[end].0 == entity {
            end += 1;
        }
        condition = condition.add(
            Condition::all()
                .add(entity_column.eq(entity))
                .add(component_column.is_in(keys[start..end].iter().map(|key| key.1))),
        );
        start = end;
    }
    condition
}

#[cfg(test)]
mod tests {
    use super::*;
    use entity::vault_state_collectibles::{Column, Entity, Model};
    use sea_orm::{DbBackend, MockDatabase, MockExecResult, QueryTrait, Transaction};

    #[test]
    fn bigint_key_filter_uses_one_array_for_any_batch_size() {
        use entity::building_state::{Column, Entity};
        let query = |ids: &[i64]| {
            Entity::find()
                .filter(Column::Region.eq(3_i16))
                .filter(bigint_keys_filter(Column::EntityId, ids))
                .build(DbBackend::Postgres)
        };
        let one = query(&[1]);
        let many = query(&[1, 2, 3]);
        assert_eq!(one.sql, many.sql);
        assert!(many.sql.contains("ANY($2::bigint[])"));
        let values = many.values.unwrap().0;
        assert_eq!(values.len(), 2);
        assert_eq!(values[1], sea_orm::Value::from(vec![1_i64, 2, 3]));
    }

    #[test]
    fn unchanged_cache_entries_are_not_cloned_or_replaced() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Counted {
            value: String,
            clones: Arc<AtomicUsize>,
        }
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.clones.fetch_add(1, Ordering::Relaxed);
                Self {
                    value: self.value.clone(),
                    clones: Arc::clone(&self.clones),
                }
            }
        }
        impl PartialEq for Counted {
            fn eq(&self, other: &Self) -> bool {
                self.value == other.value
            }
        }
        let clones = Arc::new(AtomicUsize::new(0));
        let cache = dashmap::DashMap::new();
        let mut model = Counted {
            value: "original".into(),
            clones: Arc::clone(&clones),
        };
        update_cache_if_changed(&cache, 1, &model);
        assert_eq!(clones.load(Ordering::Relaxed), 1);
        let original_allocation = cache.get(&1).unwrap().value.as_ptr();
        update_cache_if_changed(&cache, 1, &model);
        assert_eq!(clones.load(Ordering::Relaxed), 1);
        assert_eq!(cache.get(&1).unwrap().value.as_ptr(), original_allocation);
        model.value = "changed".into();
        update_cache_if_changed(&cache, 1, &model);
        assert_eq!(clones.load(Ordering::Relaxed), 2);
        assert_eq!(cache.get(&1).unwrap().value, "changed");
    }

    fn description(id: i32, name: &str) -> entity::item_list_desc::Model {
        entity::item_list_desc::Model {
            id,
            name: name.into(),
            possibilities: vec![],
        }
    }

    fn description_conflict() -> OnConflict {
        use entity::item_list_desc::Column as C;
        OnConflict::column(C::Id)
            .update_columns([C::Name, C::Possibilities])
            .to_owned()
    }

    #[test]
    fn cache_comparison_includes_nested_description_data() {
        use entity::{item_list_desc::ItemListPossibility, shared::item_stack::ItemStack};
        let cache = dashmap::DashMap::new();
        let mut model = description(1, "same name");
        model.possibilities.push(ItemListPossibility {
            probability: 0.5,
            items: vec![ItemStack {
                item_id: 10,
                quantity: 1,
                item_type: entity::inventory::ItemType::Item,
                durability: None,
            }],
        });
        update_cache_if_changed(&cache, model.id, &model);
        let original = cache.get(&1).unwrap().possibilities[0].items.as_ptr();
        update_cache_if_changed(&cache, model.id, &model);
        assert_eq!(
            cache.get(&1).unwrap().possibilities[0].items.as_ptr(),
            original
        );
        model.possibilities[0].items[0].quantity = 2;
        update_cache_if_changed(&cache, model.id, &model);
        assert_eq!(cache.get(&1).unwrap().possibilities[0].items[0].quantity, 2);
    }

    #[tokio::test]
    async fn empty_global_snapshot_only_loads_keys_then_deletes() {
        use entity::item_list_desc::Entity as E;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![description_key(1), description_key(2)]])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 2,
            }])
            .into_connection();
        reconcile_global::<E, _, _, _, _, _>(
            &db,
            &description_conflict(),
            0,
            std::iter::empty(),
            |model| model.id,
            description_filter,
            |_, _| panic!("empty snapshot has no models"),
        )
        .await
        .unwrap();
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    fn description_filter(keys: &[i32]) -> Condition {
        Condition::all().add(entity::item_list_desc::Column::Id.is_in(keys.iter().copied()))
    }

    fn description_key(id: i32) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [("0".to_string(), id.into())].into()
    }

    // Model->mock conversion treats this Vec<JSON> field as a SQL array; the
    // actual database column is JSON, so supply its wire representation.
    fn description_row(
        model: &entity::item_list_desc::Model,
    ) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [
            ("id".to_string(), model.id.into()),
            ("name".to_string(), model.name.clone().into()),
            (
                "possibilities".to_string(),
                serde_json::to_value(&model.possibilities).unwrap().into(),
            ),
        ]
        .into()
    }

    #[tokio::test]
    async fn global_description_snapshot_batches_rows_and_deletes_only_missing_ids() {
        use entity::item_list_desc::{Column as C, Entity as E};
        let a = description(1, "unchanged");
        let b = description(2, "changed");
        let c = description(4, "new");
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![
                description_key(1),
                description_key(2),
                description_key(3),
            ]])
            .append_query_results([
                vec![description_row(&a), description_row(&description(2, "old"))],
                vec![description_row(&b)], // first batch upsert
                vec![description_row(&c)], // second batch is new, no full-row read
            ])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let cache = dashmap::DashMap::new();
        let mut observed = Vec::new();
        reconcile_global::<E, _, _, _, _, _>(
            &db,
            &description_conflict(),
            2,
            vec![a.clone(), b.clone(), c.clone()].into_iter(),
            |model| model.id,
            description_filter,
            |model, changed| {
                observed.push((model.id, changed));
                update_cache_if_changed(&cache, model.id, model);
            },
        )
        .await
        .unwrap();
        assert_eq!(observed, [(1, false), (2, true), (4, true)]);
        for model in [a, b, c] {
            assert_eq!(*cache.get(&model.id).unwrap(), model);
        }
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 5);
        assert_eq!(
            log[0],
            Transaction::one(
                E::find()
                    .select_only()
                    .column(C::Id)
                    .filter(Condition::all())
                    .build(DbBackend::Postgres)
            )
        );
        assert_eq!(
            log[1],
            Transaction::one(
                E::find()
                    .filter(Condition::all())
                    .filter(description_filter(&[1, 2]))
                    .build(DbBackend::Postgres)
            )
        );
        assert_eq!(
            log[4],
            Transaction::one(
                E::delete_many()
                    .filter(Condition::all())
                    .filter(description_filter(&[3]))
                    .build(DbBackend::Postgres)
            )
        );
    }

    #[tokio::test]
    async fn database_unchanged_rows_still_repair_missing_or_stale_cache_entries() {
        use entity::item_list_desc::Entity as E;
        let a = description(1, "one");
        let b = description(2, "two");
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![description_key(1), description_key(2)]])
            .append_query_results([vec![description_row(&a), description_row(&b)]])
            .into_connection();
        let cache = dashmap::DashMap::new();
        cache.insert(2, description(2, "stale"));
        reconcile_global::<E, _, _, _, _, _>(
            &db,
            &description_conflict(),
            2,
            vec![a.clone(), b.clone()].into_iter(),
            |model| model.id,
            description_filter,
            |model, changed| {
                assert!(!changed);
                update_cache_if_changed(&cache, model.id, model);
            },
        )
        .await
        .unwrap();
        assert_eq!(*cache.get(&1).unwrap(), a);
        assert_eq!(*cache.get(&2).unwrap(), b);
        assert_eq!(db.into_transaction_log().len(), 2); // no writes
    }

    #[tokio::test]
    async fn global_description_read_failure_does_not_delete_stale_rows() {
        use entity::item_list_desc::Entity as E;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![description_key(1), description_key(2)]])
            .append_query_errors([DbErr::Custom("read failed".into())])
            .into_connection();
        let result = reconcile_global::<E, _, _, _, _, _>(
            &db,
            &description_conflict(),
            1,
            vec![description(1, "one")].into_iter(),
            |model| model.id,
            description_filter,
            |_, _| panic!("failed read must not update cache"),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[tokio::test]
    async fn buffered_live_writes_finish_before_snapshot_comparison() {
        use entity::item_list_desc::Entity as E;
        let before = description(1, "live");
        let after = description(1, "snapshot");
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![description_row(&before)]]) // flush upsert
            .append_query_results([vec![description_key(1)]]) // snapshot keys
            .append_query_results([
                vec![description_row(&before)],
                vec![description_row(&after)],
            ])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let mut messages = vec![description(1, "live").into_active_model()];
        let mut deletes = vec![2];
        flush_before_snapshot::<E, _>(
            &db,
            &description_conflict(),
            &mut messages,
            &mut deletes,
            description_filter,
        )
        .await
        .unwrap();
        assert!(messages.is_empty() && deletes.is_empty());
        reconcile_global::<E, _, _, _, _, _>(
            &db,
            &description_conflict(),
            1,
            vec![after].into_iter(),
            |model| model.id,
            description_filter,
            |_, _| {},
        )
        .await
        .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 5); // live upsert, delete, keys, compare, snapshot upsert
        assert_eq!(
            log[1],
            Transaction::one(
                E::delete_many()
                    .filter(description_filter(&[2]))
                    .build(DbBackend::Postgres)
            )
        );
        assert_eq!(
            log[2],
            Transaction::one(
                E::find()
                    .select_only()
                    .column(entity::item_list_desc::Column::Id)
                    .filter(Condition::all())
                    .build(DbBackend::Postgres)
            )
        );
    }

    #[tokio::test]
    async fn failed_live_flush_aborts_before_snapshot_reads_or_deletes() {
        use entity::item_list_desc::Entity as E;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("write failed".into())])
            .into_connection();
        let mut messages = vec![description(1, "live").into_active_model()];
        let mut deletes = vec![2];
        assert!(
            flush_before_snapshot::<E, _>(
                &db,
                &description_conflict(),
                &mut messages,
                &mut deletes,
                description_filter,
            )
            .await
            .is_err()
        );
        assert_eq!(deletes, [2]);
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    fn collectible(entity_id: i64, id: i32, count: i32) -> Model {
        Model {
            entity_id,
            id,
            activated: true,
            count,
            region: 7,
        }
    }

    // SeaORM's mock positional decoding uses numeric keys for tuple results.
    fn key_row(entity_id: i64, id: i32) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [
            ("0".to_string(), entity_id.into()),
            ("1".to_string(), id.into()),
        ]
        .into()
    }

    fn conflict() -> OnConflict {
        OnConflict::columns([Column::EntityId, Column::Id])
            .update_columns([Column::Activated, Column::Count, Column::Region])
            .to_owned()
    }

    fn filter(keys: &[(i64, i32)]) -> Condition {
        composite_filter(Column::EntityId, Column::Id, keys)
    }

    #[tokio::test]
    async fn compares_in_batches_and_deletes_only_missing_composite_keys() {
        let a = collectible(1, 10, 1);
        let old_b = collectible(1, 11, 1);
        let b = collectible(1, 11, 2);
        let c = collectible(3, 30, 1);
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1, 10), key_row(1, 11), key_row(1, 12)]])
            .append_query_results([
                vec![a.clone()],
                vec![old_b],
                vec![b.clone()], // INSERT ... RETURNING
                vec![c.clone()],
            ])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let mut observed = Vec::new();
        reconcile::<Entity, _, _, _, _, _>(
            &db,
            Column::Region,
            7,
            &conflict(),
            1,
            vec![a, b, c].into_iter(),
            |model| (model.entity_id, model.id),
            filter,
            |model, changed| observed.push((model.entity_id, model.id, changed)),
        )
        .await
        .unwrap();
        assert_eq!(observed, [(1, 10, false), (1, 11, true), (3, 30, true)]);
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 6);
        assert_eq!(
            log[0],
            Transaction::one(
                Entity::find()
                    .select_only()
                    .columns([Column::EntityId, Column::Id])
                    .filter(Column::Region.eq(7i16))
                    .build(DbBackend::Postgres)
            )
        );
        // Removing collectible 12 must not delete the retained collectibles 10/11.
        assert_eq!(
            log[5],
            Transaction::one(
                Entity::delete_many()
                    .filter(Column::Region.eq(7i16))
                    .filter(filter(&[(1, 12)]))
                    .build(DbBackend::Postgres)
            )
        );
    }

    #[tokio::test]
    async fn failed_comparison_does_not_delete_or_consume_rest_of_snapshot() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1, 10), key_row(2, 20)]])
            .append_query_errors([DbErr::Custom("read failed".into())])
            .into_connection();
        let mut consumed = 0;
        let incoming = vec![collectible(1, 10, 1), collectible(2, 20, 1)]
            .into_iter()
            .inspect(|_| consumed += 1);
        assert!(
            reconcile::<Entity, _, _, _, _, _>(
                &db,
                Column::Region,
                7,
                &conflict(),
                1,
                incoming,
                |model| (model.entity_id, model.id),
                filter,
                |_, _| {},
            )
            .await
            .is_err()
        );
        assert_eq!(consumed, 1);
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[tokio::test]
    async fn repeated_batches_reuse_key_storage_and_do_not_repeat_changes() {
        let rows = (1..=6).map(|id| collectible(id, 10, 2)).collect::<Vec<_>>();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([(1..=6).map(|id| key_row(id, 10)).collect::<Vec<_>>()])
            .append_query_results([
                vec![rows[0].clone(), collectible(2, 10, 1)],
                vec![rows[1].clone()], // first upsert
                vec![rows[2].clone(), collectible(4, 10, 1)],
                vec![rows[3].clone()], // second upsert
                vec![rows[4].clone(), collectible(6, 10, 1)],
                vec![rows[5].clone()], // third upsert
            ])
            .into_connection();
        let key_storage = std::sync::Mutex::new(Vec::new());
        let mut observed = Vec::new();
        reconcile::<Entity, _, _, _, _, _>(
            &db,
            Column::Region,
            7,
            &conflict(),
            2,
            rows.into_iter(),
            |model| (model.entity_id, model.id),
            |keys| {
                assert_eq!(keys.len(), 2);
                key_storage.lock().unwrap().push(keys.as_ptr() as usize);
                filter(keys)
            },
            |model, changed| observed.push((model.entity_id, changed)),
        )
        .await
        .unwrap();
        assert_eq!(
            observed,
            [
                (1, false),
                (2, true),
                (3, false),
                (4, true),
                (5, false),
                (6, true)
            ]
        );
        let storage = key_storage.into_inner().unwrap();
        assert_eq!(storage.len(), 3);
        assert!(storage.iter().all(|ptr| *ptr == storage[0]));
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 7);
        for (index, id) in [(2, 2), (4, 4), (6, 6)] {
            let mut expected = Entity::insert_many([collectible(id, 10, 2).into_active_model()])
                .on_conflict(conflict())
                .build(DbBackend::Postgres);
            // exec() adds primary-key RETURNING columns on PostgreSQL.
            expected.sql.push_str(" RETURNING \"entity_id\", \"id\"");
            assert_eq!(log[index], Transaction::one(expected));
        }
    }

    #[tokio::test]
    async fn partial_incoming_batch_error_preserves_prior_writes_and_skips_deletes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(99, 10)]])
            .append_query_results([vec![collectible(2, 10, 1)]]) // first batch upsert
            .into_connection();
        let mut consumed = 0;
        let incoming = stream::iter(
            vec![
                Ok(collectible(1, 10, 1)),
                Ok(collectible(2, 10, 1)),
                Ok(collectible(3, 10, 1)),
                Err(DbErr::Custom("snapshot interrupted".into())),
                Ok(collectible(4, 10, 1)),
            ]
            .into_iter()
            .inspect(|_| consumed += 1),
        );
        let mut observed = Vec::new();
        let result = reconcile_stream::<Entity, _, _, _, _, _>(
            &db,
            Column::Region,
            7,
            &conflict(),
            2,
            incoming,
            |model| (model.entity_id, model.id),
            filter,
            |model, _| observed.push(model.entity_id),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            DbErr::Custom("snapshot interrupted".into())
        );
        assert_eq!(consumed, 4);
        assert_eq!(
            observed,
            [1, 2],
            "partial batch must not be observed or written"
        );
        assert_eq!(db.into_transaction_log().len(), 2, "no stale DELETE");
    }

    #[tokio::test]
    async fn new_rows_skip_full_row_reads_and_writes_remain_batched() {
        let rows = (1..=5).map(|id| collectible(id, 10, 1)).collect::<Vec<_>>();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new(),
            ])
            .append_query_results([
                vec![rows[1].clone()],
                vec![rows[3].clone()],
                vec![rows[4].clone()],
            ])
            .into_connection();
        reconcile::<Entity, _, _, _, _, _>(
            &db,
            Column::Region,
            7,
            &conflict(),
            2,
            rows.into_iter(),
            |model| (model.entity_id, model.id),
            filter,
            |_, changed| assert!(changed),
        )
        .await
        .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 4, "only one key read and three batched writes");
        for entry in &log[1..] {
            assert!(format!("{entry:?}").contains("INSERT INTO"));
        }
    }

    #[tokio::test]
    async fn failed_key_read_does_not_consume_snapshot_or_delete_anything() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_errors([DbErr::Custom("read failed".into())])
            .into_connection();
        let incoming = std::iter::from_fn(|| -> Option<Model> {
            panic!("must not consume the snapshot after key lookup fails")
        });
        assert!(
            reconcile::<Entity, _, _, _, _, _>(
                &db,
                Column::Region,
                7,
                &conflict(),
                2,
                incoming,
                |model| (model.entity_id, model.id),
                filter,
                |_, _| {},
            )
            .await
            .is_err()
        );
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[tokio::test]
    async fn key_decode_failure_after_a_valid_key_does_not_consume_snapshot_or_delete() {
        let mut invalid_key = key_row(2, 20);
        invalid_key.insert("1".to_owned(), "not an integer".into());
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1, 10), invalid_key]])
            .into_connection();
        let incoming = std::iter::from_fn(|| -> Option<Model> {
            panic!("must not consume a snapshot after a partial key-stream failure")
        });
        assert!(
            reconcile::<Entity, _, _, _, _, _>(
                &db,
                Column::Region,
                7,
                &conflict(),
                2,
                incoming,
                |model| (model.entity_id, model.id),
                filter,
                |_, _| panic!("must not observe rows after a key-stream failure"),
            )
            .await
            .is_err()
        );
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1, "only the key SELECT may execute");
    }

    #[tokio::test]
    async fn partial_comparison_decode_failure_never_observes_writes_or_deletes() {
        let comparison_row = |id: i64, count: sea_orm::Value| {
            std::collections::BTreeMap::<String, sea_orm::Value>::from([
                ("entity_id".to_owned(), id.into()),
                ("id".to_owned(), 10i32.into()),
                ("activated".to_owned(), true.into()),
                ("count".to_owned(), count),
                ("region".to_owned(), 7i16.into()),
            ])
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1, 10), key_row(2, 10), key_row(9, 10)]])
            .append_query_results([vec![
                comparison_row(1, 1i32.into()),
                comparison_row(2, "invalid count".into()),
            ]])
            .into_connection();
        assert!(
            reconcile::<Entity, _, _, _, _, _>(
                &db,
                Column::Region,
                7,
                &conflict(),
                2,
                vec![collectible(1, 10, 2), collectible(2, 10, 2)].into_iter(),
                |model| (model.entity_id, model.id),
                filter,
                |_, _| panic!("partial comparison must not be observed"),
            )
            .await
            .is_err()
        );
        assert_eq!(
            db.into_transaction_log().len(),
            2,
            "only key and comparison SELECTs"
        );
    }

    #[tokio::test]
    async fn failed_upsert_does_not_run_stale_deletes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(2, 20)]])
            .append_query_errors([DbErr::Custom("write failed".into())])
            .into_connection();
        assert!(
            reconcile::<Entity, _, _, _, _, _>(
                &db,
                Column::Region,
                7,
                &conflict(),
                10,
                vec![collectible(1, 10, 1)].into_iter(),
                |model| (model.entity_id, model.id),
                filter,
                |_, _| {},
            )
            .await
            .is_err()
        );
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[tokio::test]
    async fn empty_snapshot_deletes_stale_keys_in_bounded_region_scoped_batches() {
        let rows = (0..1_001).map(|id| key_row(id, 10)).collect::<Vec<_>>();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([rows])
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 0,
                    rows_affected: 1_000,
                },
                MockExecResult {
                    last_insert_id: 0,
                    rows_affected: 1,
                },
            ])
            .into_connection();
        reconcile::<Entity, _, _, _, _, _>(
            &db,
            Column::Region,
            7,
            &conflict(),
            10,
            std::iter::empty(),
            |model| (model.entity_id, model.id),
            filter,
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(db.into_transaction_log().len(), 3);
    }

    #[tokio::test]
    async fn supports_single_primary_key_and_skips_unchanged_rows() {
        use entity::traveler_task_state as task;
        let model = task::Model {
            entity_id: 1,
            player_entity_id: 2,
            traveler_id: 3,
            task_id: 4,
            completed: false,
            region: 7,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![
                std::collections::BTreeMap::<String, sea_orm::Value>::from([(
                    "0".to_string(),
                    1i64.into(),
                )]),
            ]])
            .append_query_results([vec![model.clone()]])
            .into_connection();
        reconcile::<task::Entity, _, _, _, _, _>(
            &db,
            task::Column::Region,
            7,
            &OnConflict::column(task::Column::EntityId)
                .update_column(task::Column::Completed)
                .to_owned(),
            1,
            vec![model].into_iter(),
            |model| model.entity_id,
            |ids| Condition::all().add(task::Column::EntityId.is_in(ids.iter().copied())),
            |_, changed| assert!(!changed),
        )
        .await
        .unwrap();
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[test]
    fn composite_filter_groups_unsorted_keys_without_mutating_input() {
        let keys = [(2, 30), (-1, 20), (2, 10), (-1, 20), (3, 40), (-1, 10)];
        let original = keys;
        let query = Entity::find()
            .filter(filter(&keys))
            .build(DbBackend::Postgres);
        let expected = Condition::any()
            .add(
                Condition::all()
                    .add(Column::EntityId.eq(-1i64))
                    .add(Column::Id.is_in([10, 20, 20])),
            )
            .add(
                Condition::all()
                    .add(Column::EntityId.eq(2i64))
                    .add(Column::Id.is_in([10, 30])),
            )
            .add(
                Condition::all()
                    .add(Column::EntityId.eq(3i64))
                    .add(Column::Id.is_in([40])),
            );
        assert_eq!(
            query,
            Entity::find().filter(expected).build(DbBackend::Postgres)
        );
        assert_eq!(keys, original);
    }

    #[test]
    fn empty_composite_filter_preserves_empty_disjunction() {
        assert_eq!(
            Entity::find()
                .filter(filter(&[]))
                .build(DbBackend::Postgres),
            Entity::find()
                .filter(Condition::any())
                .build(DbBackend::Postgres)
        );
    }

    #[test]
    fn composite_filter_never_matches_cross_product_of_keys() {
        let query = Entity::find()
            .filter(filter(&[(1, 10), (2, 20)]))
            .build(DbBackend::Postgres)
            .to_string();
        assert!(
            query.contains("\"entity_id\" = 1 AND \"vault_state_collectibles\".\"id\" IN (10)")
        );
        assert!(
            query.contains("\"entity_id\" = 2 AND \"vault_state_collectibles\".\"id\" IN (20)")
        );
        assert!(!query.contains("IN (10, 20)"));
    }
}
