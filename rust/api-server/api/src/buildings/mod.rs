use crate::{AppRouter, AppState, Params};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::{building_desc, building_state};
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;

pub(crate) fn get_routes() -> AppRouter {
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
            "/buildings/{id}",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/buildings/{id}",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/desc/buildings",
            axum_codec::routing::get(find_building_descriptions).into(),
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
) -> Result<axum_codec::Codec<BuildingDescriptionsResponse>, (StatusCode, &'static str)> {
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
            has_action: x.has_action,
            show_in_compendium: x.show_in_compendium,
            is_ruins: x.is_ruins,
            count: building_states
                .iter()
                .filter(|building_state| building_state.building_description_id as i64 == (x.id))
                .count() as i32,
        })
        .collect::<Vec<building_desc::ApiResponse>>();

    Ok(axum_codec::Codec(BuildingDescriptionsResponse {
        buildings,
        per_page,
        total: posts.1.number_of_items,
        page,
    }))
}

pub(crate) async fn find_claim_description(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<building_desc::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_desc_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingDesc not found"));
    }

    Ok(axum_codec::Codec(posts.unwrap()))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BuildingStateWithName {
    pub entity_id: i64,
    pub claim_entity_id: i64,
    pub direction_index: i32,
    pub building_description_id: i32,
    pub constructed_by_player_entity_id: i64,
    pub building_name: String,
    pub location: Option<entity::location::Model>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BuildingStatesResponse {
    buildings: Vec<BuildingStateWithName>,
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
) -> Result<axum_codec::Codec<BuildingStatesResponse>, (StatusCode, &'static str)> {
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

    Ok(axum_codec::Codec(BuildingStatesResponse {
        buildings: posts
            .0
            .iter()
            .map(|building_state| BuildingStateWithName {
                entity_id: building_state.entity_id,
                claim_entity_id: building_state.claim_entity_id,
                direction_index: building_state.direction_index,
                building_description_id: building_state.building_description_id,
                constructed_by_player_entity_id: building_state.constructed_by_player_entity_id,
                building_name: state
                    .building_nickname_state
                    .get(&(building_state.entity_id))
                    .map_or_else(
                        || {
                            state
                                .building_desc
                                .get(&(building_state.building_description_id as i64))
                                .map_or("".into(), |building_desc| building_desc.name.clone())
                        },
                        |building_nickname_state| building_nickname_state.nickname.clone(),
                    ),
                location: state
                    .location_state
                    .get(&(building_state.entity_id))
                    .map(|location_state| location_state.to_owned()),
            })
            .collect(),
        per_page,
        total: posts.1.number_of_items,
        page,
    }))
}

pub(crate) async fn find_building_state(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<building_state::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_state_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingState not found"));
    }

    Ok(axum_codec::Codec(posts.unwrap()))
}
