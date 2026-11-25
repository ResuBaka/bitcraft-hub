use crate::AppState;
use sea_orm::{DbBackend, DbErr, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardTool {
    item_id: i32,
    name: String,
    icon_asset_name: String,
    rarity: entity::shared::JsonRarity,
}

#[derive(FromQueryResult)]
struct EquippedItem {
    player_id: i64,
    skill_id: i32,
    item_id: i32,
}

/// Read only the requested skill pockets, in one query across all visible boards.
pub(super) async fn equipped_tools(
    state: &AppState,
    players_and_skills: Vec<(i64, i32)>,
) -> Result<HashMap<(i64, i32), LeaderboardTool>, DbErr> {
    let slots = state.player_stats.tool_slots_by_skill();
    let mut players = Vec::new();
    let mut skills = Vec::new();
    let mut pockets = Vec::new();
    for (player, skill) in players_and_skills {
        if let Some(&slot) = slots.get(&skill) {
            players.push(player);
            skills.push(skill);
            pockets.push(slot);
        }
    }
    if players.is_empty() {
        return Ok(HashMap::new());
    }
    let items = EquippedItem::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT player_id, skill_id, (i.pockets -> slot -> 'contents' ->> 'item_id')::int AS item_id
         FROM unnest($1::bigint[], $2::int[], $3::int[]) AS requested(player_id, skill_id, slot)
         JOIN inventory i ON i.owner_entity_id = player_id AND i.inventory_index = 1
         WHERE i.pockets -> slot -> 'contents' ->> 'item_type' = 'Item'",
        [players.into(), skills.into(), pockets.into()],
    )).all(&state.conn).await?;
    Ok(items.into_iter().filter_map(|row| {
        let item = state.item_desc.get(&row.item_id)?;
        Some(((row.player_id, row.skill_id), LeaderboardTool {
            item_id: row.item_id,
            name: item.name.clone(),
            icon_asset_name: item.icon_asset_name.clone(),
            rarity: item.rarity.clone(),
        }))
    }).collect())
}
