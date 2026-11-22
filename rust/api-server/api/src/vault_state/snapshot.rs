//! Each snapshot row is a complete vault, but vaults need not arrive in key order.
use super::bitcraft::collectible_keys_filter;
use crate::websocket::initial_sync::bigint_keys_filter;
use crate::websocket::snapshot_write::vault_collectibles;
use entity::{
    shared::Region,
    vault_state_collectibles::{Column as C, Entity as E, Model},
};
use futures::{Stream, TryStreamExt};
use game_module::module_bindings::{VaultCollectible, VaultState};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
};
use std::collections::{HashMap, HashSet};

const ENTITIES: usize = 128;
const COLLECTIBLES: usize = 32_768;

struct Comparison<'a> {
    db: &'a DatabaseConnection,
    region: Region,
    write_limit: usize,
    // Only query vaults known to exist in this region. New/moved keys still upsert.
    fetch: Vec<i64>,
    incoming: HashMap<(i64, i32), (bool, i32)>,
    stale: Vec<(i64, i32)>,
    reads: usize,
    writes: usize,
}

impl Comparison<'_> {
    fn add(
        &mut self,
        entity: i64,
        known: bool,
        rows: impl Iterator<Item = VaultCollectible>,
    ) -> Result<(), DbErr> {
        if known {
            self.fetch.push(entity);
        }
        for row in rows {
            if self
                .incoming
                .insert((entity, row.id), (row.activated, row.count))
                .is_some()
            {
                return Err(DbErr::Custom(
                    "duplicate collectible in vault snapshot".into(),
                ));
            }
        }
        Ok(())
    }

    // The optional ID bounds partition *one oversized vault* into complete,
    // disjoint comparisons, including DB-only IDs before/between/after input IDs.
    async fn flush(&mut self, lower: Option<i32>, upper: Option<i32>) -> Result<(), DbErr> {
        if !self.fetch.is_empty() {
            let mut query = E::find()
                .select_only()
                .columns([C::EntityId, C::Id, C::Activated, C::Count])
                .filter(C::Region.eq(self.region))
                .filter(bigint_keys_filter(C::EntityId, &self.fetch));
            if let Some(lo) = lower {
                query = query.filter(C::Id.gte(lo));
            }
            if let Some(hi) = upper {
                query = query.filter(C::Id.lt(hi));
            }
            let q = query.into_tuple::<(i64, i32, bool, i32)>();
            let mut existing = q.stream(self.db).await?;
            self.reads += 1;
            while let Some((entity, id, active, count)) = existing.try_next().await? {
                match self.incoming.entry((entity, id)) {
                    std::collections::hash_map::Entry::Occupied(e)
                        if *e.get() == (active, count) =>
                    {
                        e.remove();
                    }
                    std::collections::hash_map::Entry::Vacant(_) => self.stale.push((entity, id)),
                    _ => {} // changed: keep it in the upsert batch
                }
            }
            self.fetch.clear();
        } // Drop the DB stream / release its connection before writing.
        self.writes += self.incoming.len();
        let mut changed = self
            .incoming
            .drain()
            .map(|((entity_id, id), (activated, count))| Model {
                entity_id,
                id,
                activated,
                count,
                region: self.region,
            });
        let mut batch = Vec::new();
        loop {
            batch.extend(changed.by_ref().take(self.write_limit));
            if batch.is_empty() {
                break;
            }
            self.db.execute_raw(vault_collectibles(&batch)).await?;
            batch.clear();
        }
        Ok(())
    }
}

pub(super) async fn reconcile(
    db: &DatabaseConnection,
    region: Region,
    write_limit: usize,
    incoming: impl Stream<Item = Result<VaultState, DbErr>>,
) -> Result<(), DbErr> {
    let mut remaining: HashSet<i64> = E::find()
        .select_only()
        .column(C::EntityId)
        .distinct()
        .filter(C::Region.eq(region))
        .into_tuple::<i64>()
        .stream(db)
        .await?
        .try_collect()
        .await?;
    let mut seen = HashSet::new();
    let mut batch = Comparison {
        db,
        region,
        write_limit: write_limit.clamp(1, 3000),
        fetch: Vec::new(),
        incoming: HashMap::new(),
        stale: Vec::new(),
        reads: 0,
        writes: 0,
    };
    let mut entities = 0;
    futures::pin_mut!(incoming);
    while let Some(vault) = incoming.try_next().await? {
        let entity = vault.entity_id as i64;
        if !seen.insert(entity) {
            return Err(DbErr::Custom("duplicate entity in vault snapshot".into()));
        }
        let mut rows = vault.collectibles;
        if rows.is_empty() {
            continue;
        } // empty and absent vaults both delete all regional rows at EOF
        let known = remaining.remove(&entity);
        if entities == ENTITIES || rows.len() > COLLECTIBLES - batch.incoming.len() {
            batch.flush(None, None).await?;
            entities = 0;
        }
        if rows.len() <= COLLECTIBLES {
            batch.add(entity, known, rows.into_iter())?;
            entities += 1;
        } else {
            rows.sort_unstable_by_key(|r| r.id);
            if rows.windows(2).any(|w| w[0].id == w[1].id) {
                return Err(DbErr::Custom(
                    "duplicate collectible in vault snapshot".into(),
                ));
            }
            let mut rows = rows.into_iter().peekable();
            let mut lower = None;
            while rows.peek().is_some() {
                batch.add(entity, known, rows.by_ref().take(COLLECTIBLES))?;
                let upper = rows.peek().map(|r| r.id);
                batch.flush(lower, upper).await?;
                lower = upper;
            }
        }
    }
    batch.flush(None, None).await?;
    // Only reachable after the explicit Complete marker (or complete owned
    // vector) and all comparisons/upserts. Never delete a row now in another region.
    for keys in batch.stale.chunks(1000) {
        E::delete_many()
            .filter(C::Region.eq(region))
            .filter(collectible_keys_filter(keys))
            .exec(db)
            .await?;
    }
    let absent = remaining.len();
    let mut ids = remaining.into_iter();
    loop {
        let keys: Vec<_> = ids.by_ref().take(1000).collect();
        if keys.is_empty() {
            break;
        }
        E::delete_many()
            .filter(C::Region.eq(region))
            .filter(bigint_keys_filter(C::EntityId, &keys))
            .exec(db)
            .await?;
    }
    tracing::debug!(
        region,
        vaults = seen.len(),
        comparison_selects = batch.reads,
        upserted_rows = batch.writes,
        stale_keys = batch.stale.len(),
        absent_or_empty = absent,
        "Vault snapshot reconciled"
    );
    Ok(())
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
