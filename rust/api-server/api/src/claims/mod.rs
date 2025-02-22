use crate::inventory::resolve_contents;
use crate::leaderboard::{EXCLUDED_USERS_FROM_LEADERBOARD, LeaderboardSkill, experience_to_level};
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate, WebSocketMessages};
use crate::{AppRouter, AppState};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::claim_description_state::Model;
use entity::inventory::{ExpendedRefrence, ItemExpended};
use entity::{
    building_state, cargo_desc, claim_description_state, claim_tech_desc, inventory, item_desc,
    player_state,
};
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, sea_query};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::{Query as QueryCore, sea_orm::DatabaseConnection};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/claims", get(list_claims))
        .route("/api/bitcraft/claims", get(list_claims))
        .route("/api/bitcraft/claims/{id}", get(get_claim))
        .route("/claims/{id}", get(find_claim_descriptions))
        .route("/claims/tiles/{id}", get(get_claim_tiles))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescriptionState {
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub supplies: i64,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionStateMember>,
    pub num_tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: sea_orm::prelude::Json,
    pub treasury: i32,
    pub running_upgrade: Option<claim_tech_desc::Model>,
    pub running_upgrade_started: Option<i64>,
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
    pub supplies: i64,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionStateMember>,
    pub num_tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: sea_orm::prelude::Json,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub running_upgrade: Option<claim_tech_desc::Model>,
    pub running_upgrade_started: Option<i64>,
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

impl From<claim_description_state::Model> for ClaimDescriptionState {
    fn from(claim_description: claim_description_state::Model) -> Self {
        let mut tmp_members: Vec<ClaimDescriptionStateMemberTmp> =
            serde_json::from_value(claim_description.members).unwrap();
        let mut members: Vec<ClaimDescriptionStateMember> = Vec::new();

        for member in &mut tmp_members {
            members.push(ClaimDescriptionStateMember {
                entity_id: member.entity_id,
                user_name: member.user_name.clone(),
                inventory_permission: member.inventory_permission,
                build_permission: member.build_permission,
                officer_permission: member.officer_permission,
                co_owner_permission: member.co_owner_permission,
                online_state: OnlineState::Offline,
                skills_ranks: Some(BTreeMap::new()),
            });
        }

        ClaimDescriptionState {
            entity_id: claim_description.entity_id,
            owner_player_entity_id: claim_description.owner_player_entity_id,
            owner_building_entity_id: claim_description.owner_building_entity_id,
            name: claim_description.name,
            supplies: claim_description.supplies,
            building_maintenance: claim_description.building_maintenance,
            members,
            num_tiles: claim_description.num_tiles,
            extensions: claim_description.extensions,
            neutral: claim_description.neutral,
            location: claim_description.location,
            treasury: claim_description.treasury,
            xp_gained_since_last_coin_minting: claim_description.xp_gained_since_last_coin_minting,
            running_upgrade: None,
            running_upgrade_started: None,
            tier: None,
            upgrades: vec![],
        }
    }
}

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
) -> Result<Json<Vec<entity::claim_tile_state::Model>>, (StatusCode, &'static str)> {
    let claim_tiles = state
        .claim_tile_state
        .iter()
        .filter(|a| a.claim_id == id)
        .collect::<Vec<_>>();

    Ok(Json(
        claim_tiles.iter().map(|a| a.value().clone()).collect(),
    ))
}

pub(crate) async fn get_claim(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<ClaimDescriptionStateWithInventoryAndPlayTime>, (StatusCode, &'static str)> {
    let claim = QueryCore::find_claim_description(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if claim.is_none() {
        return Err((StatusCode::NOT_FOUND, "Claim not found"));
    }

    let claim = claim.unwrap();

    let claim_tech_states =
        QueryCore::find_claim_tech_state_by_ids(&state.conn, vec![claim.entity_id.clone()])
            .await
            .expect("Cannot find claim tech states");
    let claim_tech_descs = QueryCore::all_claim_tech_desc(&state.conn)
        .await
        .expect("Cannot find claim tech descs");
    let tier_upgrades = claim_tech_descs
        .iter()
        .filter(|desc| desc.description.starts_with("Tier "))
        .map(|desc| desc.clone())
        .collect::<Vec<claim_tech_desc::Model>>();
    let tier_upgrades_ids = tier_upgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i64>>();

    let items = QueryCore::all_items(&state.conn)
        .await
        .unwrap_or_else(|err| {
            error!("Error loading items: {err}");
            vec![]
        });

    let items = items
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<i64, item_desc::Model>>();

    let cargos = QueryCore::all_cargos_desc(&state.conn)
        .await
        .unwrap_or_else(|err| {
            error!("Error loading cargos: {err}");
            vec![]
        });
    let cargos = cargos
        .into_iter()
        .map(|cargo| (cargo.id, cargo))
        .collect::<HashMap<i64, cargo_desc::Model>>();

    let mut claim = {
        let claim_tech_state = claim_tech_states
            .iter()
            .find(|state| state.entity_id == claim.entity_id);
        let mut claim: ClaimDescriptionState = claim.into();

        match claim_tech_state {
            Some(claim_tech_state) => {
                claim.running_upgrade = match tier_upgrades
                    .iter()
                    .find(|desc| desc.id == (claim_tech_state.researching as i64))
                {
                    Some(tier) => Some(tier.clone()),
                    None => None,
                };
                claim.running_upgrade_started = Some(claim_tech_state.start_timestamp);
                let learned: Vec<i64> = claim_tech_state.learned.clone();
                claim.upgrades = learned
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
                    .map(|id| id.clone())
                    .collect::<Vec<i64>>();

                if found_tiers.len() > 0 {
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

    let skills = service::Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let mut tasks: Vec<
        tokio::task::JoinHandle<
            Result<(String, Vec<LeaderboardSkill>), (StatusCode, &'static str)>,
        >,
    > = vec![];
    let player_ids = claim
        .members
        .iter()
        .map(|claim| claim.entity_id)
        .collect::<Vec<i64>>();

    for skill in skills {
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
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<LeaderboardSkill> = Vec::new();
            let entries = service::Query::get_experience_state_player_ids_by_skill_id(
                &db,
                skill.id,
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
                    level: experience_to_level(entry.experience.clone() as i64),
                    rank: rank as u64,
                });
            }

            Ok((skill.name.clone(), leaderboard))
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

    let mut jobs: Vec<JoinHandle<anyhow::Result<(String, Vec<ExpendedRefrence>)>>> = vec![];

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

    Ok(Json(claim))
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
) -> Result<Json<ClaimResponse>, (StatusCode, &'static str)> {
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
        .map(|desc| desc.clone())
        .collect::<Vec<claim_tech_desc::Model>>();
    let tier_upgrades_ids = tier_upgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i64>>();

    let claims = claims
        .into_iter()
        .map(|claim_description| {
            let claim_tech_state = claim_tech_states
                .iter()
                .find(|state| state.entity_id == claim_description.entity_id);
            let mut claim_description: ClaimDescriptionState = claim_description.into();

            match claim_tech_state {
                Some(claim_tech_state) => {
                    claim_description.running_upgrade = match tier_upgrades
                        .iter()
                        .find(|desc| desc.id == (claim_tech_state.researching as i64))
                    {
                        Some(tier) => Some(tier.clone()),
                        None => None,
                    };
                    let learned: Vec<i64> = claim_tech_state.learned.clone();
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
                        .map(|id| id.clone())
                        .collect::<Vec<i64>>();

                    if found_tiers.len() > 0 {
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

    Ok(Json(ClaimResponse {
        claims,
        per_page: posts_per_page,
        total: num_pages.number_of_items,
        page,
    }))
}

pub(crate) async fn find_claim_descriptions(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<ClaimDescriptionState>, (StatusCode, &'static str)> {
    let claim = QueryCore::find_claim_description_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if claim.is_none() {
        return Err((StatusCode::NOT_FOUND, "ClaimDescription not found"));
    }

    let posts: ClaimDescriptionState = claim.unwrap().into();

    Ok(Json(posts))
}

#[allow(dead_code)]
pub(crate) async fn load_claim_description_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<claim_description_state::Model>> {
    let item_file = File::open(storage_path.join("State/ClaimDescriptionState.json"))?;
    let claim_descriptions: Value = serde_json::from_reader(&item_file)?;
    let claim_descriptions: Vec<claim_description_state::Model> = serde_json::from_value(
        claim_descriptions
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )?;

    Ok(claim_descriptions)
}

async fn delete_claim_description_state(
    conn: &DatabaseConnection,
    known_claim_description_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "claim_tech_desc's ({}) to delete: {:?}",
        known_claim_description_ids.len(),
        known_claim_description_ids,
    );
    claim_description_state::Entity::delete_many()
        .filter(claim_description_state::Column::EntityId.is_in(known_claim_description_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_claim_description_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let claim_description_from_db = claim_description_state::Entity::find()
        .filter(
            claim_description_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|claim_description| claim_description.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let claim_description_from_db_map = claim_description_from_db
        .into_iter()
        .map(|claim_description| (claim_description.entity_id, claim_description))
        .collect::<HashMap<i64, claim_description_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|claim_description| {
            match claim_description_from_db_map.get(&claim_description.entity_id) {
                Some(claim_description_from_db) => {
                    if claim_description_from_db != *claim_description {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|claim_description| claim_description.clone().into_active_model())
        .collect::<Vec<claim_description_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} claim_tech_desc", things_to_insert.len());
    }

    let _ = claim_description_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn known_claim_description_state_ids(
    conn: &DatabaseConnection,
) -> anyhow::Result<HashSet<i64>> {
    let known_claim_description_ids: Vec<i64> = claim_description_state::Entity::find()
        .select_only()
        .column(claim_description_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_claim_description_ids = known_claim_description_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_claim_description_ids)
}

fn get_claim_description_state_on_conflict() -> OnConflict {
    sea_query::OnConflict::column(claim_description_state::Column::EntityId)
        .update_columns([
            claim_description_state::Column::OwnerPlayerEntityId,
            claim_description_state::Column::OwnerBuildingEntityId,
            claim_description_state::Column::Name,
            claim_description_state::Column::Supplies,
            claim_description_state::Column::BuildingMaintenance,
            claim_description_state::Column::Members,
            claim_description_state::Column::NumTiles,
            claim_description_state::Column::NumTileNeighbors,
            claim_description_state::Column::Extensions,
            claim_description_state::Column::Neutral,
            claim_description_state::Column::Location,
            claim_description_state::Column::Treasury,
            claim_description_state::Column::XpGainedSinceLastCoinMinting,
            claim_description_state::Column::SuppliesPurchaseThreshold,
            claim_description_state::Column::SuppliesPurchasePrice,
            claim_description_state::Column::BuildingDescriptionId,
        ])
        .to_owned()
}

pub(crate) fn get_merged_inventories(
    inventorys: Vec<inventory::Model>,
    items: &HashMap<i64, item_desc::Model>,
    cargos: &HashMap<i64, cargo_desc::Model>,
) -> Vec<entity::inventory::ExpendedRefrence> {
    let mut hashmap: HashMap<
        (i64, entity::inventory::ItemType),
        entity::inventory::ExpendedRefrence,
    > = HashMap::new();

    for inventory in inventorys {
        for pocket in inventory.pockets {
            let (_, content) = pocket.contents;
            let resolved = resolve_contents(&content, items, cargos);

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

    hashmap.into_iter().map(|(_, value)| value).collect()
}

pub(crate) async fn handle_initial_subscription(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let on_conflict = get_claim_description_state_on_conflict();

    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<claim_description_state::Model> = vec![];

    let mut known_inventory_ids = known_claim_description_state_ids(p0).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    if known_inventory_ids.contains(&building_state.entity_id) {
                        known_inventory_ids.remove(&building_state.entity_id);
                    }
                    buffer_before_insert.push(building_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                        db_insert_claim_description_state(
                            p0,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!("InitialSubscription Insert ClaimDescriptionState Error: {error}");
                }
            }
        }
    }

    if buffer_before_insert.len() > 0 {
        for buffer_chnk in buffer_before_insert.chunks(5000) {
            db_insert_claim_description_state(p0, &mut buffer_chnk.to_vec(), &on_conflict).await?;
        }
    }

    if known_inventory_ids.len() > 0 {
        delete_claim_description_state(p0, known_inventory_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    p0: &DatabaseConnection,
    tables: &Vec<TableWithOriginalEventTransactionUpdate>,
    sender: UnboundedSender<WebSocketMessages>,
) -> anyhow::Result<()> {
    let on_conflict = get_claim_description_state_on_conflict();

    let mut buffer_before_insert = HashMap::new();
    let mut potential_deletes = HashSet::new();

    for p1 in tables.iter() {
        let event_type = if p1.inserts.len() > 0 && p1.deletes.len() > 0 {
            "update"
        } else if p1.inserts.len() > 0 && p1.deletes.len() == 0 {
            "insert"
        } else if p1.deletes.len() > 0 && p1.inserts.len() == 0 {
            "delete"
        } else {
            "unknown"
        };

        if event_type == "unknown" {
            error!("Unknown event type {:?}", p1);
            continue;
        }

        if event_type == "delete" {
            for row in p1.deletes.iter() {
                match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
                    Ok(claim_description_state) => {
                        potential_deletes.insert(claim_description_state.entity_id);
                    }
                    Err(error) => {
                        error!("Event: {event_type} Error: {error} for row: {:?}", row);
                    }
                }
            }
        } else if event_type == "update" {
            let mut delete_parsed = HashMap::new();
            for row in p1.deletes.iter() {
                let parsed = serde_json::from_str::<claim_description_state::Model>(row.as_ref());

                if parsed.is_err() {
                    error!(
                        "Could not parse delete claim_description_state: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row
                    );
                } else {
                    let parsed = parsed.unwrap();
                    delete_parsed.insert(parsed.entity_id, parsed.clone());
                    potential_deletes.remove(&parsed.entity_id);
                }
            }

            for row in p1.inserts.iter().enumerate() {
                let parsed = serde_json::from_str::<claim_description_state::Model>(row.1.as_ref());

                if parsed.is_err() {
                    error!(
                        "Could not parse insert claim_description_state: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row.1
                    );
                    continue;
                }

                let parsed = parsed.unwrap();
                let id = parsed.entity_id;

                match (parsed, delete_parsed.get(&id)) {
                    (new_claim_description_state, Some(old_claim_description_state)) => {
                        buffer_before_insert.insert(
                            new_claim_description_state.entity_id,
                            new_claim_description_state.clone(),
                        );
                        potential_deletes.remove(&new_claim_description_state.entity_id);

                        if new_claim_description_state.xp_gained_since_last_coin_minting
                            != old_claim_description_state.xp_gained_since_last_coin_minting
                        {
                            let increase = if new_claim_description_state
                                .xp_gained_since_last_coin_minting
                                > old_claim_description_state.xp_gained_since_last_coin_minting
                            {
                                new_claim_description_state.xp_gained_since_last_coin_minting
                                    - old_claim_description_state.xp_gained_since_last_coin_minting
                            } else {
                                new_claim_description_state.xp_gained_since_last_coin_minting
                                    + (1000
                                        - old_claim_description_state
                                            .xp_gained_since_last_coin_minting)
                            };

                            if increase < 0 {
                                error!(
                                    "Increase is negative: {increase}, new: {:?}, old: {:?} claim_name: {:?} original_event: {:?}",
                                    new_claim_description_state.xp_gained_since_last_coin_minting,
                                    old_claim_description_state.xp_gained_since_last_coin_minting,
                                    new_claim_description_state.name,
                                    p1.original_event.reducer_call,
                                );
                            } else if increase > 200 {
                                error!(
                                    "Increase is greater than 200: {increase}, new: {:?}, old: {:?} claim_name: {:?} original_event: {:?}",
                                    new_claim_description_state.xp_gained_since_last_coin_minting,
                                    old_claim_description_state.xp_gained_since_last_coin_minting,
                                    new_claim_description_state.name,
                                    p1.original_event.reducer_call,
                                );
                            } else {
                                metrics::counter!(
                                    "claim_experience_count",
                                    &[
                                        ("claim_name", new_claim_description_state.name.clone()),
                                        (
                                            "claim_id",
                                            new_claim_description_state.entity_id.to_string()
                                        )
                                    ]
                                )
                                .increment(increase as u64);
                            }

                            sender
                                .send(WebSocketMessages::ClaimDescriptionState(
                                    new_claim_description_state,
                                ))
                                .unwrap();
                        }
                    }
                    (_new_claim_description_state, None) => {
                        error!("Could not find delete state new experience state",);
                    }
                }
            }
        } else if event_type == "insert" {
            for row in p1.inserts.iter() {
                match serde_json::from_str::<claim_description_state::Model>(row.as_ref()) {
                    Ok(claim_description_state) => {
                        buffer_before_insert.insert(
                            claim_description_state.entity_id,
                            claim_description_state.clone(),
                        );

                        sender
                            .send(WebSocketMessages::ClaimDescriptionState(
                                claim_description_state.clone(),
                            ))
                            .unwrap();
                        metrics::counter!(
                            "claim_experience_count",
                            &[
                                ("claim_name", claim_description_state.name.clone()),
                                ("claim_id", claim_description_state.entity_id.to_string())
                            ]
                        )
                        .increment(
                            claim_description_state.xp_gained_since_last_coin_minting as u64,
                        );
                    }
                    Err(error) => {
                        error!("Error: {error} for row: {:?}", row);
                    }
                }
            }
        } else {
            error!("Unknown event type {:?}", p1);
            continue;
        }
    }

    if buffer_before_insert.len() > 0 {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<claim_description_state::Model>>();
        db_insert_claim_description_state(p0, &mut buffer_before_insert_vec, &on_conflict).await?;
        buffer_before_insert.clear();
    }

    if potential_deletes.len() > 0 {
        delete_claim_description_state(p0, potential_deletes).await?;
    }

    Ok(())
}
