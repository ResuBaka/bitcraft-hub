use crate::inventory::resolve_contents;
use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::inventory::ExpendedRefrence;
use entity::{
    cargo_description, claim_description, claim_tech_desc, inventory, item, player_state,
};
use log::{debug, error, info};
use sea_orm::{sea_query, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::{sea_orm::DatabaseConnection, Query as QueryCore};
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/claims", get(list_claims))
        .route("/api/bitcraft/claims", get(list_claims))
        .route("/api/bitcraft/claims/:id", get(get_claim))
        .route("/claims/:id", get(find_claim_descriptions))
}

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
        let claim_tech_state = claim_tech_states
            .iter()
            .find(|state| state.entity_id == claim.entity_id);
        let mut claim: ClaimDescriptionState = claim.into();

        match claim_tech_state {
            Some(claim_tech_state) => {
                claim.running_upgrade = match tier_upgrades
                    .iter()
                    .find(|desc| desc.id == (claim_tech_state.researching as i64))
                    .map(|desc| desc.tier)
                {
                    Some(_tier) => Some(true),
                    None => Some(false),
                };
                let learned: Vec<i32> =
                    serde_json::from_value(claim_tech_state.learned.clone()).unwrap();
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
                let found_tiers = learned
                    .iter()
                    .filter(|id| tier_upgrades_ids.contains(&(**id as i64)))
                    .map(|id| id.clone())
                    .collect::<Vec<i32>>();

                if found_tiers.len() > 0 {
                    claim.tier = tier_upgrades
                        .iter()
                        .find(|desc| desc.id == (found_tiers[found_tiers.len() - 1] as i64))
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
                        .map(|desc| desc.tier)
                    {
                        Some(_tier) => Some(true),
                        None => Some(false),
                    };
                    let learned: Vec<i32> =
                        serde_json::from_value(claim_tech_state.learned.clone()).unwrap();
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
                    let found_tiers = learned
                        .iter()
                        .filter(|id| tier_upgrades_ids.contains(&(**id as i64)))
                        .map(|id| id.clone())
                        .collect::<Vec<i32>>();

                    if found_tiers.len() > 0 {
                        claim_description.tier = tier_upgrades
                            .iter()
                            .find(|desc| desc.id == (found_tiers[found_tiers.len() - 1] as i64))
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

pub(crate) async fn load_claim_description_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<claim_description::Model>> {
    let item_file = File::open(storage_path.join("State/ClaimDescriptionState.json"))?;
    let claim_descriptions: Value = serde_json::from_reader(&item_file)?;
    let claim_descriptions: Vec<claim_description::Model> = serde_json::from_value(
        claim_descriptions
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )?;

    Ok(claim_descriptions)
}

pub(crate) async fn load_claim_description_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM ClaimDescriptionState")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".into());
        }
    };

    Ok(json)
}

pub(crate) async fn load_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_claim_description_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_claim_description_state(&conn, claim_descriptions, None).await?;

    Ok(())
}

pub(crate) async fn import_claim_description_state(
    conn: &DatabaseConnection,
    claim_descriptions: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<claim_description::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(claim_descriptions.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(claim_description::Column::EntityId)
        .update_columns([
            claim_description::Column::OwnerPlayerEntityId,
            claim_description::Column::OwnerBuildingEntityId,
            claim_description::Column::Name,
            claim_description::Column::Supplies,
            claim_description::Column::BuildingMaintenance,
            claim_description::Column::Members,
            claim_description::Column::Tiles,
            claim_description::Column::Extensions,
            claim_description::Column::Neutral,
            claim_description::Column::Location,
            claim_description::Column::Treasury,
        ])
        .to_owned();

    let mut claim_description_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<claim_description::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let claim_description_from_db = claim_description::Entity::find()
                .filter(
                    claim_description::Column::EntityId.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|claim_description| claim_description.entity_id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if claim_description_from_db.len() != buffer_before_insert.len() {
                claim_description_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|claim_description| {
                            !claim_description_from_db
                                .iter()
                                .any(|claim_description_from_db| {
                                    claim_description_from_db.entity_id
                                        == claim_description.entity_id
                                })
                        })
                        .map(|claim_description| claim_description.entity_id),
                );
            }

            let claim_description_from_db_map = claim_description_from_db
                .into_iter()
                .map(|claim_description| (claim_description.entity_id, claim_description))
                .collect::<HashMap<i64, claim_description::Model>>();

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
                .collect::<Vec<claim_description::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            }

            for claim_description in &things_to_insert {
                let claim_description_in = claim_description_to_delete
                    .iter()
                    .position(|id| id == claim_description.entity_id.as_ref());
                if claim_description_in.is_some() {
                    claim_description_to_delete.remove(claim_description_in.unwrap());
                }
            }

            let _ = claim_description::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let claim_description_from_db = claim_description::Entity::find()
            .filter(
                claim_description::Column::EntityId.is_in(
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
            .collect::<HashMap<i64, claim_description::Model>>();

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
            .collect::<Vec<claim_description::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            claim_description::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("claim_tech_desc last batch imported");
    }
    info!(
        "Importing claim_tech_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if claim_description_to_delete.len() > 0 {
        info!(
            "claim_tech_desc's to delete: {:?}",
            claim_description_to_delete
        );
        claim_description::Entity::delete_many()
            .filter(claim_description::Column::EntityId.is_in(claim_description_to_delete))
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
            for (_, content) in pocket.contents.iter() {
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

    hashmap.into_iter().map(|(_, value)| value).collect()
}
