pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::player_state::TeleportLocation;
use entity::vault_state_collectibles::VaultStateCollectibleWithDesc;
use entity::{mobile_entity_state, player_state, player_username_state};
use log::error;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::collections::HashMap;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/players", axum_codec::routing::get(list_players).into())
        .route(
            "/players/{id}",
            axum_codec::routing::get(find_player_by_id).into(),
        )
        .route(
            "/api/bitcraft/players",
            axum_codec::routing::get(list_players).into(),
        )
        .route(
            "/api/bitcraft/players/{id}",
            axum_codec::routing::get(find_player_by_id).into(),
        )
        .route(
            "/api/bitcraft/players/{id}/location",
            axum_codec::routing::get(player_location).into(),
        )
        .route(
            "/api/bitcraft/players/all",
            axum_codec::routing::get(get_all).into(),
        )
}

#[derive(Deserialize)]
pub struct ListPlayersParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    online: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct PlayerUsernameStateResponse {
    username_state: HashMap<String, String>,
}
pub(crate) async fn get_all(
    state: State<AppState>,
) -> Result<axum_codec::Codec<PlayerUsernameStateResponse>, (StatusCode, &'static str)> {
    let currently_known_player_username_state = ::entity::player_username_state::Entity::find()
        .all(&state.conn)
        .await
        .unwrap_or(vec![])
        .into_iter()
        .map(|value| (value.entity_id.to_string(), value.username))
        .collect::<HashMap<_, _>>();
    Ok(axum_codec::Codec(PlayerUsernameStateResponse {
        username_state: currently_known_player_username_state,
    }))
}

pub async fn list_players(
    state: State<AppState>,
    Query(params): Query<ListPlayersParams>,
) -> Result<axum_codec::Codec<PlayersResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;
    let online = params.online;

    let (player, player_usernames, num_pages) =
        QueryCore::find_players(&state.conn, page, posts_per_page, search, online)
            .await
            .expect("Cannot find player_state in page");

    let merged_player = player
        .into_iter()
        .map(|player| {
            let player_username = player_usernames
                .iter()
                .find(|player_username| player_username.entity_id == player.entity_id)
                .map_or("".into(), |player_username| {
                    player_username.username.clone()
                });

            player_state::PlayerStateMerged {
                entity_id: player.entity_id as u64,
                time_played: player.time_played,
                session_start_timestamp: player.session_start_timestamp,
                time_signed_in: player.time_signed_in,
                sign_in_timestamp: player.sign_in_timestamp,
                signed_in: player.signed_in,
                traveler_tasks_expiration: player.traveler_tasks_expiration,
                teleport_location: player.teleport_location,
                username: player_username,
            }
        })
        .collect::<Vec<player_state::PlayerStateMerged>>();

    Ok(axum_codec::Codec(PlayersResponse {
        players: merged_player,
        per_page: posts_per_page,
        total: num_pages.number_of_items,
        page,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayersResponse {
    pub players: Vec<player_state::PlayerStateMerged>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct FindPlayerByIdResponse {
    pub teleport_location: TeleportLocation,
    pub entity_id: i64,
    pub time_played: i32,
    pub session_start_timestamp: i32,
    pub time_signed_in: i32,
    pub sign_in_timestamp: i32,
    pub signed_in: bool,
    pub traveler_tasks_expiration: i32,
    pub traveler_tasks: HashMap<i32, Vec<entity::traveler_task_state::Model>>,
    pub username: String,
    pub deployables: Vec<VaultStateCollectibleWithDesc>,
    pub claim_id: Option<u64>,
    pub current_claim: CurrentClaim,
    pub claims: Vec<entity::claim_state::Model>,
    pub player_location: Option<mobile_entity_state::Model>,
    pub player_action_state: Option<String>,
    pub player_action_state2: Option<entity::player_action_state::Model>,
    pub current_action_state: Option<entity::player_action_state::Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum CurrentClaim {
    Unknown,
    Unclaimed,
    Claim { claim_id: u64, name: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct PlayerLocationUpdate {
    pub entity_id: u64,
    pub location: Option<mobile_entity_state::Model>,
    pub current_claim: CurrentClaim,
}

pub(crate) fn resolve_player_location(
    state: &AppState,
    entity_id: u64,
    location: Option<mobile_entity_state::Model>,
) -> PlayerLocationUpdate {
    use crate::claims::claim_location::ClaimAtLocation;
    let current_claim = match location.as_ref().map(|location| {
        state.claim_locations.lookup(location.region, location.dimension, location.location_x, location.location_z)
    }).unwrap_or(ClaimAtLocation::Unknown) {
        ClaimAtLocation::Unknown => CurrentClaim::Unknown,
        ClaimAtLocation::Unclaimed => CurrentClaim::Unclaimed,
        ClaimAtLocation::Claim(claim_id) => CurrentClaim::Claim {
            claim_id,
            name: state.claim_state.get(&(claim_id as i64)).map(|claim| claim.name.clone()),
        },
    };
    PlayerLocationUpdate { entity_id, location, current_claim }
}

async fn player_location(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<PlayerLocationUpdate>, (StatusCode, &'static str)> {
    if !state.player_state.contains_key(&id) {
        return Err((StatusCode::NOT_FOUND, "Player not found"));
    }
    let location = state.mobile_entity_state.get(&(id as u64)).map(|row| row.value().clone());
    Ok(axum_codec::Codec(resolve_player_location(&state, id as u64, location)))
}

#[cfg(test)]
mod current_location_tests {
    use super::*;
    use game_module::module_bindings::{ClaimTileState, LocationState};

    #[test]
    fn location_resolves_claim_unclaimed_and_unknown() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(Default::default(), tx, Default::default());
        let location = mobile_entity_state::Model {
            entity_id: 1, chunk_index: 5, timestamp: 0, location_x: 1000, location_z: 2000,
            destination_x: 1000, destination_z: 2000, dimension: 1, is_walking: false, region: 2,
        };
        assert!(matches!(resolve_player_location(&state, 1, Some(location.clone())).current_claim, CurrentClaim::Unknown));
        state.claim_locations.reset(2);
        state.claim_locations.finish_initial(2);
        state.claim_locations.claims_initial(2, &[ClaimTileState { entity_id: 42, claim_id: 99 }]);
        state.claim_locations.locations_initial(2, &[LocationState { entity_id: 42, chunk_index: 5, x: 1, z: 2, dimension: 1 }]);
        state.claim_state.insert(99, entity::claim_state::Model {
            entity_id: 99, owner_player_entity_id: 1, owner_building_entity_id: 0,
            name: "Test claim".into(), neutral: false, region: 2,
        });
        assert!(matches!(resolve_player_location(&state, 1, Some(location.clone())).current_claim,
            CurrentClaim::Claim { claim_id: 99, name: Some(name) } if name == "Test claim"));
        state.claim_locations.claim(2, 42, None);
        assert!(matches!(resolve_player_location(&state, 1, Some(location)).current_claim, CurrentClaim::Unclaimed));
        assert!(matches!(resolve_player_location(&state, 1, None).current_claim, CurrentClaim::Unknown));
    }
}

pub async fn find_player_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<FindPlayerByIdResponse>, (StatusCode, &'static str)> {
    let player = state
        .player_state
        .get(&id)
        .map(|player| player.value().clone());

    if player.is_none() {
        return Err((StatusCode::NOT_FOUND, "Player not found"));
    }

    let player = player.unwrap();

    let player_username = player_username_state::Entity::find()
        .filter(player_username_state::Column::EntityId.eq(id))
        .one(&state.conn)
        .await
        .expect("Cannot find player_username");

    let player_username = match player_username {
        Some(player_username) => player_username.username,
        None => "".to_string(),
    };

    let deployables = QueryCore::find_vault_deployable_by_player_with_desc(&state.conn, id)
        .await
        .unwrap_or_else(|error| {
            error!("find_player_by_id -> Error: {error:?}");
            vec![]
        });

    // let deasdasd = QueryCore::find_deployable_entity_by_owner_entity_id(&state.conn, id)
    //     .await
    //     .unwrap_or_else(|error| {
    //         error!("find_player_by_id -> Error: {error:?}");
    //         vec![]
    //     });
    //
    // for deployable in &deasdasd {
    //     if let Some(asd) = state.mobile_entity_state.get(&(deployable.entity_id as u64)) {
    //         tracing::info!("Aas N: {} E: {} R: {} D: {} deployable {}", asd.value().location_z / 3 / 1000, asd.value().location_x / 3 / 1000, asd.value().region, asd.value().dimension, deployable.nickname);
    //     }
    // }

    let player_location = state
        .mobile_entity_state
        .get(&(id as u64))
        .map(|player_location| player_location.clone());

    let resolved_location = resolve_player_location(&state, id as u64, player_location);
    let claim_id = match &resolved_location.current_claim {
        CurrentClaim::Claim { claim_id, .. } => Some(*claim_id),
        _ => None,
    };

    let player_action_state2 = state
        .player_action_state
        .get(&(id as u64))
        .map(|player_action_state| player_action_state.value().clone());
    let player_action_state = state
        .player_action_state
        .get(&(id as u64))
        .map(|player_action_state| player_action_state.action_type.get_action_name());

    //TODO FIX IT SO that it works with the correct type
    let current_action_state = None;

    let claim_ids = state
        .player_to_claim_id_cache
        .get(&(player.entity_id as u64))
        .map(|ids| ids.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let claims = claim_ids
        .into_iter()
        .filter_map(|id| {
            state
                .claim_state
                .get(&(id as i64))
                .map(|claim| claim.value().clone())
        })
        .collect();

    let traveler_tasks_db =
        QueryCore::get_traveler_task_state_by_player_entity_ids(&state.conn, vec![id])
            .await
            .unwrap_or_else(|error| {
                error!("find_player_by_id -> Error: {error:?}");
                vec![]
            });

    let mut traveler_tasks: HashMap<_, _> = HashMap::new();
    for task in traveler_tasks_db {
        let traveler_task = traveler_tasks
            .entry(task.traveler_id)
            .or_insert_with(Vec::new);
        traveler_task.push(task);
    }

    Ok(axum_codec::Codec(FindPlayerByIdResponse {
        entity_id: player.entity_id,
        time_played: player.time_played,
        session_start_timestamp: player.session_start_timestamp,
        time_signed_in: player.time_signed_in,
        sign_in_timestamp: player.sign_in_timestamp,
        signed_in: player.signed_in,
        teleport_location: player.teleport_location.clone(),
        traveler_tasks_expiration: player.traveler_tasks_expiration,
        traveler_tasks,
        username: player_username,
        deployables,
        player_location: resolved_location.location,
        claim_id,
        current_claim: resolved_location.current_claim,
        claims,
        player_action_state,
        player_action_state2,
        current_action_state,
    }))
}
