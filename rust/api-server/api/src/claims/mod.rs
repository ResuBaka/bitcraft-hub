pub(crate) mod bitcraft;
pub(crate) mod claim_local_state;
pub(crate) mod claim_location;
pub(crate) mod claim_member_state;
pub(crate) mod claim_state;
pub(crate) mod events;
pub(crate) mod member_cache;

use crate::inventory::{InventoryChangesParams, resolve_contents, resolve_pocket};
use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::inventory::{ExpendedRefrence, ItemExpended, ItemType, ResolvedInventory};
use entity::shared::location::Location;
use entity::{
    building_state, cargo_desc, claim_tech_desc, inventory, inventory_changelog, item_desc,
    player_state,
};
use log::error;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::cmp::Ordering;
use crate::skill_descriptions::skill_sort_key;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinHandle;
use ts_rs::TS;

fn claim_progression(upgrades: &[claim_tech_desc::Model], learned: &[i32], building_tiers: &HashSet<i32>) -> (i32, i32) {
    use claim_tech_desc::ClaimTechType;

    let starter_tier = upgrades
        .iter()
        .filter(|upgrade| upgrade.tech_type == ClaimTechType::StarterTech)
        .min_by_key(|upgrade| upgrade.id)
        .map_or(2, |upgrade| upgrade.tier);

    let learned_upgrades = upgrades
        .iter()
        .filter(|upgrade| {
            matches!(upgrade.tech_type, ClaimTechType::Homestead | ClaimTechType::Settlement)
                && learned.contains(&upgrade.id)
        });
    let tier = learned_upgrades.clone().map(|upgrade| upgrade.tier).max().unwrap_or(starter_tier);
    // Even ranks are Townships; the preceding odd rank is the same-tier Homestead.
    let rank = learned_upgrades
        .filter(|upgrade| building_tiers.contains(&upgrade.tier))
        .map(|upgrade| {
            upgrade.tier * 2 - i32::from(upgrade.tech_type == ClaimTechType::Homestead)
        })
        .max()
        .unwrap_or(starter_tier * 2);
    (tier, rank)
}

async fn building_tiers_by_claim(
    conn: &sea_orm::DatabaseConnection,
    buildings: &[building_state::Model],
) -> Result<(HashMap<i64, HashSet<i32>>, HashMap<i64, Vec<ClaimStation>>), sea_orm::DbErr> {
    let ids: HashSet<i64> = buildings.iter().map(|building| building.building_description_id as i64).collect();
    let descs = entity::building_desc::Entity::find()
        .filter(entity::building_desc::Column::Id.is_in(ids))
        .all(conn)
        .await?
        .into_iter()
        .map(|desc| (desc.id, desc))
        .collect::<HashMap<_, _>>();
    let mut tiers: HashMap<i64, HashSet<i32>> = HashMap::new();
    let mut stations: HashMap<i64, HashMap<(i32, bool), ClaimStation>> = HashMap::new();
    for building in buildings {
        if let Some(desc) = descs.get(&(building.building_description_id as i64)) {
            tiers.entry(building.claim_entity_id).or_default()
                .extend(desc.functions.iter().map(|function| function.level));
            let mut seen = HashSet::new();
            for function in &desc.functions {
                for passive in [false, true] {
                    let has_slots = if passive {
                        function.refining_slots > 0 || function.refining_cargo_slots > 0
                    } else {
                        function.crafting_slots > 0
                    };
                    let key = (function.function_type, passive);
                    if has_slots && seen.insert(key) {
                        add_station(stations.entry(building.claim_entity_id).or_default(), ClaimStation {
                            id: desc.id,
                            function_type: function.function_type,
                            tier: function.level,
                            name: desc.name.clone(),
                            icon_asset_name: desc.icon_asset_name.clone(),
                            crafting: !passive,
                            passive,
                            count: 1,
                        });
                    }
                }
            }
        }
    }
    let stations = stations.into_iter().map(|(claim_id, stations)| {
        let mut stations: Vec<_> = stations.into_values().collect();
        stations.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        (claim_id, stations)
    }).collect();
    Ok((tiers, stations))
}

fn add_station(stations: &mut HashMap<(i32, bool), ClaimStation>, candidate: ClaimStation) {
    let key = (candidate.function_type, candidate.passive);
    match stations.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => { entry.insert(candidate); }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let current = entry.get_mut();
            let count = current.count + 1;
            if candidate.tier > current.tier || (candidate.tier == current.tier && candidate.id < current.id) {
                *current = candidate;
            }
            current.count = count;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClaimStation {
    pub id: i64,
    pub function_type: i32,
    pub tier: i32,
    pub name: String,
    pub icon_asset_name: String,
    pub crafting: bool,
    pub passive: bool,
    pub count: u32,
}

#[cfg(test)]
mod claim_tier_tests {
    use super::*;
    use claim_tech_desc::ClaimTechType;

    #[test]
    fn station_summary_keeps_highest_tier_and_counts_all_tiers() {
        let mut stations = HashMap::new();
        let station = |id, tier, function_type, passive: bool| ClaimStation {
            id, function_type, tier, name: format!("Station {id}"), icon_asset_name: String::new(),
            crafting: !passive, passive, count: 1,
        };
        add_station(&mut stations, station(10, 3, 20, false));
        add_station(&mut stations, station(11, 5, 20, false));
        add_station(&mut stations, station(12, 2, 20, false));
        add_station(&mut stations, station(13, 4, 20, true));
        assert_eq!(stations[&(20, false)].tier, 5);
        assert_eq!(stations[&(20, false)].id, 11);
        assert_eq!(stations[&(20, false)].count, 3);
        assert_eq!(stations[&(20, true)].count, 1);
    }

    fn upgrade(id: i32, tier: i32, tech_type: ClaimTechType) -> claim_tech_desc::Model {
        claim_tech_desc::Model {
            id,
            name: String::new(),
            description: String::new(),
            tier,
            tech_type,
            supplies_cost: 0,
            research_time: 0,
            requirements: vec![],
            input: vec![],
            members: 0,
            area: 0,
            supplies: 0,
            xp_to_mint_hex_coin: 0,
            unlocks_techs: vec![],
        }
    }

    #[test]
    fn uses_starter_tier_until_a_claim_upgrade_is_learned() {
        let upgrades = vec![
            upgrade(1, 2, ClaimTechType::StarterTech),
            upgrade(300, 3, ClaimTechType::TierUpgrade),
            upgrade(2, 3, ClaimTechType::Homestead),
        ];
        let tiers = HashSet::from([3]);
        assert_eq!(claim_progression(&upgrades, &[], &tiers), (2, 4));
        assert_eq!(claim_progression(&upgrades, &[300], &tiers), (2, 4));
        assert_eq!(claim_progression(&[], &[], &tiers), (2, 4));
    }

    #[test]
    fn uses_highest_learned_homestead_or_township_tier() {
        let upgrades = vec![
            upgrade(1, 2, ClaimTechType::StarterTech),
            upgrade(2, 3, ClaimTechType::Homestead),
            upgrade(3, 5, ClaimTechType::Homestead),
            upgrade(4, 4, ClaimTechType::Settlement),
            upgrade(5, 9, ClaimTechType::Mining),
        ];
        let tiers = HashSet::from([3, 4, 5]);
        assert_eq!(claim_progression(&upgrades, &[3, 2, 5], &tiers), (5, 9));
        assert_eq!(claim_progression(&upgrades, &[4], &tiers), (4, 8));
        assert_eq!(claim_progression(&upgrades, &[5], &tiers), (2, 4));
        assert_eq!(claim_progression(&upgrades, &[2], &tiers), (3, 5));
        let township = upgrade(6, 3, ClaimTechType::Settlement);
        assert_eq!(claim_progression(&[township], &[6], &tiers), (3, 6));
        assert_eq!(claim_progression(&upgrades, &[3, 2], &HashSet::from([3])), (5, 5));
        assert_eq!(claim_progression(&upgrades, &[3, 2], &HashSet::from([4])), (5, 4));
    }

}

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/api/bitcraft/claims/{id}/crafting", axum_codec::routing::get(crate::crafting::claim::list).into())
        .route("/api/bitcraft/claims/{id}/crafting/activity", axum_codec::routing::get(crate::crafting::claim::activity).into())
        .route(
            "/claims/{id}/events",
            axum_codec::routing::get(events::list).into(),
        )
        .route(
            "/api/bitcraft/claims/{id}/events",
            axum_codec::routing::get(events::list).into(),
        )
        .route("/claims", axum_codec::routing::get(list_claims).into())
        .route(
            "/claims/names",
            axum_codec::routing::get(get_claim_names).into(),
        )
        .route(
            "/api/bitcraft/claims",
            axum_codec::routing::get(list_claims).into(),
        )
        .route(
            "/api/bitcraft/claims/{id}",
            axum_codec::routing::get(get_claim).into(),
        )
        .route(
            "/claims/{id}/auction_listings",
            axum_codec::routing::get(get_claim_auction_listings).into(),
        )
        .route(
            "/api/bitcraft/claims/{id}/auction_listings",
            axum_codec::routing::get(get_claim_auction_listings).into(),
        )
        // .route(
        //     "/claims/{id}",
        //     axum_codec::routing::get(find_claim_descriptions).into(),
        // )
        .route(
            "/claims/tiles/{id}",
            axum_codec::routing::get(get_claim_tiles).into(),
        )
        .route(
            "/claims/inventory_changelog/{id}",
            axum_codec::routing::get(get_claim_inventory_change_log).into(),
        )
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ClaimDescriptionState {
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub supplies: i32,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionStateMember>,
    pub num_tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: Option<Location>,
    pub treasury: i32,
    pub running_upgrade: Option<claim_tech_desc::Model>,
    pub running_upgrade_started: Option<entity::shared::timestamp::Timestamp>,
    pub tier: Option<i32>,
    pub rank: i32,
    pub stations: Vec<ClaimStation>,
    pub upgrades: Vec<claim_tech_desc::Model>,
    pub xp_gained_since_last_coin_minting: i32,
    pub region: entity::shared::Region,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClaimSummaryResponse {
    pub name: String,
    pub region: entity::shared::Region,
    pub location: Option<Location>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClaimDescriptionStateWithInventoryAndPlayTime {
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub region: entity::shared::Region,
    pub supplies: i32,
    pub building_maintenance: f32,
    pub members: HashMap<i64, ClaimDescriptionStateMember>,
    pub num_tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: Option<Location>,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub running_upgrade: Option<claim_tech_desc::Model>,
    pub running_upgrade_started: Option<entity::shared::timestamp::Timestamp>,
    pub tier: Option<i32>,
    pub rank: i32,
    pub upgrades: Vec<claim_tech_desc::Model>,
    pub learned_upgrades: Vec<i32>,
    pub inventorys: HashMap<String, Vec<entity::inventory::ExpendedRefrence>>,
    pub inventory_locations: HashMap<String, Vec<InventoryItemLocation>>,
    pub tool_inventorys: HashMap<String, Vec<entity::inventory::ExpendedRefrence>>,
    pub tool_inventory_locations: HashMap<String, Vec<InventoryItemLocation>>,
    pub traveler_tasks: HashMap<String, HashMap<i32, Vec<i64>>>,
    pub traveler_player_tasks:
        HashMap<String, HashMap<i64, HashMap<i32, Vec<entity::traveler_task_state::Model>>>>,
    pub time_signed_in: u64,
    pub building_states: Vec<building_state::Model>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) enum InventoryOwnerType {
    Player,
    Building,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct InventoryLocationEntry {
    pub inventory_entity_id: i64,
    pub owner_entity_id: i64,
    pub player_owner_entity_id: i64,
    pub inventory_index: i32,
    pub cargo_index: i32,
    pub owner_type: InventoryOwnerType,
    pub owner_name: Option<String>,
    pub building_name: Option<String>,
    pub building_description_id: Option<i32>,
    pub quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct InventoryItemLocation {
    pub item_id: i32,
    pub item: ItemExpended,
    pub item_type: ItemType,
    pub durability: Option<i32>,
    pub locations: Vec<InventoryLocationEntry>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ClaimResponse {
    pub claims: Vec<ClaimDescriptionState>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ClaimAuctionListingsResponse {
    pub buy_orders: Vec<entity::auction_listing_state::AuctionListingState>,
    pub sell_orders: Vec<entity::auction_listing_state::AuctionListingState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ClaimDescriptionStateMember {
    pub entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
    pub online_state: OnlineState,
    pub skills_ranks: Option<IndexMap<String, i64>>,
    pub inventory: Option<ResolvedInventory>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) enum OnlineState {
    Online,
    Offline,
}

pub(crate) async fn get_claim_tiles(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<Vec<entity::claim_tile_state::Model>>, (StatusCode, &'static str)> {
    let mut claim_tiles = Vec::new();
    let mut seen = HashSet::new();
    state
        .claim_tile_state
        .iter_async(|key, tile| {
            if tile.claim_id == id && seen.insert(*key) {
                claim_tiles.push(tile.clone());
            }
            true
        })
        .await;

    Ok(axum_codec::Codec(claim_tiles))
}

pub(crate) async fn get_claim_auction_listings(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<ClaimAuctionListingsResponse>, (StatusCode, &'static str)> {
    let buy_orders = crate::auction_listing_state::orders_in_claim(&state.buy_order_state, id).await;
    let sell_orders = crate::auction_listing_state::orders_in_claim(&state.sell_order_state, id).await;

    Ok(axum_codec::Codec(ClaimAuctionListingsResponse {
        buy_orders,
        sell_orders,
    }))
}

struct InventoryJobResult {
    key: String,
    inventorys: Vec<ExpendedRefrence>,
    locations: Vec<InventoryItemLocation>,
    tool_inventorys: Option<Vec<ExpendedRefrence>>,
    tool_locations: Option<Vec<InventoryItemLocation>>,
}

type FlatInventoryTasks = Vec<JoinHandle<anyhow::Result<InventoryJobResult>>>;
type FlatTravelerTasks = Vec<JoinHandle<anyhow::Result<(String, HashMap<i32, Vec<i64>>)>>>;

// Both Bank and Regional Bank use this building function.
const BANK_FUNCTION_TYPE: i32 = 622_192_982;
const SETTLEMENT_STASH_FUNCTION_TYPE: i32 = 1_721_785_854;

fn is_excluded_building_inventory(function_types: impl IntoIterator<Item = i32>) -> bool {
    function_types
        .into_iter()
        .any(|id| matches!(id, BANK_FUNCTION_TYPE | SETTLEMENT_STASH_FUNCTION_TYPE))
}

#[cfg(test)]
mod excluded_building_inventory_tests {
    use super::*;

    #[test]
    fn identifies_excluded_functions_among_other_building_functions() {
        assert!(is_excluded_building_inventory([1, BANK_FUNCTION_TYPE]));
        assert!(is_excluded_building_inventory([1, SETTLEMENT_STASH_FUNCTION_TYPE]));
        assert!(!is_excluded_building_inventory([1, 3]));
        assert!(!is_excluded_building_inventory([]));
    }
}

pub(crate) async fn get_claim_inventory_change_log(
    state: State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<InventoryChangesParams>,
) -> Result<axum_codec::Codec<Vec<inventory_changelog::Model>>, (StatusCode, &'static str)> {
    let building_states = QueryCore::find_building_state_by_claim_id(&state.conn, id)
        .await
        .unwrap_or_else(|err| {
            error!("Error loading building states: {err}");
            vec![]
        });

    let building_inventories_ids = building_states
        .iter()
        .map(|building| building.entity_id)
        .collect();

    let inventory_ids = QueryCore::find_inventory_entity_ids_by_owner_entity_ids(
        &state.conn,
        building_inventories_ids,
    )
    .await
    .unwrap_or_else(|err| {
        error!("Error loading find_inventory_entity_ids_by_owner_entity_ids: {err}");
        vec![]
    });
    let (start_time, end_time) = params.parsed_day_range()?;
    let item_filters = params.item_filters()?;

    let (inventory_changes, _num_pages) = QueryCore::find_inventory_changes_by_entity_ids(
        &state.conn,
        inventory_ids,
        1000,
        start_time,
        end_time,
        item_filters,
        params.user_id,
    )
    .await
    .map_err(|e| {
        error!("Error: {e:?}");

        (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
    })?;
    Ok(axum_codec::Codec(inventory_changes))
}

pub(crate) async fn get_claim_names(
    state: State<AppState>,
) -> Result<axum_codec::Codec<HashMap<i64, ClaimSummaryResponse>>, (StatusCode, &'static str)> {
    let mut claims = HashMap::new();

    let claim_summaries = state
        .claim_state
        .iter()
        .map(|claim| (claim.entity_id, claim.name.clone(), claim.region))
        .collect::<Vec<_>>();
    for (id, name, region) in claim_summaries {
        let location = state
            .claim_local_state
            .get(&(id as u64))
            .and_then(|claim_local_state| claim_local_state.location.clone());

        claims.insert(
            id,
            ClaimSummaryResponse {
                name,
                region,
                location,
            },
        );
    }

    Ok(axum_codec::Codec(claims))
}

pub(crate) async fn get_claim(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<
    axum_codec::Codec<ClaimDescriptionStateWithInventoryAndPlayTime>,
    (StatusCode, &'static str),
> {
    let cached_claim = state
        .claim_state
        .get(&(id as i64))
        .map(|claim| claim.value().clone());
    let claim = match cached_claim {
        Some(claim) => claim,
        None => {
            let claim = QueryCore::find_claim_state(&state.conn, id as i64)
                .await
                .expect("Cannot find posts in page");
            if claim.is_none() {
                return Err((StatusCode::NOT_FOUND, "Claim not found"));
            }

            claim.unwrap()
        }
    };

    let claim_tech_states =
        QueryCore::find_claim_tech_state_by_ids(&state.conn, vec![claim.entity_id])
            .await
            .expect("Cannot find claim tech states");

    if state.claim_tech_desc.is_empty() {
        let claim_tech_descs = QueryCore::all_claim_tech_desc(&state.conn)
            .await
            .expect("Cannot find claim tech descs");

        for claim_tech_desc in claim_tech_descs {
            state
                .claim_tech_desc
                .insert(claim_tech_desc.id, claim_tech_desc);
        }
    }

    let claim_tech_state = claim_tech_states
        .iter()
        .find(|state| state.entity_id == claim.entity_id);

    let claim_local_state = state
        .claim_local_state
        .get(&(claim.entity_id as u64))
        .map(|claim| claim.value().clone());

    let player_ids = state
        .claim_member_state
        .get(&(claim.entity_id as u64))
        .map_or(vec![], |claim_members| {
            claim_members
                .values()
                .map(|member| member.player_entity_id)
                .collect()
        });

    let currently_known_player_username_state = ::entity::player_username_state::Entity::find()
        .filter(::entity::player_username_state::Column::EntityId.is_in(player_ids))
        .all(&state.conn)
        .await
        .unwrap_or(vec![])
        .into_iter()
        .map(|value| (value.entity_id, value.username))
        .collect::<HashMap<_, _>>();

    let claim_member_state = state
        .claim_member_state
        .get(&(claim.entity_id as u64))
        .map_or(vec![], |claim_members| {
            claim_members
                .values()
                .map(|member| ClaimDescriptionStateMember {
                    entity_id: member.player_entity_id,
                    user_name: currently_known_player_username_state
                        .get(&member.player_entity_id)
                        .unwrap_or(&member.user_name.clone())
                        .clone(),
                    inventory_permission: member.inventory_permission,
                    build_permission: member.build_permission,
                    officer_permission: member.officer_permission,
                    co_owner_permission: member.co_owner_permission,
                    online_state: OnlineState::Offline,
                    skills_ranks: Some(IndexMap::new()),
                    inventory: None,
                })
                .collect()
        });

    let mut claim = ClaimDescriptionState {
        entity_id: claim.entity_id,
        owner_player_entity_id: claim.owner_player_entity_id,
        owner_building_entity_id: claim.owner_building_entity_id,
        name: claim.name,
        supplies: 0,
        building_maintenance: 0.0,
        members: claim_member_state,
        num_tiles: 0,
        extensions: 0,
        neutral: false,
        location: Default::default(),
        treasury: 0,
        region: claim.region,
        running_upgrade: None,
        running_upgrade_started: None,
        tier: None,
        rank: 4,
        stations: vec![],
        upgrades: vec![],
        xp_gained_since_last_coin_minting: 0,
    };

    let mut all_upgrades = state
        .claim_tech_desc
        .iter()
        .map(|entry| entry.value().clone())
        .collect::<Vec<claim_tech_desc::Model>>();
    all_upgrades.sort_by_key(|desc| desc.id);
    claim.upgrades = all_upgrades;

    if let Some(claim_local_state) = claim_local_state {
        claim.supplies = claim_local_state.supplies;
        claim.building_maintenance = claim_local_state.building_maintenance;
        claim.num_tiles = claim_local_state.num_tiles;
        claim.treasury = claim_local_state.treasury;
        claim.location = claim_local_state.location.clone();
        claim.xp_gained_since_last_coin_minting =
            claim_local_state.xp_gained_since_last_coin_minting;
    }

    let mut learned_upgrades = Vec::new();
    let building_states = QueryCore::find_building_state_by_claim_id(&state.conn, claim.entity_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Cannot load claim buildings"))?;
    let (building_tiers, _) = building_tiers_by_claim(&state.conn, &building_states)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Cannot load building descriptions"))?;
    let empty_tiers = HashSet::new();
    let claim_building_tiers = building_tiers.get(&claim.entity_id).unwrap_or(&empty_tiers);

    match claim_tech_state {
        Some(claim_tech_state) => {
            claim.running_upgrade = if state
                .tech_tier_research_map
                .contains_key(&claim_tech_state.researching)
            {
                Some(
                    state
                        .claim_tech_desc
                        .get(&claim_tech_state.researching)
                        .unwrap()
                        .value()
                        .clone(),
                )
            } else {
                None
            };
            claim.running_upgrade_started = Some(claim_tech_state.start_timestamp.clone());
            learned_upgrades = claim_tech_state.learned.clone();
            let (tier, rank) = claim_progression(&claim.upgrades, &learned_upgrades, claim_building_tiers);
            claim.tier = Some(tier);
            claim.rank = rank;
        }
        None => {
            claim.running_upgrade = None;
            let (tier, rank) = claim_progression(&claim.upgrades, &[], claim_building_tiers);
            claim.tier = Some(tier);
            claim.rank = rank;
        }
    };

    let cached_skills = {
        let skills = state.skill_desc.read();
        (!skills.is_empty()).then(|| {
            skills.values()
                .filter(|skill| skill.skill_category != 0)
                .map(|skill| (skill.id, skill.name.clone()))
                .collect::<Vec<_>>()
        })
    };
    let mut skills = match cached_skills {
        Some(skills) => skills,
        // The DB can lag buffered skill updates/removals. Use fallback rows
        // for this response without publishing them to the live XP worker.
        None => service::Query::skill_descriptions(&state.conn)
            .await
            .map_err(|error| {
                error!("Error: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?
            .into_iter()
            .filter(|skill| skill.skill_category != 0)
            .map(|skill| (skill.id, skill.name))
            .collect(),
    };
    skills.sort_by(|(a_id, a), (b_id, b)| (skill_sort_key(a), a_id).cmp(&(skill_sort_key(b), b_id)));
    for (skill_id, skill_name) in skills {
        if let Some(a) = state.ranking_system.skill_leaderboard(skill_id) {
            for member in claim.members.iter_mut() {
                let xp = if let Some(xp) = a.get_value(&member.entity_id) {
                    xp
                } else {
                    tracing::warn!(
                        member.entity_id,
                        skill_id,
                        "Could not find player skill experience"
                    );

                    0
                };
                let skills_ranks = member.skills_ranks.get_or_insert_with(IndexMap::new);
                skills_ranks.insert(skill_name.clone(), xp);
            }
        } else {
            continue;
        };
    }
    for member in &mut claim.members {
        let inventory = QueryCore::find_inventory_by_owner_entity_ids_and_index(
            &state.conn,
            vec![member.entity_id],
            1,
        )
        .await
        .map_err(|e| {
            error!("Error: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;
        let mut pockets = vec![];
        if let Some(inventory) = inventory.first() {
            for pocket in &inventory.pockets {
                pockets.push(resolve_pocket(pocket, &state.item_desc, &state.cargo_desc));
            }
            member.inventory = Some(inventory::ResolvedInventory {
                entity_id: inventory.entity_id,
                region: inventory.region,
                pockets,
                inventory_index: inventory.inventory_index,
                cargo_index: inventory.cargo_index,
                owner_entity_id: inventory.owner_entity_id,
                player_owner_entity_id: inventory.player_owner_entity_id,
                nickname: None,
                claim: None,
            });
        }
    }

    let building_inventories_ids = building_states
        .iter()
        .filter(|building| {
            state
                .building_desc
                .get(&(building.building_description_id as i64))
                .is_none_or(|desc| {
                    !is_excluded_building_inventory(
                        desc.functions.iter().map(|function| function.function_type),
                    )
                })
        })
        .map(|building| building.entity_id)
        .collect();

    let player_inventories_ids = claim
        .members
        .iter()
        .map(|member| member.entity_id)
        .collect::<Vec<_>>();

    let current_players = player_inventories_ids
        .iter()
        .filter_map(|player_id| {
            state
                .player_state
                .get(player_id)
                .map(|player| player.value().clone())
        })
        .collect::<Vec<entity::player_state::Model>>();

    let total_time_signed_in = current_players
        .iter()
        .map(|player| player.time_signed_in as u64)
        .sum();

    let offline_players = current_players
        .clone()
        .into_iter()
        .filter(|player| !player.signed_in)
        .collect::<Vec<player_state::Model>>();
    let online_players = current_players
        .into_iter()
        .filter(|player| player.signed_in)
        .collect::<Vec<player_state::Model>>();

    for player in &online_players {
        if let Some(player_online) = claim
            .members
            .iter_mut()
            .find(|player_online| player_online.entity_id == player.entity_id)
        {
            player_online.online_state = OnlineState::Online;
        }
    }

    let player_online_ids: Vec<i64> = online_players
        .iter()
        .map(|player| player.entity_id)
        .collect();
    let player_offline_ids: Vec<i64> = offline_players
        .iter()
        .map(|player| player.entity_id)
        .collect();

    let mut claim = ClaimDescriptionStateWithInventoryAndPlayTime {
        entity_id: claim.entity_id,
        owner_player_entity_id: claim.owner_player_entity_id,
        owner_building_entity_id: claim.owner_building_entity_id,
        name: claim.name,
        region: claim.region,
        supplies: claim.supplies,
        building_maintenance: claim.building_maintenance,
        members: claim
            .members
            .into_iter()
            .map(|member| (member.entity_id, member))
            .collect(),
        num_tiles: claim.num_tiles,
        extensions: claim.extensions,
        neutral: claim.neutral,
        location: claim.location,
        treasury: claim.treasury,
        running_upgrade: claim.running_upgrade,
        running_upgrade_started: claim.running_upgrade_started,
        xp_gained_since_last_coin_minting: claim.xp_gained_since_last_coin_minting,
        tier: claim.tier,
        rank: claim.rank,
        upgrades: claim.upgrades,
        learned_upgrades,
        inventorys: HashMap::new(),
        inventory_locations: HashMap::new(),
        tool_inventorys: HashMap::new(),
        tool_inventory_locations: HashMap::new(),
        traveler_tasks: HashMap::new(),
        traveler_player_tasks: HashMap::new(),
        time_signed_in: total_time_signed_in,
        building_states,
    };

    let mut jobs: FlatInventoryTasks = vec![];
    let mut traveler_task_jobs: FlatTravelerTasks = vec![];
    let member_names: HashMap<i64, String> = claim
        .members
        .values()
        .map(|member| (member.entity_id, member.user_name.clone()))
        .collect();
    let building_desc_map: HashMap<i64, i32> = claim
        .building_states
        .iter()
        .map(|building| (building.entity_id, building.building_description_id))
        .collect();
    let building_names: HashMap<i64, String> = claim
        .building_states
        .iter()
        .filter_map(|building| {
            if let Some(nickname) = state.building_nickname_state.get(&building.entity_id) {
                return Some((building.entity_id, nickname.nickname.clone()));
            }
            let desc_id = building_desc_map.get(&building.entity_id)?;
            let desc = state.building_desc.get(&(*desc_id as i64))?;
            Some((building.entity_id, desc.name.clone()))
        })
        .collect();

    let conn: sea_orm::DatabaseConnection = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    let player_offline_ids_local = player_offline_ids.clone();
    let member_names_local = member_names.clone();
    let building_desc_map_local = building_desc_map.clone();
    let building_names_local = building_names.clone();
    jobs.push(tokio::spawn(async move {
        let offline_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_offline_ids_local).await?;
        let (offline_players_inventories, offline_players_tool_inventories) =
            split_tool_inventories(offline_players_inventories);
        let inventory_locations = get_inventory_locations(
            offline_players_inventories.clone(),
            &tmp_item_desc,
            &tmp_cargo_desc,
            &member_names_local,
            &building_desc_map_local,
            &building_names_local,
        );
        let mut merged_offline_players_inventories =
            get_merged_inventories(offline_players_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_offline_players_inventories.sort_by(inventory_sort_by);

        let tool_locations = get_inventory_locations(
            offline_players_tool_inventories.clone(),
            &tmp_item_desc,
            &tmp_cargo_desc,
            &member_names_local,
            &building_desc_map_local,
            &building_names_local,
        );
        let mut merged_offline_players_tool_inventories = get_merged_inventories(
            offline_players_tool_inventories,
            &tmp_item_desc,
            &tmp_cargo_desc,
        );
        merged_offline_players_tool_inventories.sort_by(inventory_sort_by);

        Ok(InventoryJobResult {
            key: "players_offline".to_string(),
            inventorys: merged_offline_players_inventories,
            locations: inventory_locations,
            tool_inventorys: Some(merged_offline_players_tool_inventories),
            tool_locations: Some(tool_locations),
        })
    }));

    let conn: sea_orm::DatabaseConnection = state.conn.clone();
    traveler_task_jobs.push(tokio::spawn(async move {
        let offline_traveler_tasks =
            QueryCore::get_traveler_task_state_by_player_entity_ids(&conn, player_offline_ids)
                .await?;

        let mut traveler_tasks = HashMap::new();
        for task in offline_traveler_tasks {
            let tasks = traveler_tasks.entry(task.task_id).or_insert_with(Vec::new);
            if !task.completed {
                tasks.push(task.player_entity_id);
            }
        }
        Ok(("players_offline".to_string(), traveler_tasks))
    }));

    let conn = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    let member_names_local = member_names.clone();
    let building_desc_map_local = building_desc_map.clone();
    let building_names_local = building_names.clone();
    jobs.push(tokio::spawn(async move {
        let claim_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, building_inventories_ids).await?;
        let inventory_locations = get_inventory_locations(
            claim_inventories.clone(),
            &tmp_item_desc,
            &tmp_cargo_desc,
            &member_names_local,
            &building_desc_map_local,
            &building_names_local,
        );
        let mut merged_claim_inventories =
            get_merged_inventories(claim_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_claim_inventories.sort_by(inventory_sort_by);

        Ok(InventoryJobResult {
            key: "buildings".to_string(),
            inventorys: merged_claim_inventories,
            locations: inventory_locations,
            tool_inventorys: None,
            tool_locations: None,
        })
    }));

    let conn = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    let player_online_ids_local = player_online_ids.clone();
    let member_names_local = member_names.clone();
    let building_desc_map_local = building_desc_map.clone();
    let building_names_local = building_names.clone();
    jobs.push(tokio::spawn(async move {
        let online_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_online_ids_local).await?;
        let (online_players_inventories, online_players_tool_inventories) =
            split_tool_inventories(online_players_inventories);
        let inventory_locations = get_inventory_locations(
            online_players_inventories.clone(),
            &tmp_item_desc,
            &tmp_cargo_desc,
            &member_names_local,
            &building_desc_map_local,
            &building_names_local,
        );
        let mut merged_online_players_inventories =
            get_merged_inventories(online_players_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_online_players_inventories.sort_by(inventory_sort_by);

        let tool_locations = get_inventory_locations(
            online_players_tool_inventories.clone(),
            &tmp_item_desc,
            &tmp_cargo_desc,
            &member_names_local,
            &building_desc_map_local,
            &building_names_local,
        );
        let mut merged_online_players_tool_inventories = get_merged_inventories(
            online_players_tool_inventories,
            &tmp_item_desc,
            &tmp_cargo_desc,
        );
        merged_online_players_tool_inventories.sort_by(inventory_sort_by);

        Ok(InventoryJobResult {
            key: "players".to_string(),
            inventorys: merged_online_players_inventories,
            locations: inventory_locations,
            tool_inventorys: Some(merged_online_players_tool_inventories),
            tool_locations: Some(tool_locations),
        })
    }));

    let conn: sea_orm::DatabaseConnection = state.conn.clone();
    traveler_task_jobs.push(tokio::spawn(async move {
        let offline_traveler_tasks =
            QueryCore::get_traveler_task_state_by_player_entity_ids(&conn, player_online_ids)
                .await?;

        let mut traveler_tasks = HashMap::new();
        for task in offline_traveler_tasks {
            let tasks = traveler_tasks.entry(task.task_id).or_insert_with(Vec::new);
            if !task.completed {
                tasks.push(task.player_entity_id);
            }
        }
        Ok(("players".to_string(), traveler_tasks))
    }));

    let finished_jobs = futures::future::join_all(jobs).await;

    for finished_job in finished_jobs {
        if let Err(err) = finished_job {
            error!("Error: {err:?}");
            continue;
        }

        let job = finished_job.unwrap();

        if let Ok(job) = job {
            claim
                .inventorys
                .insert(job.key.parse().unwrap(), job.inventorys);
            claim
                .inventory_locations
                .insert(job.key.parse().unwrap(), job.locations);
            if let (Some(tool_inventorys), Some(tool_locations)) =
                (job.tool_inventorys, job.tool_locations)
            {
                claim
                    .tool_inventorys
                    .insert(job.key.parse().unwrap(), tool_inventorys);
                claim
                    .tool_inventory_locations
                    .insert(job.key.parse().unwrap(), tool_locations);
            }
        }
    }

    let finished_jobs = futures::future::join_all(traveler_task_jobs).await;

    for finished_job in finished_jobs {
        if let Err(err) = finished_job {
            error!("Error: {err:?}");
            continue;
        }

        let job = finished_job.unwrap();

        if let Ok((key, value)) = job {
            claim.traveler_tasks.insert(key.parse().unwrap(), value);
        }
    }

    Ok(axum_codec::Codec(claim))
}

fn inventory_sort_by(a: &ExpendedRefrence, b: &ExpendedRefrence) -> Ordering {
    let (b_tier, a_tier) = match (b.item.clone(), a.item.clone()) {
        (ItemExpended::Cargo(cargo_b), ItemExpended::Item(cargo_a)) => (cargo_b.tier, cargo_a.tier),
        (ItemExpended::Item(item_b), ItemExpended::Cargo(item_a)) => (item_b.tier, item_a.tier),
        (ItemExpended::Item(item_b), ItemExpended::Item(item_a)) => (item_b.tier, item_a.tier),
        (ItemExpended::Cargo(cargo_b), ItemExpended::Cargo(cargo_a)) => {
            (cargo_b.tier, cargo_a.tier)
        }
    };

    if b_tier == a_tier {
        b.quantity.cmp(&a.quantity)
    } else {
        b_tier.cmp(&a_tier)
    }
}

fn split_tool_inventories(
    inventorys: Vec<inventory::Model>,
) -> (Vec<inventory::Model>, Vec<inventory::Model>) {
    let mut standard_inventories = Vec::new();
    let mut tool_inventories = Vec::new();

    for inventory in inventorys {
        if inventory.inventory_index == 1 {
            tool_inventories.push(inventory);
        } else {
            standard_inventories.push(inventory);
        }
    }

    (standard_inventories, tool_inventories)
}

#[derive(Deserialize)]
pub(crate) struct ListClaimsParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    research: Option<i32>,
    running_upgrade: Option<bool>,
}

pub(crate) async fn list_claims(
    state: State<AppState>,
    Query(params): Query<ListClaimsParams>,
) -> Result<axum_codec::Codec<ClaimResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(25);
    let search = params.search;

    let (claims, num_pages) = QueryCore::find_claim_descriptions(
        &state.conn,
        page,
        posts_per_page,
        search,
        params.research,
        params.running_upgrade,
    )
    .await
    .expect("Cannot find posts in page");

    let claim_tech_states = QueryCore::find_claim_tech_state_by_ids(
        &state.conn,
        claims.iter().map(|claim| claim.entity_id).collect(),
    )
    .await
    .expect("Cannot find claim tech states");
    let claim_tech_descs = QueryCore::all_claim_tech_desc(&state.conn)
        .await
        .expect("Cannot find claim tech descs");
    let buildings = building_state::Entity::find()
        .filter(building_state::Column::ClaimEntityId.is_in(claims.iter().map(|claim| claim.entity_id)))
        .all(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Cannot load claim buildings"))?;
    let (building_tiers, stations_by_claim) = building_tiers_by_claim(&state.conn, &buildings)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Cannot load building descriptions"))?;
    let empty_tiers = HashSet::new();

    let claims = claims
        .into_iter()
        .map(|claim_description| {
            let claim_tech_state = claim_tech_states
                .iter()
                .find(|state| state.entity_id == claim_description.entity_id);

            let claim_local_state = state
                .claim_local_state
                .get(&(claim_description.entity_id as u64))
                .map(|claim| claim.value().clone());

            let claim_member_state = state
                .claim_member_state
                .get(&(claim_description.entity_id as u64))
                .map_or(vec![], |claim_members| {
                    claim_members
                        .values()
                        .map(|member| ClaimDescriptionStateMember {
                            entity_id: member.player_entity_id,
                            user_name: member.user_name.clone(),
                            inventory_permission: member.inventory_permission,
                            build_permission: member.build_permission,
                            officer_permission: member.officer_permission,
                            co_owner_permission: member.co_owner_permission,
                            online_state: OnlineState::Offline,
                            skills_ranks: Some(IndexMap::new()),
                            inventory: None,
                        })
                        .collect()
                });

            let mut claim_description = ClaimDescriptionState {
                entity_id: claim_description.entity_id,
                owner_player_entity_id: claim_description.owner_player_entity_id,
                owner_building_entity_id: claim_description.owner_building_entity_id,
                name: claim_description.name,
                region: claim_description.region,
                supplies: 0,
                building_maintenance: 0.0,
                members: claim_member_state,
                num_tiles: 0,
                extensions: 0,
                neutral: false,
                location: Default::default(),
                treasury: 0,
                running_upgrade: None,
                running_upgrade_started: None,
                tier: None,
                rank: 4,
                stations: stations_by_claim.get(&claim_description.entity_id).cloned().unwrap_or_default(),
                upgrades: vec![],
                xp_gained_since_last_coin_minting: 0,
            };

            if let Some(claim_local_state) = claim_local_state {
                claim_description.supplies = claim_local_state.supplies;
                claim_description.building_maintenance = claim_local_state.building_maintenance;
                claim_description.num_tiles = claim_local_state.num_tiles;
                claim_description.treasury = claim_local_state.treasury;
                claim_description.location = claim_local_state.location.clone();
                claim_description.xp_gained_since_last_coin_minting =
                    claim_local_state.xp_gained_since_last_coin_minting;
            }

            match claim_tech_state {
                Some(claim_tech_state) => {
                    claim_description.running_upgrade = if state
                        .tech_tier_research_map
                        .contains_key(&claim_tech_state.researching)
                    {
                        Some(
                            state
                                .claim_tech_desc
                                .get(&claim_tech_state.researching)
                                .unwrap()
                                .value()
                                .clone(),
                        )
                    } else {
                        None
                    };
                    let learned: Vec<i32> = claim_tech_state.learned.clone();
                    claim_description.upgrades = learned
                        .iter()
                        .filter_map(|id| claim_tech_descs.iter().find(|desc| desc.id == (*id)))
                        .cloned()
                        .collect::<Vec<claim_tech_desc::Model>>();
                    let (tier, rank) = claim_progression(&claim_tech_descs, &learned, building_tiers.get(&claim_description.entity_id).unwrap_or(&empty_tiers));
                    claim_description.tier = Some(tier);
                    claim_description.rank = rank;
                }
                None => {
                    claim_description.running_upgrade = None;
                    claim_description.upgrades = vec![];
                    let (tier, rank) = claim_progression(&claim_tech_descs, &[], building_tiers.get(&claim_description.entity_id).unwrap_or(&empty_tiers));
                    claim_description.tier = Some(tier);
                    claim_description.rank = rank;
                }
            };

            claim_description
        })
        .collect::<Vec<ClaimDescriptionState>>();

    Ok(axum_codec::Codec(ClaimResponse {
        claims,
        per_page: posts_per_page,
        total: num_pages.number_of_items,
        page,
    }))
}

pub(crate) fn get_merged_inventories(
    inventorys: Vec<inventory::Model>,
    items: &Arc<dashmap::DashMap<i32, item_desc::Model>>,
    cargos: &Arc<dashmap::DashMap<i32, cargo_desc::Model>>,
) -> Vec<entity::inventory::ExpendedRefrence> {
    let mut hashmap: HashMap<
        (i32, entity::inventory::ItemType),
        entity::inventory::ExpendedRefrence,
    > = HashMap::new();

    for inventory in inventorys {
        for pocket in inventory.pockets {
            let resolved = resolve_contents(&pocket.contents, items, cargos);

            if resolved.is_none() {
                continue;
            }

            let resolved = resolved.unwrap();

            match hashmap.get_mut(&(resolved.item_id, resolved.item_type.clone())) {
                Some(value) => {
                    value.quantity += resolved.quantity;
                }
                None => {
                    hashmap.insert((resolved.item_id, resolved.item_type.clone()), resolved);
                }
            }
        }
    }

    hashmap.into_values().collect()
}

pub(crate) fn get_inventory_locations(
    inventorys: Vec<inventory::Model>,
    items: &Arc<dashmap::DashMap<i32, item_desc::Model>>,
    cargos: &Arc<dashmap::DashMap<i32, cargo_desc::Model>>,
    member_names: &HashMap<i64, String>,
    building_desc_map: &HashMap<i64, i32>,
    building_names: &HashMap<i64, String>,
) -> Vec<InventoryItemLocation> {
    struct ItemLocationsBuilder {
        item_id: i32,
        item: ItemExpended,
        item_type: ItemType,
        durability: Option<i32>,
        locations: HashMap<i64, InventoryLocationEntry>,
    }

    let mut items_map: HashMap<(i32, ItemType), ItemLocationsBuilder> = HashMap::new();

    for inventory in inventorys {
        for pocket in inventory.pockets {
            let resolved = resolve_contents(&pocket.contents, items, cargos);

            if resolved.is_none() {
                continue;
            }

            let resolved = resolved.unwrap();
            let key = (resolved.item_id, resolved.item_type.clone());
            let owner_type = if building_desc_map.contains_key(&inventory.owner_entity_id) {
                InventoryOwnerType::Building
            } else if member_names.contains_key(&inventory.owner_entity_id) {
                InventoryOwnerType::Player
            } else {
                InventoryOwnerType::Unknown
            };
            let owner_name = match owner_type {
                InventoryOwnerType::Player => member_names.get(&inventory.owner_entity_id).cloned(),
                _ => None,
            };
            let building_name = match owner_type {
                InventoryOwnerType::Building => {
                    building_names.get(&inventory.owner_entity_id).cloned()
                }
                _ => None,
            };
            let building_description_id =
                building_desc_map.get(&inventory.owner_entity_id).copied();

            let entry = items_map
                .entry(key)
                .or_insert_with(|| ItemLocationsBuilder {
                    item_id: resolved.item_id,
                    item: resolved.item.clone(),
                    item_type: resolved.item_type.clone(),
                    durability: resolved.durability,
                    locations: HashMap::new(),
                });

            let location_entry = entry
                .locations
                .entry(inventory.entity_id)
                .or_insert_with(|| InventoryLocationEntry {
                    inventory_entity_id: inventory.entity_id,
                    owner_entity_id: inventory.owner_entity_id,
                    player_owner_entity_id: inventory.player_owner_entity_id,
                    inventory_index: inventory.inventory_index,
                    cargo_index: inventory.cargo_index,
                    owner_type: owner_type.clone(),
                    owner_name: owner_name.clone(),
                    building_name: building_name.clone(),
                    building_description_id,
                    quantity: 0,
                });

            location_entry.quantity += resolved.quantity;
        }
    }

    items_map
        .into_values()
        .map(|entry| {
            let mut locations = entry.locations.into_values().collect::<Vec<_>>();
            locations.sort_by_key(|b| std::cmp::Reverse(b.quantity));
            InventoryItemLocation {
                item_id: entry.item_id,
                item: entry.item,
                item_type: entry.item_type,
                durability: entry.durability,
                locations,
            }
        })
        .collect()
}
