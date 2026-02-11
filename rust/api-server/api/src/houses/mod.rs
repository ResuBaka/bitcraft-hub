pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/houses",
            axum_codec::routing::get(find_houses).into(),
        )
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
        .route(
            "/houses/{id}",
            axum_codec::routing::get(find_house).into(),
        )
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
    pub owner: Option<String>,
}

// ─── Response Types ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct PermissionEntry {
    pub allowed_entity_id: i64,
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
pub(crate) struct HouseInventoriesResponse {
    pub house_entity_id: i64,
    pub dimension_id: Option<i64>,
    pub inventories: Vec<entity::inventory::Model>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /houses?owner={owner}
///
/// Finds houses by owner username or player entity ID.
pub(crate) async fn find_houses(
    state: State<AppState>,
    query: axum::extract::Query<FindHousesQuery>,
) -> Result<axum_codec::Codec<Vec<HouseResponse>>, (StatusCode, &'static str)> {
    let owner_param = query.owner.as_deref().unwrap_or_default();
    if owner_param.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing owner parameter"));
    }

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

    find_houses_by_owner_id(&state, owner_id).await
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
    let building_ids: Vec<i64> = ownerships.into_iter().map(|p| p.ordained_entity_id).collect();
    tracing::info!("building_ids (from ordained_entity_id): {:#?}", building_ids);
    // 2. Find Houses that have these entrance buildings OR are the houses themselves
    let houses = ::entity::player_housing_state::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(::entity::player_housing_state::Column::EntranceBuildingEntityId.is_in(building_ids.clone()))
                .add(::entity::player_housing_state::Column::EntityId.is_in(building_ids))
        )
        .all(&state.conn)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    tracing::info!("houses found: {:#?}", houses);
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

    // Find owner from PermissionState where entity_id or ordained_entity_id matches entrance_building_entity_id or house entity_id
    let owner_permission = ::entity::permission_state::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(::entity::permission_state::Column::EntityId.eq(house.entrance_building_entity_id))
                .add(::entity::permission_state::Column::OrdainedEntityId.eq(house.entrance_building_entity_id))
                .add(::entity::permission_state::Column::EntityId.eq(house.entity_id))
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
    let permissions = ::entity::permission_state::Entity::find()
        .filter(
            ::entity::permission_state::Column::OrdainedEntityId
                .eq(house.entrance_building_entity_id),
        )
        .all(&state.conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| PermissionEntry {
            allowed_entity_id: p.allowed_entity_id,
            group: p.group,
            rank: p.rank,
        })
        .collect();

    // Look up owner username
    let owner_username = if owner_id != 0 {
        ::entity::player_username_state::Entity::find()
            .filter(::entity::player_username_state::Column::EntityId.eq(owner_id as u64))
            .one(&state.conn)
            .await
            .unwrap_or_default()
            .map(|u| u.username)
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

    // 2. Find the interior network desc for this building
    let interior_network = ::entity::interior_network_desc::Entity::find_by_id(
        house.entrance_building_entity_id as i32,
    )
    .one(&state.conn)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    // 3. If we have an interior network, find the dimension description
    let dimension_id = if let Some(interior) = interior_network {
        let dim_state = ::entity::dimension_description_state::Entity::find()
            .filter(
                ::entity::dimension_description_state::Column::InteriorInstanceId
                    .eq(interior.building_id as i64),
            )
            .one(&state.conn)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        dim_state.map(|d| d.dimension_id)
    } else {
        None
    };

    // 4. Query inventories owned by the entrance building entity
    let inventories = ::entity::inventory::Entity::find()
        .filter(
            entity::inventory::Column::OwnerEntityId
                .eq(house.entrance_building_entity_id),
        )
        .all(&state.conn)
        .await
        .unwrap_or_default();

    Ok(axum_codec::Codec(HouseInventoriesResponse {
        house_entity_id: id,
        dimension_id,
        inventories,
    }))
}
