use crate::AppState;
use crate::websocket::{
    SpacetimeUpdateMessages as Message,
    batched_worker::BatchedWorker,
    initial_sync::{flush_before_snapshot, reconcile_cached, update_cache_if_changed},
    worker_channel::{UnboundedReceiver, UnboundedSender, shutdown_channel},
};
use dashmap::DashMap;
use entity::{claim_state, claim_tech_desc, shared::Region};
use game_module::module_bindings::{ClaimState, ClaimTechDesc};
use sea_orm::{
    ColumnTrait, Condition, DbErr, EntityTrait, IntoActiveModel, Iterable, PrimaryKeyToColumn,
    TryGetableMany, Value, sea_query::OnConflict,
};
use std::{collections::HashMap, hash::Hash, sync::Arc, time::Duration};

pub(crate) type ClaimStateWorker = CachedClaimWorker<ClaimState, claim_state::Entity, i64>;
pub(crate) type ClaimTechDescWorker =
    CachedClaimWorker<ClaimTechDesc, claim_tech_desc::Entity, i32>;

/// One ordered writer per table: a reconnect snapshot cannot race an earlier
/// live DB batch. Keep only the last operation per primary key in a batch.
pub(crate) struct CachedClaimWorker<T, E: EntityTrait, K> {
    rx: UnboundedReceiver<Message<T>>,
    tx: UnboundedSender<Message<T>>,
    state: AppState,
    batch: usize,
    delay: Duration,
    pending: HashMap<K, Option<E::Model>>,
    cache: Arc<DashMap<K, E::Model>>,
    pk: E::Column,
    conflict: OnConflict,
    convert: fn(T, Region) -> E::Model,
    key: fn(&E::Model) -> K,
    region: Option<(E::Column, fn(&E::Model) -> Region)>,
}

impl ClaimStateWorker {
    pub(crate) fn new(state: AppState, batch: usize, delay: Duration) -> Self {
        Self::create(
            state.claim_state.clone(),
            state,
            batch,
            delay,
            claim_state::Column::EntityId,
            |row, region| {
                claim_state::ModelBuilder::new(row)
                    .with_region(region)
                    .build()
            },
            |m| m.entity_id,
            Some((claim_state::Column::Region, |m| m.region)),
        )
    }
}

impl ClaimTechDescWorker {
    pub(crate) fn new(state: AppState, batch: usize, delay: Duration) -> Self {
        Self::create(
            state.claim_tech_desc.clone(),
            state,
            batch,
            delay,
            claim_tech_desc::Column::Id,
            |row, _| row.into(),
            |m| m.id,
            None,
        )
    }
}

impl<T: Send + 'static, E, K> CachedClaimWorker<T, E, K>
where
    E: EntityTrait,
    E::Model: Sync + PartialEq + IntoActiveModel<E::ActiveModel>,
    K: Copy + Eq + Hash + TryGetableMany + Into<Value> + Send + Sync,
{
    #[allow(clippy::too_many_arguments)]
    fn create(
        cache: Arc<DashMap<K, E::Model>>,
        state: AppState,
        batch: usize,
        delay: Duration,
        pk: E::Column,
        convert: fn(T, Region) -> E::Model,
        key: fn(&E::Model) -> K,
        region: Option<(E::Column, fn(&E::Model) -> Region)>,
    ) -> Self {
        let (tx, rx) = shutdown_channel(&state.shutdown);
        let conflict = OnConflict::column(pk)
            .update_columns(E::Column::iter().filter(|c| E::PrimaryKey::from_column(*c).is_none()))
            .to_owned();
        Self {
            rx,
            tx,
            state,
            batch,
            delay,
            cache,
            pk,
            convert,
            key,
            region,
            conflict,
            pending: HashMap::new(),
        }
    }

    async fn persist(&mut self) -> Result<(), DbErr> {
        let mut upserts = Vec::new();
        let mut deletes = Vec::new();
        for (id, row) in self.pending.drain() {
            match row {
                Some(row) => upserts.push(row.into_active_model()),
                None => deletes.push(id),
            }
        }
        flush_before_snapshot::<E, _>(
            &self.state.conn,
            &self.conflict,
            &mut upserts,
            &mut deletes,
            |ids| Condition::all().add(self.pk.is_in(ids.iter().copied())),
        )
        .await
    }
}

impl<T: Send + 'static, E, K> BatchedWorker for CachedClaimWorker<T, E, K>
where
    E: EntityTrait,
    E::ActiveModel: Send,
    E::Model: Sync + PartialEq + IntoActiveModel<E::ActiveModel>,
    K: Copy + Eq + Hash + TryGetableMany + Into<Value> + Send + Sync,
{
    type Entity = T;
    fn rx(&mut self) -> &mut UnboundedReceiver<Message<T>> {
        &mut self.rx
    }
    fn tx(&self) -> UnboundedSender<Message<T>> {
        self.tx.clone()
    }
    fn worker_name(&self) -> &'static str {
        E::default().table_name()
    }
    fn batch_delay(&self) -> Duration {
        self.delay
    }
    fn should_flush(&self) -> bool {
        self.pending.len() >= self.batch
    }
    fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }
    fn reset_batch(&mut self) {}
    async fn flush(&mut self) {
        if let Err(error) = self.persist().await {
            tracing::error!(%error, table = E::default().table_name(), "Could not persist live changes");
        }
    }
    async fn handle_message(&mut self, msg: Message<T>) {
        match msg {
            Message::Initial {
                data,
                database_name,
                ..
            } => {
                if let Err(error) = self.persist().await {
                    tracing::error!(%error, table = E::default().table_name(), "Could not flush before snapshot");
                    return;
                }
                let mut scope = Condition::all();
                if let Some((column, _)) = self.region {
                    scope = scope.add(column.eq(database_name));
                }
                if let Err(error) = reconcile_cached::<E, _>(
                    &self.state.conn,
                    &self.conflict,
                    self.batch,
                    data.into_iter()
                        .map(|row| (self.convert)(row, database_name)),
                    self.key,
                    |ids| Condition::all().add(self.pk.is_in(ids.iter().copied())),
                    &self.cache,
                    scope,
                    |model| {
                        self.region
                            .is_none_or(|(_, region)| region(model) == database_name)
                    },
                )
                .await
                {
                    tracing::error!(%error, table = E::default().table_name(), "Could not reconcile snapshot");
                }
            }
            Message::Insert {
                new, database_name, ..
            }
            | Message::Update {
                new, database_name, ..
            } => {
                let model = (self.convert)(new, database_name);
                let id = (self.key)(&model);
                update_cache_if_changed(&self.cache, id, &model);
                self.pending.insert(id, Some(model));
            }
            Message::Remove {
                delete,
                database_name,
                ..
            } => {
                let id = (self.key)(&(self.convert)(delete, database_name));
                self.cache.remove(&id);
                self.pending.insert(id, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::ClaimTechType;
    use sea_orm::{DbBackend, MockDatabase, MockExecResult};
    use std::collections::BTreeMap;

    fn tech(id: i32, name: &str) -> ClaimTechDesc {
        ClaimTechDesc {
            id,
            name: name.into(),
            description: String::new(),
            tier: 0,
            tech_type: ClaimTechType::Mining,
            supplies_cost: 1,
            research_time: 1,
            requirements: vec![],
            input: vec![],
            members: 0,
            area: 0,
            supplies: 0,
            xp_to_mint_hex_coin: 0,
            unlocks_techs: vec![42],
            cannot_be_researched: false,
            blocking_claim_techs: None,
            not_shown_in_ui: false,
        }
    }

    #[tokio::test]
    #[ignore = "requires SHARED_SYNC_TEST_URL; uses a PostgreSQL temporary table"]
    async fn claim_tech_reconnect_updates_existing_postgres_row_and_cache() -> anyhow::Result<()> {
        use sea_orm::{ConnectOptions, ConnectionTrait, Database, Schema};
        let mut options = ConnectOptions::new(std::env::var("SHARED_SYNC_TEST_URL")?);
        options.max_connections(1).set_schema_search_path("pg_temp");
        let db = Database::connect(options).await?;
        let mut table =
            Schema::new(DbBackend::Postgres).create_table_from_entity(claim_tech_desc::Entity);
        db.execute(table.temporary()).await?;
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let mut worker = ClaimTechDescWorker::new(state.clone(), 2, Duration::from_secs(1));
        for row in [tech(1, "before"), tech(2, "removed offline")] {
            worker
                .handle_message(Message::Insert {
                    new: row,
                    database_name: 0,
                    event: None,
                })
                .await;
        }
        // Both live writes are still buffered when the newer snapshot arrives.
        let mut latest = tech(1, "after");
        latest.tech_type = ClaimTechType::Forestry;
        latest.unlocks_techs = vec![17, 18];
        worker
            .handle_message(Message::Initial {
                data: vec![latest.clone()],
                database_name: 0,
                batches: None,
            })
            .await;
        let expected: claim_tech_desc::Model = latest.into();
        assert_eq!(
            claim_tech_desc::Entity::find().all(&db).await?,
            vec![expected.clone()]
        );
        assert_eq!(*state.claim_tech_desc.get(&1).unwrap(), expected);
        assert!(!state.claim_tech_desc.contains_key(&2));
        Ok(())
    }

    #[tokio::test]
    async fn claim_tech_snapshot_then_live_rename_and_delete() {
        let id: BTreeMap<String, Value> = [("id".into(), 1.into())].into();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![], vec![id.clone()], vec![id]])
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        state
            .claim_tech_desc
            .insert(9, tech(9, "cache only").into());
        let mut worker = ClaimTechDescWorker::new(state.clone(), 100, Duration::from_secs(1));
        let row = tech(1, "original");
        worker
            .handle_message(Message::Initial {
                data: vec![row.clone()],
                database_name: 0,
                batches: None,
            })
            .await;
        assert!(!state.claim_tech_desc.contains_key(&9));
        assert_eq!(state.claim_tech_desc.get(&1).unwrap().name, "original");
        let mut new = tech(1, "renamed");
        new.tech_type = ClaimTechType::Forestry;
        new.unlocks_techs = vec![17, 18];
        worker
            .handle_message(Message::Update {
                old: row,
                new: new.clone(),
                database_name: 0,
                event: None,
            })
            .await;
        worker.persist().await.unwrap();
        assert_eq!(*state.claim_tech_desc.get(&1).unwrap(), new.clone().into());
        worker
            .handle_message(Message::Remove {
                delete: new,
                database_name: 0,
                event: None,
            })
            .await;
        worker.persist().await.unwrap();
        assert!(!state.claim_tech_desc.contains_key(&1));
        let log = db.into_transaction_log();
        for entry in &log[1..3] {
            let update = entry.statements()[0]
                .sql
                .split("DO UPDATE SET")
                .nth(1)
                .unwrap();
            for col in ["name", "tech_type", "unlocks_techs"] {
                assert!(update.contains(&format!("\"{col}\"")), "{update}");
            }
        }
        assert!(log[3].statements()[0].sql.starts_with("DELETE"));
    }
}
