use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Router;
use axum_codec::Codec;
use entity::{building_desc, building_state};
use reqwest::Client;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use serde::Deserialize;
use serde_json::Value;
use service::Query as QueryCore;
use std::fs::File;
use std::path::PathBuf;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/buildings",
            axum_codec::routing::get(find_building_states).into(),
        )
        .route(
            "/api/bitcraft/buildings",
            axum_codec::routing::get(find_building_states).into(),
        )
        .route(
            "/buildings/:id",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/buildings/:id",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/desc/buildings",
            axum_codec::routing::get(find_building_descriptions).into(),
        )
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct BuildingDescriptionsResponse {
    buildings: Vec<building_desc::ApiResponse>,
    per_page: u64,
    total: u64,
    page: u64,
}

pub(crate) async fn find_building_descriptions(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<Codec<BuildingDescriptionsResponse>, (StatusCode, &'static str)> {
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

    return Ok(Codec(BuildingDescriptionsResponse {
        buildings,
        per_page,
        total: posts.1.number_of_items,
        page,
    }));
}

pub(crate) async fn find_claim_description(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<building_desc::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_desc_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingDesc not found"));
    }

    return Ok(Codec(posts.unwrap()));
}

#[axum_codec::apply(encode, decode)]
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
    state: State<AppState>,
    Query(params): Query<BuildingStatesParams>,
) -> Result<Codec<BuildingStatesResponse>, (StatusCode, &'static str)> {
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

    return Ok(Codec(BuildingStatesResponse {
        buildings: posts.0,
        per_page,
        total: posts.1.number_of_items,
        page,
    }));
}

pub(crate) async fn find_building_state(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<building_state::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_state_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingState not found"));
    }

    return Ok(Codec(posts.unwrap()));
}

pub(crate) async fn import_building_state(
    conn: &DatabaseConnection,
    storage_path: &PathBuf,
) -> anyhow::Result<()> {
    let item_file = File::open(storage_path.join("State/BuildingState.json")).unwrap();
    let building_state: Value = serde_json::from_reader(&item_file).unwrap();
    let building_states: Vec<building_state::Model> =
        serde_json::from_value(building_state.get(0).unwrap().get("rows").unwrap().clone())
            .unwrap();
    let count = building_states.len();
    let db_count = building_state::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let building_states: Vec<building_state::ActiveModel> = building_states
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for building_state in building_states.chunks(5000) {
        let _ = building_state::Entity::insert_many(building_state.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) async fn load_building_desc_from_spacetimedb(
    client: &Client,
    database: &str,
) -> anyhow::Result<Vec<building_desc::Model>> {
    let building_descs: Vec<building_desc::Model> = {
        let response = client
            .post(format!("/database/sql/{database}"))
            .json(&serde_json::json!({}))
            .send()
            .await?
            .json::<Value>()
            .await?;

        serde_json::from_value(response.get(0).unwrap().get("rows").unwrap().clone())?
    };

    Ok(building_descs)
}

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

pub(crate) async fn import_building_desc(
    conn: &DatabaseConnection,
    building_descs: &Vec<building_desc::Model>,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let chunk_size = chunk_size.unwrap_or(2500);
    let count = building_descs.len();
    let db_count = building_desc::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let building_descs: Vec<building_desc::ActiveModel> = building_descs
        .into_iter()
        .map(|x| x.clone().into_active_model())
        .collect();

    for building_desc in building_descs.chunks(chunk_size) {
        let _ = building_desc::Entity::insert_many(building_desc.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
