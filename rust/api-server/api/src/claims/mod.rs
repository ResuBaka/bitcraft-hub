pub(crate) mod claim_local_state;
pub(crate) mod claim_member_state;
pub(crate) mod claim_state;
use crate::inventory::resolve_contents;
use crate::leaderboard::{EXCLUDED_USERS_FROM_LEADERBOARD, LeaderboardSkill, experience_to_level};
use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::inventory::{ExpendedRefrence, ItemExpended};
use entity::shared::location::Location;
use entity::{building_state, cargo_desc, claim_tech_desc, inventory, item_desc, player_state};
use log::error;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use tokio::task::JoinHandle;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

// impl From<claim_description_state::Model> for ClaimDescriptionState {
//     fn from(mut claim_description: claim_description_state::Model) -> Self {
//         let mut members: Vec<ClaimDescriptionStateMember> = Vec::new();
//
//         for member in &mut claim_description.members {
//             members.push(ClaimDescriptionStateMember {
//                 entity_id: member.player_entity_id,
//                 user_name: member.user_name.clone(),
//                 inventory_permission: member.inventory_permission,
//                 build_permission: member.build_permission,
//                 officer_permission: member.officer_permission,
//                 co_owner_permission: member.co_owner_permission,
//                 online_state: OnlineState::Offline,
//                 skills_ranks: Some(BTreeMap::new()),
//             });
//         }
//
//         ClaimDescriptionState {
//             entity_id: claim_description.entity_id,
//             owner_player_entity_id: claim_description.owner_player_entity_id,
//             owner_building_entity_id: claim_description.owner_building_entity_id,
//             name: claim_description.name,
//             supplies: claim_description.supplies,
//             building_maintenance: claim_description.building_maintenance,
//             members,
//             num_tiles: claim_description.num_tiles,
//             extensions: claim_description.extensions,
//             neutral: claim_description.neutral,
//             location: claim_description.location,
//             treasury: claim_description.treasury,
//             xp_gained_since_last_coin_minting: claim_description.xp_gained_since_last_coin_minting,
//             running_upgrade: None,
//             running_upgrade_started: None,
//             tier: None,
//             upgrades: vec![],
//         }
//     }
// }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescriptionStateMemberTmp {
    pub entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ClaimDescriptionStateMember {
    pub entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
    pub online_state: OnlineState,
    pub skills_ranks: Option<BTreeMap<String, LeaderboardSkill>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    let items = state
        .item_desc
        .iter()
        .map(|item| (item.id, item.to_owned()))
        .collect::<HashMap<i32, item_desc::Model>>();

    let cargos = state
        .cargo_desc
        .iter()
        .map(|cargo| (cargo.id, cargo.to_owned()))
        .collect::<HashMap<i32, cargo_desc::Model>>();

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
    let job_items = items.clone();
    let job_cargos = cargos.clone();
    jobs.push(tokio::spawn(async move {
        let offline_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_offline_ids).await?;
        let mut merged_offline_players_inventories =
            get_merged_inventories(offline_players_inventories, &job_items, &job_cargos);
        merged_offline_players_inventories.sort_by(inventory_sort_by);

        Ok((
            "players_offline".to_string(),
            merged_offline_players_inventories,
        ))
    }));

    let conn = state.conn.clone();
    let job_items = items.clone();
    let job_cargos = cargos.clone();
    jobs.push(tokio::spawn(async move {
        let claim_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, building_inventories_ids).await?;
        let mut merged_claim_inventories =
            get_merged_inventories(claim_inventories, &job_items, &job_cargos);
        merged_claim_inventories.sort_by(inventory_sort_by);

        Ok(("buildings".to_string(), merged_claim_inventories))
    }));

    let conn = state.conn.clone();
    let job_items = items.clone();
    let job_cargos = cargos.clone();
    jobs.push(tokio::spawn(async move {
        let online_players_inventories =
            QueryCore::get_inventorys_by_owner_entity_ids(&conn, player_online_ids).await?;
        let mut merged_online_players_inventories =
            get_merged_inventories(online_players_inventories, &job_items, &job_cargos);
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

// pub(crate) async fn find_claim_descriptions(
//     state: State<std::sync::Arc<AppState>>,
//     Path(id): Path<u64>,
// ) -> Result<axum_codec::Codec<ClaimDescriptionState>, (StatusCode, &'static str)> {
//     let claim = QueryCore::find_claim_state(&state.conn, id as i64)
//         .await
//         .expect("Cannot find posts in page");
//
//     if claim.is_none() {
//         return Err((StatusCode::NOT_FOUND, "ClaimDescription not found"));
//     }
//
//     let posts: ClaimDescriptionState = claim.unwrap().into();
//
//     Ok(axum_codec::Codec(posts))
// }

// #[allow(dead_code)]
// pub(crate) async fn load_claim_description_state_from_file(
//     storage_path: &std::path::Path,
// ) -> anyhow::Result<Vec<claim_description_state::Model>> {
//     let item_file = File::open(storage_path.join("State/ClaimDescriptionState.json"))?;
//     let claim_descriptions: Value = serde_json::from_reader(&item_file)?;
//     let claim_descriptions: Vec<claim_description_state::Model> = serde_json::from_value(
//         claim_descriptions
//             .get(0)
//             .unwrap()
//             .get("rows")
//             .unwrap()
//             .clone(),
//     )?;
//
//     Ok(claim_descriptions)
// }
//
// async fn delete_claim_description_state(
//     conn: &DatabaseConnection,
//     known_claim_description_ids: HashSet<i64>,
// ) -> anyhow::Result<()> {
//     info!(
//         "claim_tech_desc's ({}) to delete: {:?}",
//         known_claim_description_ids.len(),
//         known_claim_description_ids,
//     );
//     claim_description_state::Entity::delete_many()
//         .filter(
//             claim_description_state::Column::EntityId.is_in(known_claim_description_ids.clone()),
//         )
//         .exec(conn)
//         .await?;
//
//     player_to_claim::Entity::delete_many()
//         .filter(player_to_claim::Column::ClaimId.is_in(known_claim_description_ids))
//         .exec(conn)
//         .await?;
//     Ok(())
// }
//
// async fn delete_player_to_claim(
//     conn: &DatabaseConnection,
//     known_claim_description_ids: HashSet<(i64, i64)>,
// ) -> anyhow::Result<()> {
//     player_to_claim::Entity::delete_many()
//         .filter(
//             player_to_claim::Column::ClaimId
//                 .is_in(known_claim_description_ids.iter().map(|value| value.0)),
//         )
//         .filter(
//             player_to_claim::Column::PlayerId
//                 .is_in(known_claim_description_ids.iter().map(|value| value.1)),
//         )
//         .exec(conn)
//         .await?;
//     Ok(())
// }
//
// async fn db_insert_player_to_claim(
//     conn: &DatabaseConnection,
//     buffer_before_insert: &mut Vec<player_to_claim::Model>,
//     on_conflict: &OnConflict,
// ) -> anyhow::Result<()> {
//     let claim_description_from_db = player_to_claim::Entity::find()
//         .filter(
//             player_to_claim::Column::ClaimId.is_in(
//                 buffer_before_insert
//                     .iter()
//                     .map(|claim_description| claim_description.claim_id)
//                     .collect::<Vec<i64>>(),
//             ),
//         )
//         .filter(
//             player_to_claim::Column::PlayerId.is_in(
//                 buffer_before_insert
//                     .iter()
//                     .map(|claim_description| claim_description.player_id)
//                     .collect::<Vec<i64>>(),
//             ),
//         )
//         .all(conn)
//         .await?;
//
//     let claim_description_from_db_map = claim_description_from_db
//         .into_iter()
//         .map(|claim_description| {
//             (
//                 (claim_description.claim_id, claim_description.player_id),
//                 claim_description,
//             )
//         })
//         .collect::<HashMap<(i64, i64), player_to_claim::Model>>();
//
//     let things_to_insert = buffer_before_insert
//         .iter()
//         .filter(|claim_description| {
//             match claim_description_from_db_map
//                 .get(&(claim_description.claim_id, claim_description.player_id))
//             {
//                 Some(claim_description_from_db) => claim_description_from_db != *claim_description,
//                 None => true,
//             }
//         })
//         .map(|claim_description| claim_description.clone().into_active_model())
//         .collect::<Vec<player_to_claim::ActiveModel>>();
//
//     if things_to_insert.is_empty() {
//         debug!("Nothing to insert");
//         buffer_before_insert.clear();
//         return Ok(());
//     } else {
//         debug!("Inserting {} claim_tech_desc", things_to_insert.len());
//     }
//
//     let _ = player_to_claim::Entity::insert_many(things_to_insert)
//         .on_conflict(on_conflict.clone())
//         .exec(conn)
//         .await?;
//
//     buffer_before_insert.clear();
//     Ok(())
// }
//
// async fn db_insert_claim_description_state(
//     conn: &DatabaseConnection,
//     buffer_before_insert: &mut Vec<Model>,
//     on_conflict: &OnConflict,
// ) -> anyhow::Result<()> {
//     let claim_description_from_db = claim_description_state::Entity::find()
//         .filter(
//             claim_description_state::Column::EntityId.is_in(
//                 buffer_before_insert
//                     .iter()
//                     .map(|claim_description| claim_description.entity_id)
//                     .collect::<Vec<i64>>(),
//             ),
//         )
//         .all(conn)
//         .await?;
//
//     let claim_description_from_db_map = claim_description_from_db
//         .into_iter()
//         .map(|claim_description| (claim_description.entity_id, claim_description))
//         .collect::<HashMap<i64, claim_description_state::Model>>();
//
//     let things_to_insert = buffer_before_insert
//         .iter()
//         .filter(|claim_description| {
//             match claim_description_from_db_map.get(&claim_description.entity_id) {
//                 Some(claim_description_from_db) => claim_description_from_db != *claim_description,
//                 None => true,
//             }
//         })
//         .map(|claim_description| claim_description.clone().into_active_model())
//         .collect::<Vec<claim_description_state::ActiveModel>>();
//
//     if things_to_insert.is_empty() {
//         debug!("Nothing to insert");
//         buffer_before_insert.clear();
//         return Ok(());
//     } else {
//         debug!("Inserting {} claim_tech_desc", things_to_insert.len());
//     }
//
//     let _ = claim_description_state::Entity::insert_many(things_to_insert)
//         .on_conflict(on_conflict.clone())
//         .exec(conn)
//         .await?;
//
//     buffer_before_insert.clear();
//     Ok(())
// }
//
// async fn known_claim_description_state_ids(
//     conn: &DatabaseConnection,
// ) -> anyhow::Result<HashSet<i64>> {
//     let known_claim_description_ids: Vec<i64> = claim_description_state::Entity::find()
//         .select_only()
//         .column(claim_description_state::Column::EntityId)
//         .into_tuple()
//         .all(conn)
//         .await?;
//
//     let known_claim_description_ids = known_claim_description_ids
//         .into_iter()
//         .collect::<HashSet<i64>>();
//     Ok(known_claim_description_ids)
// }
//
// async fn known_player_to_claim_ids(
//     conn: &DatabaseConnection,
// ) -> anyhow::Result<HashSet<(i64, i64)>> {
//     let known_player_to_claim_ids = player_to_claim::Entity::find()
//         .select_only()
//         .column(player_to_claim::Column::ClaimId)
//         .column(player_to_claim::Column::PlayerId)
//         .into_tuple()
//         .all(conn)
//         .await?;
//     let known_player_to_claim_ids = known_player_to_claim_ids
//         .into_iter()
//         .collect::<HashSet<(i64, i64)>>();
//     Ok(known_player_to_claim_ids)
// }
//
// fn get_claim_description_state_on_conflict() -> OnConflict {
//     sea_query::OnConflict::column(claim_description_state::Column::EntityId)
//         .update_columns([
//             claim_description_state::Column::OwnerPlayerEntityId,
//             claim_description_state::Column::OwnerBuildingEntityId,
//             claim_description_state::Column::Name,
//             claim_description_state::Column::Supplies,
//             claim_description_state::Column::BuildingMaintenance,
//             claim_description_state::Column::Members,
//             claim_description_state::Column::NumTiles,
//             claim_description_state::Column::NumTileNeighbors,
//             claim_description_state::Column::Extensions,
//             claim_description_state::Column::Neutral,
//             claim_description_state::Column::Location,
//             claim_description_state::Column::Treasury,
//             claim_description_state::Column::XpGainedSinceLastCoinMinting,
//             claim_description_state::Column::SuppliesPurchaseThreshold,
//             claim_description_state::Column::SuppliesPurchasePrice,
//             claim_description_state::Column::BuildingDescriptionId,
//         ])
//         .to_owned()
// }
// fn get_player_to_claim_on_conflict() -> OnConflict {
//     sea_query::OnConflict::columns(vec![
//         player_to_claim::Column::PlayerId,
//         player_to_claim::Column::ClaimId,
//     ])
//     .update_columns([
//         player_to_claim::Column::PlayerId,
//         player_to_claim::Column::ClaimId,
//         player_to_claim::Column::InventoryPermission,
//         player_to_claim::Column::BuildPermission,
//         player_to_claim::Column::OfficerPermission,
//         player_to_claim::Column::CoOwnerPermission,
//     ])
//     .to_owned()
// }

pub(crate) fn get_merged_inventories(
    inventorys: Vec<inventory::Model>,
    items: &HashMap<i32, item_desc::Model>,
    cargos: &HashMap<i32, cargo_desc::Model>,
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
//
// pub(crate) async fn handle_initial_subscription(
//     app_state: &Arc<AppState>,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let on_conflict = get_claim_description_state_on_conflict();
//     let on_conflict_player_to_claim = get_player_to_claim_on_conflict();
//
//     let chunk_size = 5000;
//     let mut buffer_before_insert: Vec<claim_description_state::Model> = vec![];
//     let mut buffer_player_to_claim_before_insert: Vec<player_to_claim::Model> = vec![];
//
//     let mut known_inventory_ids = known_claim_description_state_ids(&app_state.conn).await?;
//     let mut known_player_to_claim_ids = known_player_to_claim_ids(&app_state.conn).await?;
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
//                 Ok(building_state) => {
//                     if known_inventory_ids.contains(&building_state.entity_id) {
//                         known_inventory_ids.remove(&building_state.entity_id);
//                     }
//                     buffer_before_insert.push(building_state.clone());
//                     app_state
//                         .claim_description_state
//                         .insert(building_state.entity_id as u64, building_state.clone());
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_claim_description_state(
//                             &app_state.conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                         )
//                         .await?;
//                     }
//                     building_state.members.iter().for_each(
//                         |member: &claim_description_state::Member| {
//                             if known_player_to_claim_ids
//                                 .contains(&(building_state.entity_id, member.player_entity_id))
//                             {
//                                 known_player_to_claim_ids
//                                     .remove(&(building_state.entity_id, member.player_entity_id));
//                             }
//                             buffer_player_to_claim_before_insert.push(player_to_claim::Model {
//                                 player_id: member.player_entity_id,
//                                 claim_id: building_state.entity_id,
//                                 inventory_permission: member.inventory_permission,
//                                 build_permission: member.build_permission,
//                                 officer_permission: member.officer_permission,
//                                 co_owner_permission: member.co_owner_permission,
//                             });
//                         },
//                     );
//                     if buffer_player_to_claim_before_insert.len() == chunk_size {
//                         db_insert_player_to_claim(
//                             &app_state.conn,
//                             &mut buffer_player_to_claim_before_insert,
//                             &on_conflict_player_to_claim,
//                         )
//                         .await?;
//                     }
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_claim_description_state(
//                             &app_state.conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                         )
//                         .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!("InitialSubscription Insert ClaimDescriptionState Error: {error}");
//                 }
//             }
//         }
//     }
//     if !buffer_player_to_claim_before_insert.is_empty() {
//         db_insert_player_to_claim(
//             &app_state.conn,
//             &mut buffer_player_to_claim_before_insert,
//             &on_conflict_player_to_claim,
//         )
//         .await?;
//     }
//
//     if !buffer_before_insert.is_empty() {
//         for buffer_chnk in buffer_before_insert.chunks(5000) {
//             db_insert_claim_description_state(
//                 &app_state.conn,
//                 &mut buffer_chnk.to_vec(),
//                 &on_conflict,
//             )
//             .await?;
//         }
//     }
//
//     if !known_inventory_ids.is_empty() {
//         delete_claim_description_state(&app_state.conn, known_inventory_ids).await?;
//     }
//
//     if !known_player_to_claim_ids.is_empty() {
//         for known_player_to_claim_id in known_player_to_claim_ids.iter() {
//             app_state
//                 .claim_description_state
//                 .remove(&(known_player_to_claim_id.0 as u64));
//         }
//
//         delete_player_to_claim(&app_state.conn, known_player_to_claim_ids).await?;
//     }
//
//     Ok(())
// }
//
// pub(crate) async fn handle_transaction_update(
//     app_state: &Arc<AppState>,
//     tables: &[TableWithOriginalEventTransactionUpdate],
//     sender: UnboundedSender<WebSocketMessages>,
// ) -> anyhow::Result<()> {
//     let on_conflict = get_claim_description_state_on_conflict();
//     let on_conflict_player_to_claim = get_player_to_claim_on_conflict();
//
//     let mut buffer_before_insert = HashMap::new();
//     let mut potential_deletes = HashSet::new();
//     let mut buffer_before_player_to_claim_insert: Vec<player_to_claim::Model> = vec![];
//     let mut potential_player_to_claim_deletes: HashSet<(i64, i64)> = HashSet::new();
//
//     for p1 in tables.iter() {
//         let event_type = if !p1.inserts.is_empty() && !p1.deletes.is_empty() {
//             "update"
//         } else if !p1.inserts.is_empty() && p1.deletes.is_empty() {
//             "insert"
//         } else if !p1.deletes.is_empty() && p1.inserts.is_empty() {
//             "delete"
//         } else {
//             "unknown"
//         };
//
//         if event_type == "unknown" {
//             error!("Unknown event type {:?}", p1);
//             continue;
//         }
//
//         if event_type == "delete" {
//             for row in p1.deletes.iter() {
//                 match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
//                     Ok(claim_description_state) => {
//                         potential_deletes.insert(claim_description_state.entity_id);
//                     }
//                     Err(error) => {
//                         error!("Event: {event_type} Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         } else if event_type == "update" {
//             let mut delete_parsed = HashMap::new();
//             for row in p1.deletes.iter() {
//                 let parsed = serde_json::from_str::<claim_description_state::Model>(row.as_ref());
//
//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse delete claim_description_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row
//                     );
//                 } else {
//                     let parsed = parsed.unwrap();
//                     delete_parsed.insert(parsed.entity_id, parsed.clone());
//                     potential_deletes.remove(&parsed.entity_id);
//                 }
//             }
//
//             for row in p1.inserts.iter().enumerate() {
//                 let parsed = serde_json::from_str::<claim_description_state::Model>(row.1.as_ref());
//
//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse insert claim_description_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row.1
//                     );
//                     continue;
//                 }
//
//                 let parsed = parsed.unwrap();
//                 let id = parsed.entity_id;
//
//                 match (parsed, delete_parsed.get(&id)) {
//                     (new_claim_description_state, Some(old_claim_description_state)) => {
//                         old_claim_description_state
//                             .members
//                             .iter()
//                             .for_each(|member| {
//                                 if !new_claim_description_state.members.contains(member) {
//                                     potential_player_to_claim_deletes.insert((
//                                         new_claim_description_state.entity_id,
//                                         member.player_entity_id,
//                                     ));
//                                 }
//                             });
//                         new_claim_description_state
//                             .members
//                             .iter()
//                             .for_each(|member| {
//                                 if !old_claim_description_state.members.contains(member) {
//                                     buffer_before_player_to_claim_insert.push(
//                                         player_to_claim::Model {
//                                             player_id: member.player_entity_id,
//                                             claim_id: new_claim_description_state.entity_id,
//                                             inventory_permission: member.inventory_permission,
//                                             build_permission: member.build_permission,
//                                             officer_permission: member.officer_permission,
//                                             co_owner_permission: member.co_owner_permission,
//                                         },
//                                     );
//                                 }
//                             });
//
//                         app_state.claim_description_state.insert(
//                             new_claim_description_state.entity_id as u64,
//                             new_claim_description_state.clone(),
//                         );
//                         buffer_before_insert.insert(
//                             new_claim_description_state.entity_id,
//                             new_claim_description_state.clone(),
//                         );
//                         potential_deletes.remove(&new_claim_description_state.entity_id);
//
//                         if new_claim_description_state.xp_gained_since_last_coin_minting
//                             != old_claim_description_state.xp_gained_since_last_coin_minting
//                         {
//                             let increase = if new_claim_description_state
//                                 .xp_gained_since_last_coin_minting
//                                 > old_claim_description_state.xp_gained_since_last_coin_minting
//                             {
//                                 new_claim_description_state.xp_gained_since_last_coin_minting
//                                     - old_claim_description_state.xp_gained_since_last_coin_minting
//                             } else {
//                                 new_claim_description_state.xp_gained_since_last_coin_minting
//                                     + (1000
//                                         - old_claim_description_state
//                                             .xp_gained_since_last_coin_minting)
//                             };
//
//                             if increase < 0 {
//                                 error!(
//                                     "Increase is negative: {increase}, new: {:?}, old: {:?} claim_name: {:?} original_event: {:?}",
//                                     new_claim_description_state.xp_gained_since_last_coin_minting,
//                                     old_claim_description_state.xp_gained_since_last_coin_minting,
//                                     new_claim_description_state.name,
//                                     p1.original_event.reducer_call,
//                                 );
//                             } else if increase > 200 {
//                                 error!(
//                                     "Increase is greater than 200: {increase}, new: {:?}, old: {:?} claim_name: {:?} original_event: {:?}",
//                                     new_claim_description_state.xp_gained_since_last_coin_minting,
//                                     old_claim_description_state.xp_gained_since_last_coin_minting,
//                                     new_claim_description_state.name,
//                                     p1.original_event.reducer_call,
//                                 );
//                             } else {
//                                 metrics::counter!(
//                                     "claim_experience_count",
//                                     &[
//                                         ("claim_name", new_claim_description_state.name.clone()),
//                                         (
//                                             "claim_id",
//                                             new_claim_description_state.entity_id.to_string()
//                                         )
//                                     ]
//                                 )
//                                 .increment(increase as u64);
//                             }
//
//                             sender
//                                 .send(WebSocketMessages::ClaimDescriptionState(
//                                     new_claim_description_state,
//                                 ))
//                                 .unwrap();
//                         }
//                     }
//                     (_new_claim_description_state, None) => {
//                         error!("Could not find delete state new experience state",);
//                     }
//                 }
//             }
//         } else if event_type == "insert" {
//             for row in p1.inserts.iter() {
//                 match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
//                     Ok(claim_description_state) => {
//                         claim_description_state.members.iter().for_each(
//                             |member: &claim_description_state::Member| {
//                                 buffer_before_player_to_claim_insert.push(player_to_claim::Model {
//                                     player_id: member.player_entity_id,
//                                     claim_id: claim_description_state.entity_id,
//                                     inventory_permission: member.inventory_permission,
//                                     build_permission: member.build_permission,
//                                     officer_permission: member.officer_permission,
//                                     co_owner_permission: member.co_owner_permission,
//                                 });
//                             },
//                         );
//
//                         app_state.claim_description_state.insert(
//                             claim_description_state.entity_id as u64,
//                             claim_description_state.clone(),
//                         );
//                         buffer_before_insert.insert(
//                             claim_description_state.entity_id,
//                             claim_description_state.clone(),
//                         );
//
//                         sender
//                             .send(WebSocketMessages::ClaimDescriptionState(
//                                 claim_description_state.clone(),
//                             ))
//                             .unwrap();
//                         metrics::counter!(
//                             "claim_experience_count",
//                             &[
//                                 ("claim_name", claim_description_state.name.clone()),
//                                 ("claim_id", claim_description_state.entity_id.to_string())
//                             ]
//                         )
//                         .increment(
//                             claim_description_state.xp_gained_since_last_coin_minting as u64,
//                         );
//                     }
//                     Err(error) => {
//                         error!("Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         } else {
//             error!("Unknown event type {:?}", p1);
//             continue;
//         }
//     }
//     if !buffer_before_player_to_claim_insert.is_empty() {
//         db_insert_player_to_claim(
//             &app_state.conn,
//             &mut buffer_before_player_to_claim_insert,
//             &on_conflict_player_to_claim,
//         )
//         .await?;
//     }
//     if !buffer_before_insert.is_empty() {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<claim_description_state::Model>>();
//         db_insert_claim_description_state(
//             &app_state.conn,
//             &mut buffer_before_insert_vec,
//             &on_conflict,
//         )
//         .await?;
//         buffer_before_insert.clear();
//     }
//     if !potential_player_to_claim_deletes.is_empty() {
//         delete_player_to_claim(&app_state.conn, potential_player_to_claim_deletes).await?;
//     }
//
//     if !potential_deletes.is_empty() {
//         for potential_delete in potential_deletes.iter() {
//             app_state
//                 .claim_description_state
//                 .remove(&(*potential_delete as u64));
//         }
//
//         delete_claim_description_state(&app_state.conn, potential_deletes).await?;
//     }
//
//     Ok(())
// }
