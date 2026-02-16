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
        // Resolve owner entity ID
        let owner_id = if let Ok(id) = owner_param.parse::<i64>() {
            id
        } else {
            // Search by username
            let user = ::entity::player_username_state::Entity::find()
                .filter(::entity::player_username_state::Column::Username.eq(owner_param))
                .one(&state.conn)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
                .ok_or((StatusCode::NOT_FOUND, "User not found"))?;
            user.entity_id as i64
        };

        // Find houses for this owner
        // 1. Find permissions where allowed_entity_id is owner_id and rank is 7 (Owner) for buildings/houses
        let ownerships = ::entity::permission_state::Entity::find()
            .filter(::entity::permission_state::Column::AllowedEntityId.eq(owner_id))
            .filter(::entity::permission_state::Column::Rank.eq(7)) // Permission::Owner matches rank 7
            .all(&state.conn)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        // Collect building IDs that are owned
        let building_ids: Vec<i64> = ownerships
            .into_iter()
            .map(|p| p.ordained_entity_id)
            .collect();

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
        // Find owner from PermissionState
        let owner_permission = ::entity::permission_state::Entity::find()
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
            .unwrap_or_default(); // Ignore errors here, just handle missing owner

        let owner_id = owner_permission
            .as_ref()
            .map(|p| p.allowed_entity_id)
            .unwrap_or(0);
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
    // 1. Find permissions where allowed_entity_id is owner_id and rank is 7 (Owner)
    let ownerships = ::entity::permission_state::Entity::find()
        .filter(::entity::permission_state::Column::AllowedEntityId.eq(owner_id))
        .filter(::entity::permission_state::Column::Rank.eq(7)) // Permission::Owner matches rank 7
        .all(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
    let building_ids: Vec<i64> = ownerships
        .into_iter()
        .map(|p| p.ordained_entity_id)
        .collect();
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

    // Find owner from PermissionState where ordained_entity_id matches entrance_building_entity_id or house entity_id
    let owner_permission = ::entity::permission_state::Entity::find()
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
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let owner_id = owner_permission.map(|p| p.allowed_entity_id).unwrap_or(0);

    build_house_response(&state, house, owner_id).await
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

    let usernames: HashMap<i64, String> = if !user_ids.is_empty() {
        let models = ::entity::player_username_state::Entity::find()
            .filter(::entity::player_username_state::Column::EntityId.is_in(user_ids.clone()))
            .all(&state.conn)
            .await
            .unwrap_or_default();

        models
            .into_iter()
            .map(|u| (u.entity_id as i64, u.username))
            .collect()
    } else {
        HashMap::new()
    };

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

    // 2. Find the dimension associated with this house
    let dimension_desc = ::entity::dimension_description_state::Entity::find()
        .filter(
            ::entity::dimension_description_state::Column::DimensionNetworkEntityId
                .eq(house.network_entity_id),
        )
        .one(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let mut dimension_id = dimension_desc.as_ref().map(|d| d.dimension_id);

    if dimension_id.is_none() {
        // Try fallback to InteriorNetworkDesc
        // let network_desc =
        //    ::entity::interior_network_desc::Entity::find_by_id(house.network_entity_id as i32)
        //        .one(&state.conn)
        //        .await
        //        .unwrap_or_default();

        // Fallback to LocationState via exit portal
        let portal_exit = ::entity::portal_state::Entity::find_by_id(house.exit_portal_entity_id)
            .one(&state.conn)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        if let Some(portal) = portal_exit {
            let location_state = ::entity::location_state::Entity::find_by_id(portal.entity_id)
                .one(&state.conn)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
        }
    }

    // 3. Find all entities in this dimension
    let mut dimension_entity_ids = vec![];

    if let Some(dim_id) = dimension_id {
        // Query LocationState for all entities in this specific interior dimension AND region
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
        dimension_entity_ids = locations.into_iter().map(|loc| loc.entity_id).collect();
    }
    let mut buildings_entity_ids = vec![];
    // 3b. Find only buildings and deployables assigned to this specific interior claim
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
    for building in buildings {
        buildings_entity_ids.push(building.entity_id);
    }

    // 4. Query inventories
    let inventory_models = ::entity::inventory::Entity::find()
        .filter(::entity::inventory::Column::OwnerEntityId.is_in(buildings_entity_ids))
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
    let mut inventories = Vec::with_capacity(inventory_models.len());
    for inventory in inventory_models {
        let mut pockets = Vec::with_capacity(inventory.pockets.len());
        for pocket in &inventory.pockets {
            pockets.push(crate::inventory::resolve_pocket(
                pocket,
                &state.item_desc,
                &state.cargo_desc,
            ));
        }

        let mut nickname = None;

        // Try to get building nickname
        let building_state =
            ::entity::building_state::Entity::find_by_id(inventory.owner_entity_id)
                .one(&state.conn)
                .await
                .unwrap_or_default();

        if let Some(building) = building_state {
            if let Some(name) = state.building_nickname_state.get(&building.entity_id) {
                nickname = Some(name.nickname.clone());
            }

            if nickname.is_none() {
                if let Some(desc) = state
                    .building_desc
                    .get(&(building.building_description_id as i64))
                {
                    nickname = Some(desc.name.clone());
                }
            }
        }

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

    Ok(axum_codec::Codec(HouseInventoriesResponse {
        house_entity_id: id,
        dimension_id,
        inventories,
    }))
}
