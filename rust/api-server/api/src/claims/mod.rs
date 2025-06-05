pub(crate) mod claim_local_state;
pub(crate) mod claim_member_state;
pub(crate) mod claim_state;
use crate::inventory::{resolve_contents, resolve_pocket};
use crate::leaderboard::{EXCLUDED_USERS_FROM_LEADERBOARD, LeaderboardSkill, experience_to_level};
use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::inventory::{ExpendedRefrence, ItemExpended, ResolvedInventory};
use entity::shared::location::Location;
use entity::{building_state, cargo_desc, claim_tech_desc, inventory, item_desc, player_state};
use log::error;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::task::JoinHandle;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/claims", axum_codec::routing::get(list_claims).into())
        .route(
            "/api/bitcraft/claims",
            axum_codec::routing::get(list_claims).into(),
        )
        .route(
            "/api/bitcraft/claims/{id}",
            axum_codec::routing::get(get_claim).into(),
        )
        // .route(
        //     "/claims/{id}",
        //     axum_codec::routing::get(find_claim_descriptions).into(),
        // )
        .route(
            "/claims/tiles/{id}",
            axum_codec::routing::get(get_claim_tiles).into(),
        )
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub upgrades: Vec<claim_tech_desc::Model>,
    pub xp_gained_since_last_coin_minting: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClaimDescriptionStateWithInventoryAndPlayTime {
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
    pub xp_gained_since_last_coin_minting: i32,
    pub running_upgrade: Option<claim_tech_desc::Model>,
    pub running_upgrade_started: Option<entity::shared::timestamp::Timestamp>,
    pub tier: Option<i32>,
    pub upgrades: Vec<claim_tech_desc::Model>,
    pub inventorys: HashMap<String, Vec<entity::inventory::ExpendedRefrence>>,
    pub time_signed_in: u64,
    pub building_states: Vec<building_state::Model>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ClaimResponse {
    pub claims: Vec<ClaimDescriptionState>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ClaimDescriptionStateMemberTmp {
    pub entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
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
    pub skills_ranks: Option<BTreeMap<String, LeaderboardSkill>>,
    pub inventory: Option<ResolvedInventory>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) enum OnlineState {
    Online,
    Offline,
}

pub(crate) async fn get_claim_tiles(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<Vec<entity::claim_tile_state::Model>>, (StatusCode, &'static str)> {
    let claim_tiles = state
        .claim_tile_state
        .iter()
        .filter(|a| a.claim_id == id)
        .collect::<Vec<_>>();

    Ok(axum_codec::Codec(
        claim_tiles.iter().map(|a| a.value().clone()).collect(),
    ))
}

type ClaimLeaderboardTasks =
    Vec<JoinHandle<Result<(String, Vec<LeaderboardSkill>), (StatusCode, &'static str)>>>;

type FlatInventoryTasks = Vec<JoinHandle<anyhow::Result<(String, Vec<ExpendedRefrence>)>>>;

pub(crate) async fn get_claim(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<
    axum_codec::Codec<ClaimDescriptionStateWithInventoryAndPlayTime>,
    (StatusCode, &'static str),
> {
    let claim = QueryCore::find_claim_state(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if claim.is_none() {
        return Err((StatusCode::NOT_FOUND, "Claim not found"));
    }

    let claim = claim.unwrap();

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

    let tier_upgrades = state
        .claim_tech_desc
        .iter()
        .filter_map(|desc| {
            if desc.description.starts_with("Tier ") {
                return Some(desc.to_owned());
            };

            None
        })
        .collect::<Vec<claim_tech_desc::Model>>();
    let tier_upgrades_ids = tier_upgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i32>>();

    let mut claim = {
        let claim_tech_state = claim_tech_states
            .iter()
            .find(|state| state.entity_id == claim.entity_id);

        let claim_local_state = state.claim_local_state.get(&(claim.entity_id as u64));

        let claim_member_state = state
            .claim_member_state
            .get(&(claim.entity_id as u64))
            .map_or(vec![], |claim_members| {
                claim_members
                    .iter()
                    .map(|member| ClaimDescriptionStateMember {
                        entity_id: member.player_entity_id,
                        user_name: member.user_name.clone(),
                        inventory_permission: member.inventory_permission,
                        build_permission: member.build_permission,
                        officer_permission: member.officer_permission,
                        co_owner_permission: member.co_owner_permission,
                        online_state: OnlineState::Offline,
                        skills_ranks: Some(BTreeMap::new()),
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
            running_upgrade: None,
            running_upgrade_started: None,
            tier: None,
            upgrades: vec![],
            xp_gained_since_last_coin_minting: 0,
        };

        if let Some(claim_local_state) = claim_local_state {
            claim.supplies = claim_local_state.supplies;
            claim.building_maintenance = claim_local_state.building_maintenance;
            claim.num_tiles = claim_local_state.num_tiles;
            claim.treasury = claim_local_state.treasury;
            claim.location = claim_local_state.location.clone();
            claim.xp_gained_since_last_coin_minting =
                claim_local_state.xp_gained_since_last_coin_minting;
        }

        match claim_tech_state {
            Some(claim_tech_state) => {
                claim.running_upgrade = tier_upgrades
                    .iter()
                    .find(|desc| desc.id == claim_tech_state.researching)
                    .cloned();
                claim.running_upgrade_started = Some(claim_tech_state.start_timestamp.clone());
                let learned: Vec<i32> = claim_tech_state.learned.clone();
                claim.upgrades = learned
                    .iter()
                    .map(|id| {
                        state
                            .claim_tech_desc
                            .iter()
                            .find(|desc| desc.id == (*id))
                            .unwrap()
                            .clone()
                    })
                    .collect::<Vec<claim_tech_desc::Model>>();
                let found_tiers = learned
                    .iter()
                    .filter(|id| tier_upgrades_ids.contains(&(**id)))
                    .copied()
                    .collect::<Vec<i32>>();

                if !found_tiers.is_empty() {
                    claim.tier = tier_upgrades
                        .iter()
                        .find(|desc| desc.id == (found_tiers[found_tiers.len() - 1]))
                        .map(|desc| desc.tier);
                } else {
                    claim.tier = Some(1);
                }
            }
            None => {
                claim.running_upgrade = None;
                claim.upgrades = vec![];
                claim.tier = Some(1);
            }
        };

        claim
    };

    if state.skill_desc.is_empty() {
        let skills = service::Query::skill_descriptions(&state.conn)
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

        for skill in skills {
            state.skill_desc.insert(skill.id, skill.clone());
        }
    }

    let mut tasks: ClaimLeaderboardTasks = vec![];
    let player_ids = claim
        .members
        .iter()
        .map(|member| member.entity_id)
        .collect::<Vec<i64>>();

    for skill in state.skill_desc.iter() {
        if skill.name == "ANY" {
            continue;
        }

        if skill.name == "Lore Keeper" {
            continue;
        }

        if skill.name == "Trading" {
            continue;
        }

        if skill.name == "Taming" {
            continue;
        }

        if skill.name == "Exploration" {
            continue;
        }

        let db = state.conn.clone();
        let player_ids = player_ids.clone();

        let skill_id = skill.id;
        let skill_name = skill.name.clone();
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<LeaderboardSkill> = Vec::new();
            let entries = service::Query::get_experience_state_player_ids_by_skill_id(
                &db,
                skill_id,
                player_ids,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            for (i, entry) in entries.into_iter().enumerate() {
                let rank = i + 1;
                let player_name = None;

                leaderboard.push(LeaderboardSkill {
                    player_id: entry.entity_id,
                    player_name,
                    experience: entry.experience,
                    level: experience_to_level(entry.experience as i64),
                    rank: rank as u64,
                });
            }

            Ok((skill_name, leaderboard))
        }));
    }
    for member in &mut claim.members {
        let (inventorys, num_pages) = QueryCore::find_inventory_by_owner_entity_ids(
            &state.conn,
            vec![member.entity_id.clone()],
        )
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;
        let mut pockets = vec![];
        if let Some(inventory) = inventorys.iter().find(|inv| inv.inventory_index == 1) {
            for pocket in &inventory.pockets {
                pockets.push(resolve_pocket(pocket, &state.item_desc, &state.cargo_desc));
            }
            member.inventory = Some(inventory::ResolvedInventory {
                entity_id: inventory.entity_id,
                pockets,
                inventory_index: inventory.inventory_index,
                cargo_index: inventory.cargo_index,
                owner_entity_id: inventory.owner_entity_id,
                player_owner_entity_id: inventory.player_owner_entity_id,
                nickname: None,
            });
        }
    }

    let results = futures::future::join_all(tasks).await;

    for result in results {
        if let Err(err) = result {
            error!("Error: {:?}", err);
            continue;
        }

        let result = result.unwrap();

        if let Err(err) = result {
            error!("Error: {:?}", err);
            continue;
        }

        let (key, value) = result?;

        for member in &mut claim.members {
            let leaderboard = value
                .iter()
                .find(|entry| entry.player_id == member.entity_id);

            if let Some(leaderboard) = leaderboard {
                if member.skills_ranks.is_none() {
                    member.skills_ranks = Some(BTreeMap::new());
                }

                let skills_ranks = member.skills_ranks.as_mut().unwrap();
                skills_ranks.insert(key.clone(), leaderboard.clone());
            }
        }
    }

    let building_states = QueryCore::find_building_state_by_claim_id(&state.conn, claim.entity_id)
        .await
        .unwrap_or_else(|err| {
            error!("Error loading building states: {err}");
            vec![]
        });

    let building_inventories_ids = building_states
        .iter()
        .map(|building| building.entity_id)
        .collect();

    let player_inventories_ids = claim
        .members
        .iter()
        .map(|member| member.entity_id)
        .collect();

    let current_players = QueryCore::find_player_by_ids(&state.conn, player_inventories_ids)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, ""))?;

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

    let player_online_ids = online_players
        .iter()
        .map(|player| player.entity_id)
        .collect();
    let player_offline_ids = offline_players
        .iter()
        .map(|player| player.entity_id)
        .collect();

    let mut claim = ClaimDescriptionStateWithInventoryAndPlayTime {
        entity_id: claim.entity_id,
        owner_player_entity_id: claim.owner_player_entity_id,
        owner_building_entity_id: claim.owner_building_entity_id,
        name: claim.name,
        supplies: claim.supplies,
        building_maintenance: claim.building_maintenance,
        members: claim.members,
        num_tiles: claim.num_tiles,
        extensions: claim.extensions,
        neutral: claim.neutral,
        location: claim.location,
        treasury: claim.treasury,
        running_upgrade: claim.running_upgrade,
        running_upgrade_started: claim.running_upgrade_started,
        xp_gained_since_last_coin_minting: claim.xp_gained_since_last_coin_minting,
        tier: claim.tier,
        upgrades: claim.upgrades,
        inventorys: HashMap::new(),
        time_signed_in: total_time_signed_in,
        building_states,
    };

    let mut jobs: FlatInventoryTasks = vec![];

    let conn = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    jobs.push(tokio::spawn(async move {
        let offline_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_offline_ids).await?;
        let mut merged_offline_players_inventories =
            get_merged_inventories(offline_players_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_offline_players_inventories.sort_by(inventory_sort_by);

        Ok((
            "players_offline".to_string(),
            merged_offline_players_inventories,
        ))
    }));

    let conn = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    jobs.push(tokio::spawn(async move {
        let claim_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, building_inventories_ids).await?;
        let mut merged_claim_inventories =
            get_merged_inventories(claim_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_claim_inventories.sort_by(inventory_sort_by);

        Ok(("buildings".to_string(), merged_claim_inventories))
    }));

    let conn = state.conn.clone();
    let tmp_item_desc = state.item_desc.clone();
    let tmp_cargo_desc = state.cargo_desc.clone();
    jobs.push(tokio::spawn(async move {
        let online_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_online_ids).await?;
        let mut merged_online_players_inventories =
            get_merged_inventories(online_players_inventories, &tmp_item_desc, &tmp_cargo_desc);
        merged_online_players_inventories.sort_by(inventory_sort_by);

        Ok(("players".to_string(), merged_online_players_inventories))
    }));

    let finished_jobs = futures::future::join_all(jobs).await;

    for finished_job in finished_jobs {
        if let Err(err) = finished_job {
            error!("Error: {:?}", err);
            continue;
        }

        let job = finished_job.unwrap();

        if let Ok((key, value)) = job {
            claim.inventorys.insert(key.parse().unwrap(), value);
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

#[derive(Deserialize)]
pub(crate) struct ListClaimsParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    research: Option<i32>,
    running_upgrade: Option<bool>,
}

pub(crate) async fn list_claims(
    state: State<std::sync::Arc<AppState>>,
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
    let tier_upgrades = claim_tech_descs
        .iter()
        .filter(|desc| desc.description.starts_with("Tier "))
        .cloned()
        .collect::<Vec<claim_tech_desc::Model>>();
    let tier_upgrades_ids = tier_upgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i32>>();

    let claims = claims
        .into_iter()
        .map(|claim_description| {
            let claim_tech_state = claim_tech_states
                .iter()
                .find(|state| state.entity_id == claim_description.entity_id);

            let claim_local_state = state
                .claim_local_state
                .get(&(claim_description.entity_id as u64));

            let claim_member_state = state
                .claim_member_state
                .get(&(claim_description.entity_id as u64))
                .map_or(vec![], |claim_members| {
                    claim_members
                        .iter()
                        .map(|member| ClaimDescriptionStateMember {
                            entity_id: member.player_entity_id,
                            user_name: member.user_name.clone(),
                            inventory_permission: member.inventory_permission,
                            build_permission: member.build_permission,
                            officer_permission: member.officer_permission,
                            co_owner_permission: member.co_owner_permission,
                            online_state: OnlineState::Offline,
                            skills_ranks: Some(BTreeMap::new()),
                            inventory: None,
                        })
                        .collect()
                });

            let mut claim_description = ClaimDescriptionState {
                entity_id: claim_description.entity_id,
                owner_player_entity_id: claim_description.owner_player_entity_id,
                owner_building_entity_id: claim_description.owner_building_entity_id,
                name: claim_description.name,
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
                    claim_description.running_upgrade = tier_upgrades
                        .iter()
                        .find(|desc| desc.id == claim_tech_state.researching)
                        .cloned();
                    let learned: Vec<i32> = claim_tech_state.learned.clone();
                    claim_description.upgrades = learned
                        .iter()
                        .map(|id| {
                            claim_tech_descs
                                .iter()
                                .find(|desc| desc.id == (*id))
                                .unwrap()
                                .clone()
                        })
                        .collect::<Vec<claim_tech_desc::Model>>();
                    let found_tiers = learned
                        .iter()
                        .filter(|id| tier_upgrades_ids.contains(&(**id)))
                        .cloned()
                        .collect::<Vec<i32>>();

                    if !found_tiers.is_empty() {
                        claim_description.tier = tier_upgrades
                            .iter()
                            .find(|desc| desc.id == (found_tiers[found_tiers.len() - 1]))
                            .map(|desc| desc.tier);
                    } else {
                        claim_description.tier = Some(1);
                    }
                }
                None => {
                    claim_description.running_upgrade = None;
                    claim_description.upgrades = vec![];
                    claim_description.tier = Some(1);
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
