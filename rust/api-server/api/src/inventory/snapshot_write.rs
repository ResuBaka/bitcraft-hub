use entity::inventory::Model;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use std::num::NonZeroUsize;

#[cfg(test)]
mod tests;

// Upper estimates for compact JSON, including a separator. A pocket has only
// fixed field names, integers, booleans and a two-variant enum (no free text).
// The base allows for the scalar columns, array framing and bind lengths.
fn estimated_bytes(row: &Model) -> usize {
    row.pockets.iter().fold(128usize, |size, p| {
        size.saturating_add(if p.contents.is_some() { 144 } else { 56 })
    })
}

fn next_write(rows: &[Model], limit: NonZeroUsize) -> (usize, usize) {
    crate::websocket::snapshot_write::write_prefix(rows, limit, estimated_bytes)
}

const UPSERT: &str = r#"
INSERT INTO inventory (entity_id, pockets, inventory_index, cargo_index, owner_entity_id, player_owner_entity_id, region)
SELECT id, pocket_json::jsonb, inv_idx, cargo_idx, owner_id, player_owner_id, rgn
FROM unnest($1::bigint[], $2::text[], $3::integer[], $4::integer[], $5::bigint[], $6::bigint[], $7::smallint[])
    AS batch(id, pocket_json, inv_idx, cargo_idx, owner_id, player_owner_id, rgn)
ON CONFLICT (entity_id) DO UPDATE SET
    pockets = EXCLUDED.pockets, inventory_index = EXCLUDED.inventory_index,
    cargo_index = EXCLUDED.cargo_index, owner_entity_id = EXCLUDED.owner_entity_id,
    player_owner_entity_id = EXCLUDED.player_owner_entity_id, region = EXCLUDED.region"#;

fn upsert_statement(rows: impl ExactSizeIterator<Item = Model>) -> Result<Statement, DbErr> {
    let n = rows.len();
    let mut ids = Vec::with_capacity(n);
    let mut pockets = Vec::with_capacity(n);
    let mut inventory = Vec::with_capacity(n);
    let mut cargo = Vec::with_capacity(n);
    let mut owners = Vec::with_capacity(n);
    let mut players = Vec::with_capacity(n);
    let mut regions = Vec::with_capacity(n);
    for row in rows {
        ids.push(row.entity_id);
        // Bind compact JSON text rather than materializing a serde_json::Value
        // tree for each pocket. Postgres stores exactly one JSONB array per row.
        pockets.push(serde_json::to_string(&row.pockets).map_err(|e| DbErr::Json(e.to_string()))?);
        inventory.push(row.inventory_index);
        cargo.push(row.cargo_index);
        owners.push(row.owner_entity_id);
        players.push(row.player_owner_entity_id);
        regions.push(row.region);
    }
    Ok(Statement::from_sql_and_values(
        DbBackend::Postgres,
        UPSERT,
        [
            ids.into(),
            pockets.into(),
            inventory.into(),
            cargo.into(),
            owners.into(),
            players.into(),
            regions.into(),
        ],
    ))
}

/// `changes` is the output of one row-limited comparison batch. Consume it
/// in order without cloning pockets or retaining an encoded copy to size it.
pub(super) async fn write_changes(
    db: &impl ConnectionTrait,
    changes: &mut Vec<Model>,
    limit: NonZeroUsize,
) -> Result<(), DbErr> {
    let mut remaining = changes.drain(..);
    while !remaining.as_slice().is_empty() {
        let (count, bytes) = next_write(remaining.as_slice(), limit);
        if bytes > limit.get() {
            metrics::counter!("inventory_snapshot_oversize_rows_total").increment(1);
        }
        db.execute_raw(upsert_statement(remaining.by_ref().take(count))?)
            .await?;
    }
    Ok(())
}
