//! Changed-row writers for complete snapshots. Reconciliation owns the scope,
//! comparison, ordering, row limit and stale-key handling.
use migration::OnConflict;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, EntityTrait, IntoActiveModel, Statement};
use std::num::NonZeroUsize;

// Each expression supplies one typed bind array in INSERT column order.
macro_rules! arrays {
    ($rows:ident, $sql:literal, |$r:ident| $($field:expr),+ $(,)?) => {
        Statement::from_sql_and_values(DbBackend::Postgres, $sql, [
            $($rows.iter().map(|$r| $field).collect::<Vec<_>>().into()),+
        ])
    };
}

mod custom;
mod regional;
pub(crate) use custom::{extraction_writer, players};
pub(crate) use regional::{
    buildings, deployable_writer, experience, progressive_actions, traveler_tasks,
};

#[cfg(test)]
mod replay;

pub(crate) enum Writer<'a, E: EntityTrait> {
    Orm(&'a OnConflict),
    Fixed(fn(&[E::Model]) -> Statement),
    ByteLimited {
        statement: fn(&[E::Model]) -> Result<Statement, DbErr>,
        estimate: fn(&E::Model) -> usize,
        limit: NonZeroUsize,
    },
}

/// Largest prefix that fits, except that a single oversized row makes progress.
pub(crate) fn write_prefix<T>(
    rows: &[T],
    limit: NonZeroUsize,
    estimate: impl Fn(&T) -> usize,
) -> (usize, usize) {
    let mut bytes = 0usize;
    for (i, row) in rows.iter().enumerate() {
        let next = bytes.saturating_add(estimate(row));
        if i != 0 && next > limit.get() {
            return (i, bytes);
        }
        bytes = next;
    }
    (rows.len(), bytes)
}

impl<'a, E: EntityTrait> From<&'a OnConflict> for Writer<'a, E> {
    fn from(conflict: &'a OnConflict) -> Self {
        Self::Orm(conflict)
    }
}

impl<E: EntityTrait> Writer<'_, E>
where
    E::Model: IntoActiveModel<E::ActiveModel>,
{
    pub(crate) async fn write(
        &self,
        db: &impl ConnectionTrait,
        changes: &mut Vec<E::Model>,
    ) -> Result<(), DbErr> {
        if changes.is_empty() {
            return Ok(());
        }
        match self {
            Self::Orm(conflict) => {
                E::insert_many(changes.drain(..).map(IntoActiveModel::into_active_model))
                    .on_conflict((*conflict).clone())
                    .exec(db)
                    .await?;
            }
            Self::Fixed(statement) => {
                let stmt = statement(changes);
                changes.clear();
                db.execute_raw(stmt).await?;
            }
            Self::ByteLimited {
                statement,
                estimate,
                limit,
            } => {
                let mut remaining = changes.drain(..);
                while !remaining.as_slice().is_empty() {
                    let (count, bytes) = write_prefix(remaining.as_slice(), *limit, estimate);
                    if bytes > limit.get() {
                        metrics::counter!("snapshot_oversize_rows_total",
                            "table" => E::default().table_name().to_owned())
                        .increment(1);
                    }
                    let stmt = statement(&remaining.as_slice()[..count])?;
                    remaining.by_ref().take(count).for_each(drop);
                    db.execute_raw(stmt).await?;
                }
            }
        }
        Ok(())
    }
}

// All columns have fixed maximum wire widths. At the existing 3,000-row
// limit even the wider craft statement has < 300 KiB of encoded bind payload.
pub(crate) fn passive_crafts(rows: &[entity::passive_craft_state::Model]) -> Statement {
    // SeaORM's array binder expects non-null elements. A presence array keeps
    // NULL distinct from every possible slot value (including zero).
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO passive_craft_state (entity_id, owner_entity_id, recipe_id, building_entity_id, "timestamp", status, slot, region)
SELECT id, owner, recipe, building, ts, st, CASE WHEN has_slot THEN slot_value END, rgn
FROM unnest($1::bigint[], $2::bigint[], $3::integer[], $4::bigint[], $5::timestamptz[],
            $6::smallint[], $7::bigint[], $8::boolean[], $9::smallint[])
    AS b(id, owner, recipe, building, ts, st, slot_value, has_slot, rgn)
ON CONFLICT (entity_id) DO UPDATE SET owner_entity_id = EXCLUDED.owner_entity_id,
    recipe_id = EXCLUDED.recipe_id, building_entity_id = EXCLUDED.building_entity_id,
    "timestamp" = EXCLUDED."timestamp", status = EXCLUDED.status, slot = EXCLUDED.slot, region = EXCLUDED.region"#,
        [
            rows.iter().map(|r| r.entity_id).collect::<Vec<_>>().into(),
            rows.iter()
                .map(|r| r.owner_entity_id)
                .collect::<Vec<_>>()
                .into(),
            rows.iter().map(|r| r.recipe_id).collect::<Vec<_>>().into(),
            rows.iter()
                .map(|r| r.building_entity_id)
                .collect::<Vec<_>>()
                .into(),
            rows.iter().map(|r| r.timestamp).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.status).collect::<Vec<_>>().into(),
            rows.iter()
                .map(|r| r.slot.unwrap_or_default())
                .collect::<Vec<_>>()
                .into(),
            rows.iter()
                .map(|r| r.slot.is_some())
                .collect::<Vec<_>>()
                .into(),
            rows.iter().map(|r| r.region).collect::<Vec<_>>().into(),
        ],
    )
}

pub(crate) fn vault_collectibles(rows: &[entity::vault_state_collectibles::Model]) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO vault_state_collectibles (entity_id, id, activated, count, region)
SELECT * FROM unnest($1::bigint[], $2::integer[], $3::boolean[], $4::integer[], $5::smallint[])
ON CONFLICT (entity_id, id) DO UPDATE SET activated = EXCLUDED.activated,
    count = EXCLUDED.count, region = EXCLUDED.region"#,
        [
            rows.iter().map(|r| r.entity_id).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.id).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.activated).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.count).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.region).collect::<Vec<_>>().into(),
        ],
    )
}
