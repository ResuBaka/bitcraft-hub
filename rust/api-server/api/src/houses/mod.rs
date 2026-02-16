pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/houses", axum_codec::routing::get(find_houses).into())
        .route(
            "/api/bitcraft/houses",
            axum_codec::routing::get(find_houses).into(),
        )
        .route(
            "/houses/by_owner/{id}",
            axum_codec::routing::get(find_houses_by_owner).into(),
        )
        .route(
            "/api/bitcraft/houses/by_owner/{id}",
            axum_codec::routing::get(find_houses_by_owner).into(),
        )
        .route("/houses/{id}", axum_codec::routing::get(find_house).into())
        .route(
            "/api/bitcraft/houses/{id}",
            axum_codec::routing::get(find_house).into(),
        )
        .route(
            "/houses/{id}/inventories",
            axum_codec::routing::get(find_house_inventories).into(),
        )
        .route(
            "/api/bitcraft/houses/{id}/inventories",
            axum_codec::routing::get(find_house_inventories).into(),
        )
}

// ─── Request Types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct FindHousesQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub owner: Option<String>,
}

// ─── Response Types ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct PermissionEntry {
    pub allowed_entity_id: i64,
    pub allowed_username: Option<String>,
    pub group: i32,
    pub rank: i32,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct HouseResponse {
    pub entity_id: i64,
    pub entrance_building_entity_id: i64,
    pub network_entity_id: i64,
    pub exit_portal_entity_id: i64,
    pub rank: i32,
    pub is_empty: bool,
    pub region_index: i32,
    pub region: String,
    /// The player entity ID who owns this house.
    pub owner_entity_id: i64,
    /// The username of the owner, if known.
    pub owner_username: Option<String>,
    /// Players who are granted permissions on this house.
    pub permissions: Vec<PermissionEntry>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct HousesResponse {
    pub houses: Vec<HouseResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct HouseInventoriesResponse {
    pub house_entity_id: i64,
    pub dimension_id: Option<i64>,
    pub inventories: Vec<entity::inventory::ResolvedInventory>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /houses?owner={owner}&page={page}&per_page={per_page}
///
/// Finds houses, optionally filtering by owner. Supports pagination.
pub(crate) async fn find_houses(
    state: State<AppState>,
    query: axum::extract::Query<FindHousesQuery>,
) -> Result<axum_codec::Codec<HousesResponse>, (StatusCode, &'static str)> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    // Limit per_page to avoid massive queries
    let per_page = per_page.min(100);

    let owner_param = query.owner.as_deref().unwrap_or_default();

    let mut house_query = ::entity::player_housing_state::Entity::find();

    // Apply filtering if owner is provided
    if !owner_param.is_empty() {
        let owner_id = resolve_owner_id_by_param(&state, owner_param).await?;
        let building_ids = get_owned_building_entity_ids(&state, owner_id).await?;

        // Filter houses that match these IDs
        house_query = house_query.filter(
            sea_orm::Condition::any()
                .add(
                    ::entity::player_housing_state::Column::EntranceBuildingEntityId
                        .is_in(building_ids.clone()),
                )
                .add(::entity::player_housing_state::Column::EntityId.is_in(building_ids)),
        );
    }

    // Pagination
    let paginator = house_query.paginate(&state.conn, per_page);
    let total = paginator
        .num_items()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
    let houses_models = paginator
        .fetch_page(page - 1)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let mut houses_response = Vec::with_capacity(houses_models.len());

    for house in houses_models {
        let owner_id = get_owner_entity_id_for_house(&state, &house).await;
        let resp = build_house_response(&state, house, owner_id).await?;
        houses_response.push(resp.0);
    }

    Ok(axum_codec::Codec(HousesResponse {
        houses: houses_response,
        page,
        per_page,
        total,
    }))
}

/// GET /houses/by_owner/{id}
///
/// Finds houses by owner player entity ID.
pub(crate) async fn find_houses_by_owner(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<Vec<HouseResponse>>, (StatusCode, &'static str)> {
    find_houses_by_owner_id(&state, id).await
}

async fn find_houses_by_owner_id(
    state: &AppState,
    owner_id: i64,
) -> Result<axum_codec::Codec<Vec<HouseResponse>>, (StatusCode, &'static str)> {
    let building_ids = get_owned_building_entity_ids(state, owner_id).await?;

    // 2. Find Houses that have these entrance buildings OR are the houses themselves
    let houses = ::entity::player_housing_state::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(
                    ::entity::player_housing_state::Column::EntranceBuildingEntityId
                        .is_in(building_ids.clone()),
                )
                .add(::entity::player_housing_state::Column::EntityId.is_in(building_ids)),
        )
        .all(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let mut response = Vec::new();
    for house in houses {
        let resp = build_house_response(state, house, owner_id).await?;
        response.push(resp.0);
    }

    Ok(axum_codec::Codec(response))
}

/// GET /houses/{id}
///
/// Returns the house details for the given house entity id.
pub(crate) async fn find_house(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<HouseResponse>, (StatusCode, &'static str)> {
    // Look up the housing state by entity_id
    let house = ::entity::player_housing_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "House not found"))?;

    let owner_id = get_owner_entity_id_for_house(&state, &house).await;

    build_house_response(&state, house, owner_id).await
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn resolve_owner_id_by_param(
    state: &AppState,
    owner_param: &str,
) -> Result<i64, (StatusCode, &'static str)> {
    if let Ok(id) = owner_param.parse::<i64>() {
        Ok(id)
    } else {
        // Search by username
        let user = ::entity::player_username_state::Entity::find()
            .filter(::entity::player_username_state::Column::Username.eq(owner_param))
            .one(&state.conn)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::NOT_FOUND, "User not found"))?;
        Ok(user.entity_id as i64)
    }
}

async fn get_owned_building_entity_ids(
    state: &AppState,
    owner_id: i64,
) -> Result<Vec<i64>, (StatusCode, &'static str)> {
    let ownerships = ::entity::permission_state::Entity::find()
        .filter(::entity::permission_state::Column::AllowedEntityId.eq(owner_id))
        .filter(::entity::permission_state::Column::Rank.eq(7)) // Permission::Owner matches rank 7
        .all(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    Ok(ownerships
        .into_iter()
        .map(|p| p.ordained_entity_id)
        .collect())
}

async fn get_owner_entity_id_for_house(
    state: &AppState,
    house: &::entity::player_housing_state::Model,
) -> i64 {
    ::entity::permission_state::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(
                    ::entity::permission_state::Column::OrdainedEntityId
                        .eq(house.entrance_building_entity_id),
                )
                .add(::entity::permission_state::Column::OrdainedEntityId.eq(house.entity_id)),
        )
        .filter(::entity::permission_state::Column::Rank.eq(7))
        .one(&state.conn)
        .await
        .unwrap_or_default()
        .map(|p| p.allowed_entity_id)
        .unwrap_or(0)
}

async fn build_house_response(
    state: &AppState,
    house: ::entity::player_housing_state::Model,
    owner_id: i64,
) -> Result<axum_codec::Codec<HouseResponse>, (StatusCode, &'static str)> {
    // Look up permissions where ordained_entity_id matches the house's
    // entrance_building_entity_id (the building that is the house).
    let permission_models = ::entity::permission_state::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(
                    ::entity::permission_state::Column::OrdainedEntityId
                        .eq(house.entrance_building_entity_id),
                )
                // Also check against house entity ID directly to be safe
                .add(::entity::permission_state::Column::OrdainedEntityId.eq(house.entity_id)),
        )
        .all(&state.conn)
        .await
        .unwrap_or_default();

    // Collect all allowed_entity_ids to fetch usernames
    let mut user_ids: Vec<i64> = permission_models
        .iter()
        .map(|p| p.allowed_entity_id)
        .collect();
    if owner_id != 0 {
        user_ids.push(owner_id);
    }
    user_ids.sort();
    user_ids.dedup();

    let usernames = get_usernames_for_ids(state, &user_ids).await;

    let permissions = permission_models
        .into_iter()
        .map(|p| PermissionEntry {
            allowed_entity_id: p.allowed_entity_id,
            allowed_username: usernames.get(&p.allowed_entity_id).cloned(),
            group: p.group,
            rank: p.rank,
        })
        .collect();

    let owner_username = if owner_id != 0 {
        usernames.get(&owner_id).cloned()
    } else {
        None
    };

    Ok(axum_codec::Codec(HouseResponse {
        entity_id: house.entity_id,
        entrance_building_entity_id: house.entrance_building_entity_id,
        network_entity_id: house.network_entity_id,
        exit_portal_entity_id: house.exit_portal_entity_id,
        rank: house.rank,
        is_empty: house.is_empty,
        region_index: house.region_index,
        region: house.region,
        owner_entity_id: owner_id,
        owner_username,
        permissions,
    }))
}

/// GET /houses/{id}/inventories
///
/// Resolves the interior dimension for the house and returns all inventories
/// in that dimension.
pub(crate) async fn find_house_inventories(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<HouseInventoriesResponse>, (StatusCode, &'static str)> {
    // 1. Find the house
    let house = ::entity::player_housing_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::NOT_FOUND, "House not found"))?;

    let dimension_id = get_dimension_id_for_house(&state, &house).await?;

    let mut inventories = Vec::new();

    if let Some(dim_id) = dimension_id {
        let building_ids = get_building_entity_ids_in_dimension(&state, &house, dim_id).await?;

        // 4. Query inventories
        let inventory_models = ::entity::inventory::Entity::find()
            .filter(::entity::inventory::Column::OwnerEntityId.is_in(building_ids))
            .all(&state.conn)
            .await
            .map_err(|e| {
                tracing::error!("Database error querying Inventory: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error querying Inventory",
                )
            })?;

        // 5. Resolve the inventories
        for inventory in inventory_models {
            let pockets = resolve_inventory_pockets(&state, &inventory.pockets);
            let nickname = get_building_nickname(&state, inventory.owner_entity_id).await;

            inventories.push(::entity::inventory::ResolvedInventory {
                entity_id: inventory.entity_id,
                pockets,
                inventory_index: inventory.inventory_index,
                cargo_index: inventory.cargo_index,
                owner_entity_id: inventory.owner_entity_id,
                player_owner_entity_id: inventory.player_owner_entity_id,
                nickname,
                claim: None,
            });
        }
    }

    Ok(axum_codec::Codec(HouseInventoriesResponse {
        house_entity_id: id,
        dimension_id,
        inventories,
    }))
}

async fn get_dimension_id_for_house(
    state: &AppState,
    house: &::entity::player_housing_state::Model,
) -> Result<Option<i64>, (StatusCode, &'static str)> {
    let portal_exit = ::entity::portal_state::Entity::find_by_id(house.exit_portal_entity_id)
        .one(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if let Some(portal) = portal_exit {
        let portal_location = ::entity::location_state::Entity::find_by_id(portal.entity_id)
            .one(&state.conn)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        if let Some(loc) = portal_location {
            return Ok(Some(loc.dimension));
        }
    }

    Ok(None)
}

async fn get_building_entity_ids_in_dimension(
    state: &AppState,
    house: &::entity::player_housing_state::Model,
    dim_id: i64,
) -> Result<Vec<i64>, (StatusCode, &'static str)> {
    // dimension_id is only unique within a region
    let mut region: String = "bitcraft-".to_owned();
    region.push_str(&house.region_index.to_string());

    let locations = ::entity::location_state::Entity::find()
        .filter(::entity::location_state::Column::Dimension.eq(dim_id))
        .filter(::entity::location_state::Column::Region.eq(region))
        .all(&state.conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error querying LocationState: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error querying LocationState",
            )
        })?;

    let dimension_entity_ids: Vec<i64> = locations.into_iter().map(|loc| loc.entity_id).collect();

    let buildings = ::entity::building_state::Entity::find()
        .filter(::entity::building_state::Column::EntityId.is_in(dimension_entity_ids))
        .all(&state.conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error querying BuildingState: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error querying BuildingState",
            )
        })?;

    Ok(buildings.into_iter().map(|b| b.entity_id).collect())
}

fn resolve_inventory_pockets(
    state: &AppState,
    pockets: &[::entity::inventory::Pocket],
) -> Vec<::entity::inventory::ItemSlotResolved> {
    pockets
        .iter()
        .map(|pocket| crate::inventory::resolve_pocket(pocket, &state.item_desc, &state.cargo_desc))
        .collect()
}

async fn get_building_nickname(state: &AppState, entity_id: i64) -> Option<String> {
    // Try to get building nickname from AppState cache first
    if let Some(name) = state.building_nickname_state.get(&entity_id) {
        return Some(name.nickname.clone());
    }

    // Fallback to database then description name
    let building_state = ::entity::building_state::Entity::find_by_id(entity_id)
        .one(&state.conn)
        .await
        .unwrap_or_default();

    if let Some(building) = building_state {
        if let Some(desc) = state
            .building_desc
            .get(&(building.building_description_id as i64))
        {
            return Some(desc.name.clone());
        }
    }

    None
}

async fn get_usernames_for_ids(state: &AppState, user_ids: &[i64]) -> HashMap<i64, String> {
    if user_ids.is_empty() {
        return HashMap::new();
    }

    let models = ::entity::player_username_state::Entity::find()
        .filter(::entity::player_username_state::Column::EntityId.is_in(user_ids.to_vec()))
        .all(&state.conn)
        .await
        .unwrap_or_default();

    models
        .into_iter()
        .map(|u| (u.entity_id as i64, u.username))
        .collect()
}
