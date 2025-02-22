use crate::config::Config;
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use crate::{AppRouter, AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::building_state::Model;
use entity::{building_desc, building_state};
use log::{debug, error, info};
use migration::OnConflict;
use reqwest::Client;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
    sea_query,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/buildings", get(find_building_states))
        .route("/api/bitcraft/buildings", get(find_building_states))
        .route("/buildings/{id}", get(find_building_state))
        .route("/api/bitcraft/buildings/{id}", get(find_building_state))
        .route(
            "/api/bitcraft/desc/buildings",
            get(find_building_descriptions),
        )
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BuildingDescriptionsResponse {
    buildings: Vec<building_desc::ApiResponse>,
    per_page: u64,
    total: u64,
    page: u64,
}

pub(crate) async fn find_building_descriptions(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<Params>,
) -> Result<Json<BuildingDescriptionsResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);
    let search = params.search;

    let posts = QueryCore::find_building_descs(&state.conn, page, per_page, search)
        .await
        .expect("Cannot find posts in page");

    let building_ids = posts.0.iter().map(|x| x.id).collect::<Vec<i64>>();
    let building_states = QueryCore::find_building_state_by_desc_ids(&state.conn, building_ids)
        .await
        .expect("Cannot find building states");

    let buildings = posts
        .0
        .into_iter()
        .map(|x| building_desc::ApiResponse {
            id: x.id,
            functions: x.functions,
            name: x.name,
            description: x.description,
            rested_buff_duration: x.rested_buff_duration,
            light_radius: x.light_radius,
            model_asset_name: x.model_asset_name,
            icon_asset_name: x.icon_asset_name,
            unenterable: x.unenterable,
            wilderness: x.wilderness,
            footprint: x.footprint,
            max_health: x.max_health,
            decay: x.decay,
            maintenance: x.maintenance,
            interaction_level: x.interaction_level,
            has_action: x.has_action,
            show_in_compendium: x.show_in_compendium,
            is_ruins: x.is_ruins,
            count: building_states
                .iter()
                .filter(|building_state| building_state.building_description_id as i64 == (x.id))
                .count() as i32,
        })
        .collect::<Vec<building_desc::ApiResponse>>();

    Ok(Json(BuildingDescriptionsResponse {
        buildings,
        per_page,
        total: posts.1.number_of_items,
        page,
    }))
}

pub(crate) async fn find_claim_description(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<building_desc::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_desc_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingDesc not found"));
    }

    Ok(Json(posts.unwrap()))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BuildingStatesResponse {
    buildings: Vec<building_state::Model>,
    per_page: u64,
    total: u64,
    page: u64,
}

#[derive(Deserialize)]
pub(crate) struct BuildingStatesParams {
    page: Option<u64>,
    per_page: Option<u64>,
    claim_entity_id: Option<i64>,
    with_inventory: Option<bool>,
}

pub(crate) async fn find_building_states(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<BuildingStatesParams>,
) -> Result<Json<BuildingStatesResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);
    let search = params.claim_entity_id;
    let with_inventory = params.with_inventory.unwrap_or(false);
    let mut buildings_with_inventory_ids = None;

    if with_inventory {
        let buildings_with_inventory = QueryCore::find_building_descs_with_inventory(&state.conn)
            .await
            .expect("Cannot find posts in page");

        buildings_with_inventory_ids = Some(
            buildings_with_inventory
                .iter()
                .map(|building| building.id)
                .collect(),
        );
    }

    let posts = QueryCore::find_building_states(
        &state.conn,
        page,
        per_page,
        search,
        buildings_with_inventory_ids,
    )
    .await
    .expect("Cannot find posts in page");

    Ok(Json(BuildingStatesResponse {
        buildings: posts.0,
        per_page,
        total: posts.1.number_of_items,
        page,
    }))
}

pub(crate) async fn find_building_state(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<building_state::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_state_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingState not found"));
    }

    Ok(Json(posts.unwrap()))
}

#[allow(dead_code)]
pub(crate) async fn load_building_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<building_state::Model>> {
    let building_states: Vec<building_state::Model> = {
        let item_file = File::open(storage_path.join("State/BuildingState.json"))?;
        let building_state: Value = serde_json::from_reader(&item_file)?;

        serde_json::from_value(building_state.get(0).unwrap().get("rows").unwrap().clone())?
    };

    Ok(building_states)
}

pub(crate) async fn load_building_state_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM building_state")
        .send()
        .await;

    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error:?}");
            return Ok("".into());
        }
    };

    Ok(json)
}

pub(crate) async fn load_state_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let building_descs =
        load_building_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_building_state(&conn, building_descs, None).await?;

    Ok(())
}

pub(crate) async fn import_building_state(
    conn: &DatabaseConnection,
    building_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<building_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(building_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(building_state::Column::EntityId)
        .update_columns([
            building_state::Column::ClaimEntityId,
            building_state::Column::DirectionIndex,
            building_state::Column::BuildingDescriptionId,
            building_state::Column::ConstructedByPlayerEntityId,
            building_state::Column::Nickname,
        ])
        .to_owned();

    let mut known_building_state_ids = get_known_building_state_ids(conn).await?;

    while let Ok(value) = json_stream_reader.deserialize_next::<building_state::Model>() {
        if known_building_state_ids.contains(&value.entity_id) {
            known_building_state_ids.remove(&value.entity_id);
        }

        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            db_insert_building_state(conn, &mut buffer_before_insert, &on_conflict).await?;
        }
    }

    if buffer_before_insert.len() > 0 {
        db_insert_building_state(conn, &mut buffer_before_insert, &on_conflict).await?;
        info!("building_state last batch imported");
    }
    info!(
        "Importing building_state finished in {}s",
        start.elapsed().as_secs()
    );

    if known_building_state_ids.len() > 0 {
        delete_building_state(conn, known_building_state_ids).await?;
    }

    Ok(())
}

async fn delete_building_state(
    conn: &DatabaseConnection,
    known_building_state_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "building_state's ({}) to delete: {:?}",
        known_building_state_ids.len(),
        known_building_state_ids
    );
    building_state::Entity::delete_many()
        .filter(building_state::Column::EntityId.is_in(known_building_state_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn get_known_building_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_building_state_ids: Vec<i64> = building_state::Entity::find()
        .select_only()
        .column(building_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_building_state_ids = known_building_state_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_building_state_ids)
}

async fn db_insert_building_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let building_state_from_db = building_state::Entity::find()
        .filter(
            building_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|building_state| building_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let building_state_from_db_map = building_state_from_db
        .into_iter()
        .map(|building_state| (building_state.entity_id, building_state))
        .collect::<HashMap<i64, building_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|building_state| {
            match building_state_from_db_map.get(&building_state.entity_id) {
                Some(building_state_from_db) => {
                    if building_state_from_db != *building_state {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|building_state| building_state.clone().into_active_model())
        .collect::<Vec<building_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} building_state", things_to_insert.len());
    }

    let _ = building_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();

    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn load_building_desc_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<building_desc::Model>> {
    let building_descs: Vec<building_desc::Model> = {
        let item_file = File::open(storage_path.join("Desc/BuildingDesc.json"))?;
        let building_desc: Value = serde_json::from_reader(&item_file)?;

        serde_json::from_value(building_desc.get(0).unwrap().get("rows").unwrap().clone())?
    };

    Ok(building_descs)
}

#[allow(dead_code)]
async fn delete_building_descs(
    conn: &DatabaseConnection,
    buildings_desc_to_delete: Vec<i64>,
) -> anyhow::Result<()> {
    info!("Buildings_desc's to delete: {:?}", buildings_desc_to_delete);
    building_desc::Entity::delete_many()
        .filter(building_desc::Column::Id.is_in(buildings_desc_to_delete))
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn import_job_building_desc(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_internal_building_state(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_building_state(config.clone(), conn, client);
    }
}

fn import_internal_building_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let building_state = load_state_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_building_state) = building_state {
                    info!("BuildingState imported");
                } else {
                    error!("BuildingState import failed: {:?}", building_state);
                }
            });
    });
}

pub(crate) async fn handle_transaction_update(
    p0: &DatabaseConnection,
    tables: &Vec<TableWithOriginalEventTransactionUpdate>,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(building_state::Column::EntityId)
        .update_columns([
            building_state::Column::ClaimEntityId,
            building_state::Column::DirectionIndex,
            building_state::Column::BuildingDescriptionId,
            building_state::Column::ConstructedByPlayerEntityId,
            building_state::Column::Nickname,
        ])
        .to_owned();

    let mut found_in_inserts = HashSet::new();

    // let mut known_player_username_state_ids = get_known_player_uusername_state_ids(p0).await?;
    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<building_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    let current_building_state =
                        QueryCore::find_building_state_by_id(&p0, building_state.entity_id).await?;

                    if current_building_state.is_some() {
                        let current_building_state = current_building_state.unwrap();
                        if current_building_state != building_state {
                            found_in_inserts.insert(building_state.entity_id);
                            let _ = building_state::Entity::insert(
                                building_state.clone().into_active_model(),
                            )
                            .on_conflict(on_conflict.clone())
                            .exec(p0)
                            .await?;
                        }
                    } else {
                        found_in_inserts.insert(building_state.entity_id);
                        let _ = building_state::Entity::insert(
                            building_state.clone().into_active_model(),
                        )
                        .exec(p0)
                        .await?;
                    }
                }
                Err(_) => {}
            }
        }
    }

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<building_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    if found_in_inserts.contains(&building_state.entity_id) {
                        continue;
                    }

                    let current_building_state =
                        QueryCore::find_building_state_by_id(&p0, building_state.entity_id).await?;

                    if current_building_state.is_some() {
                        let _ = building_state::Entity::delete_many()
                            .filter(building_state::Column::EntityId.eq(building_state.entity_id))
                            .exec(p0)
                            .await?;
                    }
                }
                Err(_) => {}
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(building_state::Column::EntityId)
        .update_columns([
            building_state::Column::ClaimEntityId,
            building_state::Column::DirectionIndex,
            building_state::Column::BuildingDescriptionId,
            building_state::Column::ConstructedByPlayerEntityId,
            building_state::Column::Nickname,
        ])
        .to_owned();

    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<building_state::Model> = vec![];

    let mut known_building_state_ids = get_known_building_state_ids(p0).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<building_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    if known_building_state_ids.contains(&building_state.entity_id) {
                        known_building_state_ids.remove(&building_state.entity_id);
                    }
                    buffer_before_insert.push(building_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                        db_insert_building_state(p0, &mut buffer_before_insert, &on_conflict)
                            .await?;
                    }
                }
                Err(error) => {
                    error!("InitialSubscription Insert BuildingState Error: {error}");
                }
            }
        }
    }

    if buffer_before_insert.len() > 0 {
        for buffer_chnk in buffer_before_insert.chunks(5000) {
            db_insert_building_state(p0, &mut buffer_chnk.to_vec(), &on_conflict).await?;
        }
    }

    if known_building_state_ids.len() > 0 {
        delete_building_state(p0, known_building_state_ids).await?;
    }

    Ok(())
}
