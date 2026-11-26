use super::resource_sync::Changes;
use entity::shared::Region;
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, DbBackend, FromQueryResult, Statement,
    sqlx::{self, PgConnection},
};
use std::collections::{HashMap, HashSet};

pub(super) const BASE: i32 = 48;
pub(super) const MAX_LEVEL: i32 = 8;
const BATCH: usize = 3_000;
type Bin = (i32, i32, i32); // resource ID, base X, base Z
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Delta {
    count: i32,
    x2: i64,
    z: i64,
}

type Deltas = HashMap<Bin, Delta>;

pub(super) async fn rebuild(txn: &mut PgConnection, region: Region) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM atlas_resource_cluster WHERE region = $1")
        .bind(region)
        .execute(&mut *txn)
        .await?;
    sqlx::query(
        "WITH base AS MATERIALIZED (
             SELECT resource_id, x / $2 AS bx, z / $2 AS bz, count(*) AS n,
                    sum(2 * x::bigint + (z & 1)) AS sx2, sum(z) AS sz
             FROM atlas_resource_stage WHERE x >= 0 AND z >= 0
             GROUP BY resource_id, bx, bz
         )
         INSERT INTO atlas_resource_cluster
             (region, level, resource_id, bucket_x, bucket_z, node_count, sum_x2, sum_z)
         SELECT $1, level, resource_id, bx / (1 << level), bz / (1 << level),
                sum(n)::integer, sum(sx2)::bigint, sum(sz)::bigint
         FROM base CROSS JOIN generate_series(0, $3::integer) AS levels(level)
         GROUP BY level, resource_id, bx / (1 << level), bz / (1 << level)",
    )
    .bind(region)
    .bind(BASE)
    .bind(MAX_LEVEL)
    .execute(txn)
    .await?;
    Ok(())
}

#[derive(FromQueryResult)]
struct Entry {
    entity_id: i64,
    resource_id: Option<i32>,
    x: Option<i32>,
    z: Option<i32>,
}

fn sample(resource: Option<i32>, position: Option<(i32, i32)>) -> Option<(Bin, i64, i64)> {
    let (x, z) = position?;
    // Store double X to retain the staggered hex rows with integer sums.
    (x >= 0 && z >= 0).then_some((
        (resource?, x / BASE, z / BASE),
        i64::from(x) * 2 + i64::from(z & 1),
        i64::from(z),
    ))
}

fn apply_changes(entry: Entry, changes: &Changes, deltas: &mut Deltas) {
    let id = entry.entity_id as u64;
    let old_position = entry.x.zip(entry.z);
    let old = sample(entry.resource_id, old_position);
    let new = sample(
        changes
            .resources
            .get(&id)
            .copied()
            .unwrap_or(entry.resource_id),
        changes.locations.get(&id).copied().unwrap_or(old_position),
    );
    if old == new {
        return;
    }
    if let Some((bin, x2, z)) = old {
        let delta = deltas.entry(bin).or_default();
        delta.count -= 1;
        delta.x2 -= x2;
        delta.z -= z;
    }
    if let Some((bin, x2, z)) = new {
        let delta = deltas.entry(bin).or_default();
        delta.count += 1;
        delta.x2 += x2;
        delta.z += z;
    }
}

// Read just the changed rows before the projection writes. The same transaction
// commits both node records and their related overview counts.
pub(super) async fn deltas(
    txn: &DatabaseTransaction,
    region: Region,
    changes: &Changes,
) -> Result<Deltas, sea_orm::DbErr> {
    let ids: Vec<i64> = changes
        .resources
        .keys()
        .chain(changes.locations.keys())
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|id| id as i64)
        .collect();
    let mut deltas = Deltas::new();
    for batch in ids.chunks(BATCH) {
        let rows = Entry::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT entity_id, resource_id, x, z FROM atlas_resource_entry
             WHERE region = $1 AND entity_id = ANY($2::bigint[])",
            [region.into(), batch.to_vec().into()],
        ))
        .all(txn)
        .await?;
        let mut unseen: HashSet<_> = batch.iter().copied().collect();
        for entry in rows {
            unseen.remove(&entry.entity_id);
            apply_changes(entry, changes, &mut deltas);
        }
        for entity_id in unseen {
            apply_changes(
                Entry {
                    entity_id,
                    resource_id: None,
                    x: None,
                    z: None,
                },
                changes,
                &mut deltas,
            );
        }
    }
    deltas.retain(|_, delta| delta.count != 0 || delta.x2 != 0 || delta.z != 0);
    Ok(deltas)
}

pub(super) async fn write(
    txn: &DatabaseTransaction,
    region: Region,
    deltas: Deltas,
) -> Result<(), sea_orm::DbErr> {
    if deltas.is_empty() {
        return Ok(());
    }
    let deltas: Vec<_> = deltas.into_iter().collect();
    for batch in deltas.chunks(BATCH) {
        let mut ids = Vec::new();
        let mut xs = Vec::new();
        let mut zs = Vec::new();
        let mut counts = Vec::new();
        let mut sum_x2 = Vec::new();
        let mut sum_z = Vec::new();
        for &((id, x, z), delta) in batch {
            ids.push(id);
            xs.push(x);
            zs.push(z);
            counts.push(delta.count);
            sum_x2.push(delta.x2);
            sum_z.push(delta.z);
        }
        txn.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO atlas_resource_cluster AS dst
                 (region, level, resource_id, bucket_x, bucket_z, node_count, sum_x2, sum_z)
             SELECT $1, level, id, x / (1 << level), z / (1 << level),
                    sum(delta)::integer, sum(dx2)::bigint, sum(dz)::bigint
             FROM unnest($2::integer[], $3::integer[], $4::integer[], $5::integer[],
                         $6::bigint[], $7::bigint[]) AS d(id, x, z, delta, dx2, dz)
             CROSS JOIN generate_series(0, $8::integer) AS levels(level)
             GROUP BY level, id, x / (1 << level), z / (1 << level)
             HAVING sum(delta) <> 0 OR sum(dx2) <> 0 OR sum(dz) <> 0
             ON CONFLICT (region, level, resource_id, bucket_x, bucket_z)
             DO UPDATE SET node_count = dst.node_count + EXCLUDED.node_count,
                           sum_x2 = dst.sum_x2 + EXCLUDED.sum_x2, sum_z = dst.sum_z + EXCLUDED.sum_z",
            [region.into(), ids.into(), xs.into(), zs.into(), counts.into(), sum_x2.into(), sum_z.into(), MAX_LEVEL.into()],
        )).await?;
    }
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM atlas_resource_cluster WHERE region = $1 AND node_count = 0",
        [region.into()],
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_track_moves_type_changes_and_depletion() {
        let mut changes = Changes::default();
        changes.locations.insert(1, Some((48, 96)));
        changes.resources.insert(2, None);
        changes.resources.insert(3, Some(6));
        changes.locations.insert(3, Some((1, 1)));
        let mut result = Deltas::new();
        for (entity_id, resource_id, x, z) in [
            (1, Some(7), Some(47), Some(96)),
            (2, Some(8), Some(96), Some(0)),
            (3, None, None, None),
        ] {
            apply_changes(
                Entry {
                    entity_id,
                    resource_id,
                    x,
                    z,
                },
                &changes,
                &mut result,
            );
        }
        assert_eq!(
            result,
            HashMap::from([
                (
                    (7, 0, 2),
                    Delta {
                        count: -1,
                        x2: -94,
                        z: -96
                    }
                ),
                (
                    (7, 1, 2),
                    Delta {
                        count: 1,
                        x2: 96,
                        z: 96
                    }
                ),
                (
                    (8, 2, 0),
                    Delta {
                        count: -1,
                        x2: -192,
                        z: 0
                    }
                ),
                (
                    (6, 0, 0),
                    Delta {
                        count: 1,
                        x2: 3,
                        z: 1
                    }
                )
            ])
        );
    }
}
