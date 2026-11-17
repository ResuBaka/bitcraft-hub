use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use entity::{
    building_state, claim_member_state, claim_state, passive_craft_state, player_username_state,
    progressive_action_state, public_progressive_action_state,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// The item page uses the produced item for {0} and an ingredient for {1}.
// Extend that convention to further ingredients and retain unknown placeholders.
fn format_template(template: &str, values: &[Option<String>]) -> String {
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let placeholder = &rest[..=end];
        let replacement = rest[1..end]
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index))
            .and_then(|value| value.as_deref());
        result.push_str(replacement.unwrap_or(placeholder));
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

fn recipe_name(state: &AppState, recipe: &entity::crafting_recipe::Model) -> String {
    use entity::inventory::ItemType;
    let resolve = |id, kind: &ItemType| match kind {
        ItemType::Item => state.item_desc.get(&id).map(|item| item.name.clone()),
        ItemType::Cargo => state.cargo_desc.get(&id).map(|cargo| cargo.name.clone()),
    };
    let mut values = vec![
        recipe
            .crafted_item_stacks
            .first()
            .and_then(|item| resolve(item.item_id, &item.item_type)),
    ];
    values.extend(
        recipe
            .consumed_item_stacks
            .iter()
            .map(|item| resolve(item.item_id, &item.item_type)),
    );
    format_template(&recipe.name, &values)
}

#[derive(Serialize)]
pub(crate) struct ClaimCraft {
    id: String,
    building_id: String,
    building_name: String,
    building_icon_asset_name: Option<String>,
    building_function_types: Vec<i32>,
    building_context: Option<BuildingContext>,
    owner_id: String,
    owner_name: Option<String>,
    is_member: bool,
    recipe_id: i32,
    recipe_name: Option<String>,
    outputs: Vec<CraftIcon>,
    kind: &'static str,
    status: &'static str,
    quantity: i32,
    progress: Option<i32>,
    progress_total: Option<i64>,
    public: Option<bool>,
    slot: Option<i64>,
    timestamp: Option<String>,
    duration_seconds: Option<f32>,
    hex_coin_estimate: HexCoinEstimate,
}

/// Estimate assumes one non-critical progress point per hit, no competing XP,
/// and no change to claim technology. Actual minting is shared across the claim.
#[derive(Serialize)]
struct HexCoinEstimate {
    estimated_remaining_coins: Option<u64>,
    xp_per_coin: Option<u32>,
    claim_xp_remainder: Option<u32>,
    assumed_xp_per_hit: Option<u64>,
}

fn mint_threshold(
    learned: &[i32],
    mut lookup: impl FnMut(i32) -> Option<u32>,
) -> Option<Option<u32>> {
    let mut minimum = u32::MAX;
    for id in learned {
        let xp = lookup(*id)?;
        if xp > 0 {
            minimum = minimum.min(xp);
        }
    }
    Some((minimum != u32::MAX).then_some(minimum))
}

fn estimate_hex_coins(
    remaining: Option<i64>,
    xp_per_progress: Option<f32>,
    threshold: Option<Option<u32>>,
    remainder: Option<u32>,
    eligible: Option<bool>,
) -> HexCoinEstimate {
    let xp_per_coin = threshold.flatten();
    let assumed_xp_per_hit = xp_per_progress
        .filter(|xp| xp.is_finite() && *xp >= 0.0 && (*xp as f64).ceil() < u64::MAX as f64)
        .map(|xp| xp.ceil() as u64);
    let estimated_remaining_coins = match (eligible, remaining, threshold) {
        (Some(false), _, _) | (_, Some(0), _) | (_, _, Some(None)) => Some(0),
        (Some(true), Some(remaining), Some(Some(rate))) if rate > 0 && remaining > 0 => {
            assumed_xp_per_hit
                .and_then(|xp| (remaining as u64).checked_mul(xp))
                .and_then(|xp| xp.checked_add(remainder? as u64))
                .map(|xp| xp / u64::from(rate))
        }
        _ => None,
    };
    HexCoinEstimate {
        estimated_remaining_coins,
        xp_per_coin,
        claim_xp_remainder: remainder,
        assumed_xp_per_hit,
    }
}

#[derive(Clone, Serialize)]
struct BuildingContext {
    claim_id: Option<String>,
    claim_name: Option<String>,
    location: Option<BuildingLocation>,
}

#[derive(Clone, Serialize)]
struct BuildingLocation {
    x: i64,
    z: i64,
    dimension: i64,
    region: entity::shared::Region,
}

#[derive(Serialize)]
struct CraftIcon {
    name: String,
    icon_asset_name: String,
    tier: Option<i32>,
}

fn recipe_outputs(state: &AppState, recipe: &entity::crafting_recipe::Model) -> Vec<CraftIcon> {
    use entity::inventory::ItemType;
    recipe
        .crafted_item_stacks
        .iter()
        .map(|item| {
            let resolved = match item.item_type {
                ItemType::Item => state.item_desc.get(&item.item_id).map(|item| {
                    (
                        item.name.clone(),
                        item.icon_asset_name.clone(),
                        Some(item.tier),
                    )
                }),
                ItemType::Cargo => state.cargo_desc.get(&item.item_id).map(|item| {
                    (
                        item.name.clone(),
                        item.icon_asset_name.clone(),
                        Some(item.tier),
                    )
                }),
            };
            let (name, icon_asset_name, tier) = resolved.unwrap_or_else(|| {
                (
                    format!(
                        "{} #{}",
                        match item.item_type {
                            ItemType::Item => "Item",
                            ItemType::Cargo => "Cargo",
                        },
                        item.item_id
                    ),
                    String::new(),
                    None,
                )
            });
            CraftIcon {
                name,
                icon_asset_name,
                tier,
            }
        })
        .collect()
}

fn station_functions(functions: &[entity::building_desc::Function], passive: bool) -> Vec<i32> {
    let mut types: Vec<_> = functions
        .iter()
        .filter(|function| {
            if passive {
                function.refining_slots > 0 || function.refining_cargo_slots > 0
            } else {
                function.crafting_slots > 0
            }
        })
        .map(|function| function.function_type)
        .collect();
    types.sort_unstable();
    types.dedup();
    types
}

#[derive(Serialize)]
pub(crate) struct CraftActivity {
    id: String,
    timestamp: String,
    actor_id: String,
    actor_name: Option<String>,
    is_member: bool,
    building_id: Option<String>,
    recipe_name: Option<String>,
    recipe_id: Option<i32>,
    event_type: &'static str,
    progress: Option<i32>,
    is_crit: Option<bool>,
    quantity: Option<i32>,
}

pub(crate) async fn activity(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<Vec<CraftActivity>>, StatusCode> {
    use entity::craft_activity;
    let claim = claim_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .map_err(query_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let events = craft_activity::Entity::find()
        .filter(craft_activity::Column::ClaimEntityId.eq(id))
        .order_by_desc(craft_activity::Column::Timestamp)
        .order_by_desc(craft_activity::Column::Id)
        .limit(100)
        .all(&state.conn)
        .await
        .map_err(query_error)?;
    if events.is_empty() {
        return Ok(axum_codec::Codec(vec![]));
    }
    let members: HashSet<_> = claim_member_state::Entity::find()
        .filter(claim_member_state::Column::ClaimEntityId.eq(id))
        .all(&state.conn)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|m| m.player_entity_id)
        .chain((claim.owner_player_entity_id != 0).then_some(claim.owner_player_entity_id))
        .collect();
    let ids: HashSet<_> = events.iter().map(|e| e.actor_entity_id).collect();
    let names: HashMap<_, _> = player_username_state::Entity::find()
        .filter(player_username_state::Column::EntityId.is_in(ids))
        .all(&state.conn)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|p| (p.entity_id, p.username))
        .collect();
    Ok(axum_codec::Codec(
        events
            .into_iter()
            .map(|e| CraftActivity {
                id: e.id.to_string(),
                timestamp: e.timestamp.to_rfc3339(),
                actor_id: e.actor_entity_id.to_string(),
                actor_name: names.get(&e.actor_entity_id).cloned(),
                is_member: members.contains(&e.actor_entity_id),
                building_id: e.building_entity_id.map(|id| id.to_string()),
                recipe_name: e.recipe_id.and_then(|id| {
                    state
                        .crafting_recipe_desc
                        .get(&id)
                        .map(|r| recipe_name(&state, &r))
                }),
                recipe_id: e.recipe_id,
                event_type: if e.event_type == 1 {
                    "Complete"
                } else {
                    "Progress"
                },
                progress: e.progress,
                is_crit: e.is_crit,
                quantity: e.craft_count,
            })
            .collect(),
    ))
}

fn query_error(error: sea_orm::DbErr) -> StatusCode {
    tracing::error!(%error, "Could not query claim crafting");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn active_status(progress: i32, total: Option<i64>, expired: bool) -> &'static str {
    if total.is_some_and(|t| i64::from(progress) >= t) {
        "Complete"
    } else if total.is_none() {
        "Unknown"
    } else if expired {
        "Suspended"
    } else {
        "Active"
    }
}

/// `GET /api/bitcraft/claims/{id}/crafting?include_private=false&include_passive=false`.
/// Both flags default to true. They are independent: passive crafts have no
/// public/private marker and are controlled only by `include_passive`.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub(crate) struct ClaimCraftParams {
    include_private: bool,
    include_passive: bool,
}

impl Default for ClaimCraftParams {
    fn default() -> Self {
        Self {
            include_private: true,
            include_passive: true,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<ClaimCraftParams>,
) -> Result<axum_codec::Codec<Vec<ClaimCraft>>, StatusCode> {
    let claim = claim_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .map_err(query_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let buildings = building_state::Entity::find()
        .filter(building_state::Column::ClaimEntityId.eq(id))
        .all(&state.conn)
        .await
        .map_err(query_error)?;
    if buildings.is_empty() {
        return Ok(axum_codec::Codec(vec![]));
    }
    let building_ids: Vec<_> = buildings.iter().map(|b| b.entity_id).collect();
    let mut active_query = progressive_action_state::Entity::find().filter(
        progressive_action_state::Column::BuildingEntityId.is_in(building_ids.iter().copied()),
    );
    if !params.include_private {
        active_query = active_query.filter(
            progressive_action_state::Column::EntityId.in_subquery(
                public_progressive_action_state::Entity::find()
                    .select_only()
                    .column(public_progressive_action_state::Column::EntityId)
                    .into_query(),
            ),
        );
    }
    let active = active_query.all(&state.conn).await.map_err(query_error)?;
    let passive = if params.include_passive {
        passive_craft_state::Entity::find()
            .filter(
                passive_craft_state::Column::BuildingEntityId.is_in(building_ids.iter().copied()),
            )
            .all(&state.conn)
            .await
            .map_err(query_error)?
    } else {
        vec![]
    };
    let members: HashSet<_> = claim_member_state::Entity::find()
        .filter(claim_member_state::Column::ClaimEntityId.eq(id))
        .all(&state.conn)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|m| m.player_entity_id)
        .chain((claim.owner_player_entity_id != 0).then_some(claim.owner_player_entity_id))
        .collect();
    let members_by_claim = HashMap::from([(id, members)]);
    let axum_codec::Codec(mut crafts) = assemble(
        &state,
        buildings,
        active,
        passive,
        members_by_claim,
        HashMap::new(),
    )
    .await?;
    // Public markers may change between the active query and the later marker
    // lookup. Avoid returning an active row we now know is private.
    if !params.include_private {
        crafts.retain(|craft| craft.kind != "Active" || craft.public == Some(true));
    }
    Ok(axum_codec::Codec(crafts))
}

pub(crate) async fn player_crafts(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<Vec<ClaimCraft>>, StatusCode> {
    let active = progressive_action_state::Entity::find()
        .filter(progressive_action_state::Column::OwnerEntityId.eq(id))
        .all(&state.conn)
        .await
        .map_err(query_error)?;
    let passive = passive_craft_state::Entity::find()
        .filter(passive_craft_state::Column::OwnerEntityId.eq(id))
        .all(&state.conn)
        .await
        .map_err(query_error)?;
    let building_ids: HashSet<_> = active
        .iter()
        .map(|p| p.building_entity_id)
        .chain(passive.iter().map(|p| p.building_entity_id))
        .collect();
    if building_ids.is_empty() {
        return Ok(axum_codec::Codec(vec![]));
    }
    let buildings = building_state::Entity::find()
        .filter(building_state::Column::EntityId.is_in(building_ids))
        .all(&state.conn)
        .await
        .map_err(query_error)?;
    let mut building_context: HashMap<i64, BuildingContext> = buildings
        .iter()
        .map(|building| {
            (
                building.entity_id,
                BuildingContext {
                    claim_id: (building.claim_entity_id != 0)
                        .then(|| building.claim_entity_id.to_string()),
                    claim_name: None,
                    location: None,
                },
            )
        })
        .collect();
    if !building_context.is_empty() {
        for location in entity::location_state::Entity::find()
            .filter(
                entity::location_state::Column::EntityId.is_in(building_context.keys().copied()),
            )
            .all(&state.conn)
            .await
            .map_err(query_error)?
        {
            if let Some(building) = building_context.get_mut(&location.entity_id) {
                building.location = Some(BuildingLocation {
                    x: location.x,
                    z: location.z,
                    dimension: location.dimension,
                    region: location.region,
                });
            }
        }
    }
    let claim_ids: HashSet<_> = buildings
        .iter()
        .map(|b| b.claim_entity_id)
        .filter(|id| *id != 0)
        .collect();
    let mut members_by_claim: HashMap<i64, HashSet<i64>> = HashMap::new();
    if !claim_ids.is_empty() {
        for member in claim_member_state::Entity::find()
            .filter(claim_member_state::Column::ClaimEntityId.is_in(claim_ids.iter().copied()))
            .filter(claim_member_state::Column::PlayerEntityId.eq(id))
            .all(&state.conn)
            .await
            .map_err(query_error)?
        {
            members_by_claim
                .entry(member.claim_entity_id)
                .or_default()
                .insert(id);
        }
        let claims = claim_state::Entity::find()
            .filter(claim_state::Column::EntityId.is_in(claim_ids))
            .all(&state.conn)
            .await
            .map_err(query_error)?;
        let mut claim_names = HashMap::new();
        for claim in claims {
            claim_names.insert(claim.entity_id.to_string(), claim.name);
            if claim.owner_player_entity_id == id {
                members_by_claim
                    .entry(claim.entity_id)
                    .or_default()
                    .insert(id);
            }
        }
        for building in building_context.values_mut() {
            building.claim_name = building
                .claim_id
                .as_ref()
                .and_then(|id| claim_names.get(id).cloned());
        }
    }
    assemble(
        &state,
        buildings,
        active,
        passive,
        members_by_claim,
        building_context,
    )
    .await
}

async fn assemble(
    state: &AppState,
    buildings: Vec<building_state::Model>,
    active: Vec<progressive_action_state::Model>,
    passive: Vec<passive_craft_state::Model>,
    members_by_claim: HashMap<i64, HashSet<i64>>,
    building_context: HashMap<i64, BuildingContext>,
) -> Result<axum_codec::Codec<Vec<ClaimCraft>>, StatusCode> {
    let public_ids: HashSet<_> = if active.is_empty() {
        HashSet::new()
    } else {
        public_progressive_action_state::Entity::find()
            .filter(
                public_progressive_action_state::Column::EntityId
                    .is_in(active.iter().map(|p| p.entity_id)),
            )
            .all(&state.conn)
            .await
            .map_err(query_error)?
            .into_iter()
            .map(|p| p.entity_id)
            .collect()
    };
    let owner_ids: HashSet<_> = active
        .iter()
        .map(|p| p.owner_entity_id)
        .chain(passive.iter().map(|p| p.owner_entity_id))
        .collect();
    let names: HashMap<_, _> = if owner_ids.is_empty() {
        HashMap::new()
    } else {
        player_username_state::Entity::find()
            .filter(player_username_state::Column::EntityId.is_in(owner_ids))
            .all(&state.conn)
            .await
            .map_err(query_error)?
            .into_iter()
            .map(|p| (p.entity_id, p.username))
            .collect()
    };
    let active_buildings: HashSet<_> = active.iter().map(|p| p.building_entity_id).collect();
    let claim_ids: HashSet<_> = buildings
        .iter()
        .filter(|b| active_buildings.contains(&b.entity_id) && b.claim_entity_id != 0)
        .map(|b| b.claim_entity_id)
        .collect();
    let mut mint_thresholds: HashMap<i64, Option<u32>> = HashMap::new();
    if !claim_ids.is_empty() {
        for tech in entity::claim_tech_state::Entity::find()
            .filter(entity::claim_tech_state::Column::EntityId.is_in(claim_ids))
            .all(&state.conn)
            .await
            .map_err(query_error)?
        {
            if let Some(threshold) = mint_threshold(&tech.learned, |id| {
                state
                    .claim_tech_desc
                    .get(&id)
                    .map(|desc| desc.xp_to_mint_hex_coin as u32)
            }) {
                mint_thresholds.insert(tech.entity_id, threshold);
            }
        }
    }
    let building_info: HashMap<_, _> = buildings
        .into_iter()
        .map(|b| {
            let info = state
                .building_desc
                .get(&(b.building_description_id as i64))
                .map(|d| {
                    (
                        d.name.clone(),
                        station_functions(&d.functions, false),
                        station_functions(&d.functions, true),
                        Some(d.icon_asset_name.clone()),
                        b.claim_entity_id,
                    )
                })
                .unwrap_or_else(|| {
                    (
                        format!("Building #{}", b.building_description_id),
                        vec![],
                        vec![],
                        None,
                        b.claim_entity_id,
                    )
                });
            (b.entity_id, info)
        })
        .collect();
    let fallback = ("Unknown building".to_string(), vec![], vec![], None, 0);
    let mut result = Vec::with_capacity(active.len() + passive.len());
    for p in active {
        let known_building = building_info.contains_key(&p.building_entity_id);
        let building = building_info
            .get(&p.building_entity_id)
            .unwrap_or(&fallback);
        let recipe = state.crafting_recipe_desc.get(&p.recipe_id);
        let total = recipe
            .as_ref()
            .map(|r| i64::from(r.actions_required) * i64::from(p.craft_count));
        let status = active_status(
            p.progress,
            total,
            p.lock_expiration.__timestamp_micros_since_unix_epoch__ <= Utc::now(),
        );
        let threshold = mint_thresholds.get(&building.4).copied();
        let remainder = state
            .claim_local_state
            .get(&(building.4 as u64))
            .map(|local| local.xp_gained_since_last_coin_minting as u32);
        let xp = recipe
            .as_ref()
            .and_then(|r| r.experience_per_progress.first().map(|xp| xp.quantity));
        let eligible = if recipe
            .as_ref()
            .is_some_and(|r| r.experience_per_progress.is_empty())
        {
            Some(false)
        } else {
            known_building.then_some(building.4 != 0)
        };
        let hex_coin_estimate = estimate_hex_coins(
            total.map(|total| (total - i64::from(p.progress)).max(0)),
            xp,
            threshold,
            remainder,
            eligible,
        );
        result.push(ClaimCraft {
            id: p.entity_id.to_string(),
            building_id: p.building_entity_id.to_string(),
            building_name: building.0.clone(),
            building_icon_asset_name: building.3.clone(),
            building_function_types: building.1.clone(),
            building_context: building_context.get(&p.building_entity_id).cloned(),
            owner_id: p.owner_entity_id.to_string(),
            owner_name: names.get(&p.owner_entity_id).cloned(),
            is_member: members_by_claim
                .get(&building.4)
                .is_some_and(|members| members.contains(&p.owner_entity_id)),
            recipe_id: p.recipe_id,
            recipe_name: recipe.as_ref().map(|r| recipe_name(state, r)),
            outputs: recipe
                .as_ref()
                .map(|r| recipe_outputs(state, r))
                .unwrap_or_default(),
            kind: "Active",
            status,
            quantity: p.craft_count,
            progress: Some(p.progress),
            progress_total: total,
            public: Some(public_ids.contains(&p.entity_id)),
            slot: None,
            timestamp: Some(
                p.lock_expiration
                    .__timestamp_micros_since_unix_epoch__
                    .to_rfc3339(),
            ),
            duration_seconds: None,
            hex_coin_estimate,
        });
    }
    for p in passive {
        let building = building_info
            .get(&p.building_entity_id)
            .unwrap_or(&fallback);
        let recipe = state.crafting_recipe_desc.get(&p.recipe_id);
        result.push(ClaimCraft {
            id: p.entity_id.to_string(),
            building_id: p.building_entity_id.to_string(),
            building_name: building.0.clone(),
            building_icon_asset_name: building.3.clone(),
            building_function_types: building.2.clone(),
            building_context: building_context.get(&p.building_entity_id).cloned(),
            owner_id: p.owner_entity_id.to_string(),
            owner_name: names.get(&p.owner_entity_id).cloned(),
            is_member: members_by_claim
                .get(&building.4)
                .is_some_and(|members| members.contains(&p.owner_entity_id)),
            recipe_id: p.recipe_id,
            recipe_name: recipe.as_ref().map(|r| recipe_name(state, r)),
            outputs: recipe
                .as_ref()
                .map(|r| recipe_outputs(state, r))
                .unwrap_or_default(),
            kind: "Passive",
            status: match p.status {
                0 => "Queued",
                1 => "Processing",
                2 => "Complete",
                _ => "Unknown",
            },
            quantity: 1,
            progress: None,
            progress_total: None,
            public: None,
            slot: p.slot,
            timestamp: Some(p.timestamp.to_rfc3339()),
            duration_seconds: recipe.as_ref().map(|r| r.time_requirement),
            hex_coin_estimate: estimate_hex_coins(None, None, None, None, Some(false)),
        });
    }
    result.sort_by(|a, b| {
        a.building_name
            .cmp(&b.building_name)
            .then_with(|| a.building_id.cmp(&b.building_id))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(axum_codec::Codec(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claim_craft_query_defaults_and_boolean_validation() {
        fn parse(
            query: &str,
        ) -> Result<ClaimCraftParams, axum::extract::rejection::QueryRejection> {
            let uri: axum::http::Uri = format!("/?{query}").parse().unwrap();
            Query::<ClaimCraftParams>::try_from_uri(&uri).map(|Query(value)| value)
        }
        let defaults = parse("").unwrap();
        assert!(defaults.include_private && defaults.include_passive);
        let public_only = parse("include_private=false&include_passive=false").unwrap();
        assert!(!public_only.include_private && !public_only.include_passive);
        let passive_only_filter = parse("include_passive=false").unwrap();
        assert!(passive_only_filter.include_private && !passive_only_filter.include_passive);
        assert!(parse("include_private=invalid").is_err());
    }

    #[tokio::test]
    async fn claim_filters_are_independent_and_skip_passive_query() {
        use sea_orm::{DbBackend, MockDatabase};
        let claim = claim_state::Model {
            entity_id: 42,
            owner_player_entity_id: 6,
            owner_building_entity_id: 0,
            name: "Test".into(),
            neutral: false,
            region: 7,
        };
        let building = building_state::Model {
            entity_id: 2,
            claim_entity_id: 42,
            direction_index: 0,
            building_description_id: 3,
            constructed_by_player_entity_id: 0,
            region: 7,
        };
        let action = |id| {
            entity::progressive_action_state::ModelBuilder::new(
                game_module::module_bindings::ProgressiveActionState {
                    entity_id: id,
                    building_entity_id: 2,
                    function_type: 1,
                    progress: 1,
                    recipe_id: 5,
                    craft_count: 1,
                    last_crit_outcome: 0,
                    owner_entity_id: 6,
                    lock_expiration: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
                    preparation: false,
                },
            )
            .with_region(7)
            .build()
        };
        let passive = passive_craft_state::Model {
            entity_id: 10,
            owner_entity_id: 6,
            recipe_id: 5,
            building_entity_id: 2,
            timestamp: Utc::now().into(),
            status: 0,
            slot: None,
            region: 7,
        };
        for include_private in [false, true] {
            for include_passive in [false, true] {
                let mut db = MockDatabase::new(DbBackend::Postgres)
                    .append_query_results([vec![claim.clone()]])
                    .append_query_results([vec![building.clone()]])
                    // The mock represents the result of the SQL public-project filter.
                    .append_query_results([if include_private {
                        vec![action(1), action(2)]
                    } else {
                        vec![action(1)]
                    }]);
                if include_passive {
                    db = db.append_query_results([vec![passive.clone()]]);
                }
                let db = db
                    .append_query_results([Vec::<claim_member_state::Model>::new()])
                    .append_query_results([vec![public_progressive_action_state::Model {
                        entity_id: 1,
                        building_entity_id: 2,
                        owner_entity_id: 6,
                        region: 7,
                    }]])
                    .append_query_results([Vec::<player_username_state::Model>::new()])
                    .append_query_results([Vec::<entity::claim_tech_state::Model>::new()])
                    .into_connection();
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                let state = AppState::new(db.clone(), tx, Default::default());
                let params = ClaimCraftParams {
                    include_private,
                    include_passive,
                };
                let axum_codec::Codec(crafts) =
                    list(State(state), Path(42), Query(params)).await.unwrap();
                assert_eq!(
                    crafts.iter().filter(|c| c.kind == "Active").count(),
                    if include_private { 2 } else { 1 }
                );
                assert_eq!(
                    crafts.iter().filter(|c| c.kind == "Passive").count(),
                    usize::from(include_passive)
                );
                assert!(crafts.iter().any(|c| c.id == "1" && c.public == Some(true)));
                if include_private {
                    assert!(
                        crafts
                            .iter()
                            .any(|c| c.id == "2" && c.public == Some(false))
                    );
                }
                let log = db.into_transaction_log();
                let active_sql = format!("{:?}", log[2]);
                assert_eq!(
                    active_sql.contains("public_progressive_action_state"),
                    !include_private
                );
                let passive_queries = log
                    .iter()
                    .filter(|entry| format!("{entry:?}").contains("passive_craft_state"))
                    .count();
                assert_eq!(passive_queries, usize::from(include_passive));
            }
        }
    }
    #[test]
    fn coin_threshold_uses_minimum_nonzero_and_preserves_missing_data() {
        let values = HashMap::from([(1, 0), (2, 100), (3, 50), (4, u32::MAX)]);
        assert_eq!(
            mint_threshold(&[1, 2, 3], |id| values.get(&id).copied()),
            Some(Some(50))
        );
        assert_eq!(
            mint_threshold(&[1, 4], |id| values.get(&id).copied()),
            Some(None)
        );
        assert_eq!(mint_threshold(&[2, 5], |id| values.get(&id).copied()), None);
    }
    #[test]
    fn hex_coin_estimate_uses_rounded_xp_and_existing_remainder() {
        let estimate = estimate_hex_coins(Some(10), Some(1.2), Some(Some(6)), Some(3), Some(true));
        assert_eq!(estimate.assumed_xp_per_hit, Some(2));
        assert_eq!(estimate.estimated_remaining_coins, Some(3));
        assert_eq!(estimate.xp_per_coin, Some(6));
        assert_eq!(estimate.claim_xp_remainder, Some(3));
    }

    #[test]
    fn hex_coin_estimate_preserves_unknown_and_zero_cases() {
        assert_eq!(
            estimate_hex_coins(Some(10), Some(1.0), None, Some(0), Some(true))
                .estimated_remaining_coins,
            None
        );
        assert_eq!(
            estimate_hex_coins(Some(10), Some(1.0), Some(Some(10)), None, Some(true))
                .estimated_remaining_coins,
            None
        );
        assert_eq!(
            estimate_hex_coins(Some(10), None, Some(Some(10)), Some(0), Some(true))
                .estimated_remaining_coins,
            None
        );
        assert_eq!(
            estimate_hex_coins(Some(10), Some(1.0), Some(None), None, Some(true))
                .estimated_remaining_coins,
            Some(0)
        );
        assert_eq!(
            estimate_hex_coins(None, None, None, None, Some(false)).estimated_remaining_coins,
            Some(0)
        );
        assert_eq!(
            estimate_hex_coins(Some(0), None, None, None, None).estimated_remaining_coins,
            Some(0)
        );
    }
    #[tokio::test]
    async fn player_crafts_include_building_location_and_claim() {
        use sea_orm::{DbBackend, MockDatabase};
        let passive = passive_craft_state::Model {
            entity_id: 1,
            owner_entity_id: 42,
            recipe_id: 5,
            building_entity_id: 2,
            timestamp: Utc::now().into(),
            status: 1,
            slot: Some(0),
            region: 7,
        };
        let building = building_state::Model {
            entity_id: 2,
            claim_entity_id: 3,
            direction_index: 0,
            building_description_id: 4,
            constructed_by_player_entity_id: 0,
            region: 7,
        };
        let location = entity::location_state::Model {
            entity_id: 2,
            chunk_index: 0,
            x: -12,
            z: 15,
            dimension: 1,
            region: 7,
        };
        let claim = claim_state::Model {
            entity_id: 3,
            owner_player_entity_id: 42,
            owner_building_entity_id: 0,
            name: "Home".into(),
            neutral: false,
            region: 7,
        };
        let username = player_username_state::Model {
            entity_id: 42,
            username: "Player".into(),
            region: 7,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<progressive_action_state::Model>::new()])
            .append_query_results([vec![passive]])
            .append_query_results([vec![building]])
            .append_query_results([vec![location]])
            .append_query_results([Vec::<claim_member_state::Model>::new()])
            .append_query_results([vec![claim]])
            .append_query_results([vec![username]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let axum_codec::Codec(crafts) = player_crafts(State(state), Path(42)).await.unwrap();
        assert_eq!(crafts.len(), 1);
        let context = crafts[0].building_context.as_ref().unwrap();
        assert_eq!(context.claim_id.as_deref(), Some("3"));
        assert_eq!(context.claim_name.as_deref(), Some("Home"));
        let location = context.location.as_ref().unwrap();
        assert_eq!(
            (location.x, location.z, location.dimension, location.region),
            (-12, 15, 1, 7)
        );
        assert!(crafts[0].is_member);
    }
    #[tokio::test]
    async fn player_crafts_keep_unclaimed_buildings_when_location_is_missing() {
        use sea_orm::{DbBackend, MockDatabase};
        let passive = passive_craft_state::Model {
            entity_id: 1,
            owner_entity_id: 42,
            recipe_id: 5,
            building_entity_id: 2,
            timestamp: Utc::now().into(),
            status: 0,
            slot: None,
            region: 7,
        };
        let building = building_state::Model {
            entity_id: 2,
            claim_entity_id: 0,
            direction_index: 0,
            building_description_id: 4,
            constructed_by_player_entity_id: 0,
            region: 7,
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<progressive_action_state::Model>::new()])
            .append_query_results([vec![passive]])
            .append_query_results([vec![building]])
            .append_query_results([Vec::<entity::location_state::Model>::new()])
            .append_query_results([Vec::<player_username_state::Model>::new()])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let axum_codec::Codec(crafts) =
            player_crafts(State(AppState::new(db, tx, Default::default())), Path(42))
                .await
                .unwrap();
        let context = crafts[0].building_context.as_ref().unwrap();
        assert!(
            context.claim_id.is_none()
                && context.claim_name.is_none()
                && context.location.is_none()
        );
    }
    #[tokio::test]
    async fn player_crafts_queries_only_the_requested_owner() {
        use sea_orm::{DbBackend, MockDatabase};
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<progressive_action_state::Model>::new()])
            .append_query_results([Vec::<passive_craft_state::Model>::new()])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let axum_codec::Codec(crafts) = player_crafts(State(state), Path(42)).await.unwrap();
        assert!(crafts.is_empty());
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("owner_entity_id"));
        assert!(format!("{:?}", log[1]).contains("owner_entity_id"));
        assert!(format!("{log:?}").contains("42"));
    }
    #[test]
    fn station_function_types_ignore_unrelated_slots_and_sort_deduplicate() {
        fn function(
            id: i32,
            crafting: i32,
            refining: i32,
            cargo: i32,
        ) -> entity::building_desc::Function {
            entity::building_desc::Function {
                function_type: id,
                level: 0,
                crafting_slots: crafting,
                storage_slots: 0,
                cargo_slots: 0,
                refining_slots: refining,
                refining_cargo_slots: cargo,
                item_slot_size: 0,
                cargo_slot_size: 0,
                trade_orders: 0,
                allowed_item_id_per_slot: vec![],
                concurrent_crafts_per_player: 0,
                terraform: false,
                housing_slots: 0,
                housing_income: 0,
            }
        }
        let functions = vec![
            function(9, 0, 1, 0),
            function(2, 1, 0, 0),
            function(7, 0, 0, 0),
            function(4, 0, 0, 1),
            function(9, 0, 2, 0),
        ];
        assert_eq!(station_functions(&functions, false), vec![2]);
        assert_eq!(station_functions(&functions, true), vec![4, 9]);
    }
    #[test]
    fn recipe_templates_resolve_each_index_and_preserve_missing_values() {
        let values = vec![
            Some("Copper Ingot".to_owned()),
            Some("Copper Ore".to_owned()),
            None,
            Some("Coal".to_owned()),
        ];
        assert_eq!(
            format_template("Smelt {0} from {1} and {3}: {0}, {2}, {9}", &values),
            "Smelt Copper Ingot from Copper Ore and Coal: Copper Ingot, {2}, {9}"
        );
        assert_eq!(
            format_template("Unknown {name} and {", &values),
            "Unknown {name} and {"
        );
    }
    #[test]
    fn status_matches_game_rules() {
        assert_eq!(active_status(10, Some(10), true), "Complete");
        assert_eq!(active_status(9, Some(10), true), "Suspended");
        assert_eq!(active_status(9, Some(10), false), "Active");
        assert_eq!(active_status(9, None, false), "Unknown");
    }
}
