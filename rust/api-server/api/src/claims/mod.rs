use crate::inventory::{resolve_contents, resolve_pocket};
use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use entity::inventory::{ExpendedRefrence, ItemSlotResolved};
use entity::{
    cargo_description, claim_description, claim_tech_desc, inventory, item, player_state,
};
use sea_orm::{EntityTrait, IntoActiveModel, PaginatorTrait};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use service::{sea_orm::DatabaseConnection, Query as QueryCore};
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescriptionState {
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub supplies: f32,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionStateMember>,
    pub tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: sea_orm::prelude::Json,
    pub treasury: i32,
    pub running_upgrade: Option<bool>,
    pub tier: Option<i32>,
    pub upgrades: Vec<claim_tech_desc::Model>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescriptionStateWithInventoryAndPlayTime {
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub supplies: f32,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionStateMember>,
    pub tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: sea_orm::prelude::Json,
    pub treasury: i32,
    pub running_upgrade: Option<bool>,
    pub tier: Option<i32>,
    pub upgrades: Vec<claim_tech_desc::Model>,
    pub inventorys: HashMap<String, Vec<entity::inventory::ExpendedRefrence>>,
    pub time_played: u64,
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct ClaimResponse {
    pub claims: Vec<ClaimDescriptionState>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

impl From<claim_description::Model> for ClaimDescriptionState {
    fn from(claim_description: claim_description::Model) -> Self {
        ClaimDescriptionState {
            entity_id: claim_description.entity_id,
            owner_player_entity_id: claim_description.owner_player_entity_id,
            owner_building_entity_id: claim_description.owner_building_entity_id,
            name: claim_description.name,
            supplies: claim_description.supplies,
            building_maintenance: claim_description.building_maintenance,
            members: serde_json::from_value(claim_description.members).unwrap(),
            tiles: claim_description.tiles,
            extensions: claim_description.extensions,
            neutral: claim_description.neutral,
            location: claim_description.location,
            treasury: claim_description.treasury,
            running_upgrade: None,
            tier: None,
            upgrades: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescriptionStateMember {
    pub entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
}

pub(crate) async fn get_claim(
    state: State<AppState>,
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
    let tierUpgrades = claim_tech_descs
        .iter()
        .filter(|desc| desc.description.starts_with("Tier "))
        .map(|desc| desc.clone())
        .collect::<Vec<claim_tech_desc::Model>>();
    let tierUpgradesIds = tierUpgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i64>>();

    let items = QueryCore::all_items(&state.conn)
        .await
        .expect("Cannot find items");
    let items = items
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<i64, item::Model>>();

    let cargos = QueryCore::all_cargos_desc(&state.conn)
        .await
        .expect("Cannot find cargos");
    let cargos = cargos
        .into_iter()
        .map(|cargo| (cargo.id, cargo))
        .collect::<HashMap<i64, cargo_description::Model>>();

    let claim = {
        let mut claimTechState = claim_tech_states
            .iter()
            .find(|state| state.entity_id == claim.entity_id);
        let mut claim: ClaimDescriptionState = claim.into();

        match claimTechState {
            Some(claimTechState) => {
                claim.running_upgrade = match tierUpgrades
                    .iter()
                    .find(|desc| desc.id == (claimTechState.researching as i64))
                    .map(|desc| desc.tier)
                {
                    Some(tier) => Some(true),
                    None => Some(false),
                };
                let learned: Vec<i32> =
                    serde_json::from_value(claimTechState.learned.clone()).unwrap();
                claim.upgrades = learned
                    .iter()
                    .map(|id| {
                        claim_tech_descs
                            .iter()
                            .find(|desc| desc.id == (*id as i64))
                            .unwrap()
                            .clone()
                    })
                    .collect::<Vec<claim_tech_desc::Model>>();
                let foundTiers = learned
                    .iter()
                    .filter(|id| tierUpgradesIds.contains(&(**id as i64)))
                    .map(|id| id.clone())
                    .collect::<Vec<i32>>();

                if foundTiers.len() > 0 {
                    claim.tier = tierUpgrades
                        .iter()
                        .find(|desc| desc.id == (foundTiers[foundTiers.len() - 1] as i64))
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

    let building_states = QueryCore::find_building_state_by_claim_id(&state.conn, claim.entity_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, ""))?;

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

    let total_played = current_players
        .iter()
        .map(|player| player.time_played as u64)
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
        tiles: claim.tiles,
        extensions: claim.extensions,
        neutral: claim.neutral,
        location: claim.location,
        treasury: claim.treasury,
        running_upgrade: claim.running_upgrade,
        tier: claim.tier,
        upgrades: claim.upgrades,
        inventorys: HashMap::new(),
        time_played: total_played,
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
        merged_offline_players_inventories.sort_by(|a, b| b.quantity.cmp(&a.quantity));

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
        merged_claim_inventories.sort_by(|a, b| b.quantity.cmp(&a.quantity));

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
        merged_online_players_inventories.sort_by(|a, b| b.quantity.cmp(&a.quantity));

        Ok(("players".to_string(), merged_online_players_inventories))
    }));

    let finished_jobs = futures::future::join_all(jobs).await;

    for finished_job in finished_jobs {
        if let Err(err) = finished_job {
            println!("Error: {:?}", err);
            continue;
        }

        let job = finished_job.unwrap();

        if let Ok((key, value)) = job {
            claim.inventorys.insert(key.parse().unwrap(), value);
        }
    }

    Ok(Json(claim))
}

pub(crate) async fn list_claims(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<ClaimResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (claims, num_pages) =
        QueryCore::find_claim_descriptions(&state.conn, page, posts_per_page, search)
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
    let tierUpgrades = claim_tech_descs
        .iter()
        .filter(|desc| desc.description.starts_with("Tier "))
        .map(|desc| desc.clone())
        .collect::<Vec<claim_tech_desc::Model>>();
    let tierUpgradesIds = tierUpgrades
        .iter()
        .map(|desc| desc.id)
        .collect::<Vec<i64>>();

    let claims = claims
        .into_iter()
        .map(|claim_description| {
            let mut claimTechState = claim_tech_states
                .iter()
                .find(|state| state.entity_id == claim_description.entity_id);
            let mut claim_description: ClaimDescriptionState = claim_description.into();

            match claimTechState {
                Some(claimTechState) => {
                    claim_description.running_upgrade = match tierUpgrades
                        .iter()
                        .find(|desc| desc.id == (claimTechState.researching as i64))
                        .map(|desc| desc.tier)
                    {
                        Some(tier) => Some(true),
                        None => Some(false),
                    };
                    let learned: Vec<i32> =
                        serde_json::from_value(claimTechState.learned.clone()).unwrap();
                    claim_description.upgrades = learned
                        .iter()
                        .map(|id| {
                            claim_tech_descs
                                .iter()
                                .find(|desc| desc.id == (*id as i64))
                                .unwrap()
                                .clone()
                        })
                        .collect::<Vec<claim_tech_desc::Model>>();
                    let foundTiers = learned
                        .iter()
                        .filter(|id| tierUpgradesIds.contains(&(**id as i64)))
                        .map(|id| id.clone())
                        .collect::<Vec<i32>>();

                    if foundTiers.len() > 0 {
                        claim_description.tier = tierUpgrades
                            .iter()
                            .find(|desc| desc.id == (foundTiers[foundTiers.len() - 1] as i64))
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
    state: State<AppState>,
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

pub(crate) async fn import_claim_description_state(
    conn: &DatabaseConnection,
    storage_path: &PathBuf
) -> anyhow::Result<()> {
    let item_file =
        File::open(storage_path.join("State/ClaimDescriptionState.json"))
            .unwrap();
    let claim_descriptions: Value = serde_json::from_reader(&item_file).unwrap();
    let claim_descriptions: Vec<claim_description::Model> = serde_json::from_value(
        claim_descriptions
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let count = claim_descriptions.len();
    let db_count = claim_description::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        println!("ClaimDescriptionState already imported");
        return Ok(());
    }

    let claim_descriptions: Vec<claim_description::ActiveModel> = claim_descriptions
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for claim_description in claim_descriptions.chunks(5000) {
        let _ = claim_description::Entity::insert_many(claim_description.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) fn get_merged_inventories(
    inventorys: Vec<inventory::Model>,
    items: &HashMap<i64, item::Model>,
    cargos: &HashMap<i64, cargo_description::Model>,
) -> Vec<entity::inventory::ExpendedRefrence> {
    let mut hashmap: HashMap<i64, entity::inventory::ExpendedRefrence> = HashMap::new();

    for inventory in inventorys {
        for pocket in inventory.pockets {
            for (key, content) in pocket.contents.iter() {
                let resolved = resolve_contents(content, items, cargos);

                if resolved.is_none() {
                    continue;
                }

                let resolved = resolved.unwrap();

                match hashmap.get_mut(&resolved.item_id) {
                    Some(value) => {
                        value.quantity += resolved.quantity;
                    }
                    None => {
                        hashmap.insert(resolved.item_id, resolved);
                    }
                }
            }
        }
    }

    hashmap.into_iter().map(|(key, value)| value).collect()
}
