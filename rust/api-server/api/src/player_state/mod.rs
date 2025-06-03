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
}

#[derive(Deserialize)]
pub struct ListPlayersParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    online: Option<bool>,
}

pub async fn list_players(
    state: State<std::sync::Arc<AppState>>,
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
                .unwrap();

            player_state::PlayerStateMerged {
                entity_id: player.entity_id as u64,
                time_played: player.time_played,
                session_start_timestamp: player.session_start_timestamp,
                time_signed_in: player.time_signed_in,
                sign_in_timestamp: player.sign_in_timestamp,
                signed_in: player.signed_in,
                traveler_tasks_expiration: player.traveler_tasks_expiration,
                teleport_location: player.teleport_location,
                username: player_username.username.clone(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct PlayersResponse {
    pub players: Vec<player_state::PlayerStateMerged>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FindPlayerByIdResponse {
    pub teleport_location: TeleportLocation,
    pub entity_id: i64,
    pub time_played: i32,
    pub session_start_timestamp: i32,
    pub time_signed_in: i32,
    pub sign_in_timestamp: i32,
    pub signed_in: bool,
    pub traveler_tasks_expiration: i32,
    pub username: String,
    pub deployables: Vec<VaultStateCollectibleWithDesc>,
    pub claim_id: Option<u64>,
    pub claim_ids: Vec<u64>,
    pub player_location: Option<mobile_entity_state::Model>,
    pub player_action_state: Option<String>,
    pub player_action_state2: Option<entity::player_action_state::Model>,
    pub current_action_state: Option<entity::player_action_state::Model>,
}

pub async fn find_player_by_id(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<FindPlayerByIdResponse>, (StatusCode, &'static str)> {
    let player = player_state::Entity::find_by_id(id)
        .one(&state.conn)
        .await
        .expect("Cannot find player");

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
            error!("find_player_by_id -> Error: {:?}", error);
            vec![]
        });

    let player_location = state
        .mobile_entity_state
        .get(&(id as u64))
        .map(|player_location| player_location.clone());

    let chunk_index = player_location
        .as_ref()
        .map(|player_location| player_location.chunk_index);

    let claim_id = if chunk_index.is_some() {
        if let Some(claim_id) = state.claim_tile_state.get(&(chunk_index.unwrap())) {
            Some(claim_id.claim_id)
        } else {
            None
        }
    } else {
        None
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
    let current_action_state = state.action_state.get(&(id as u64)).and_then(|_value| {
        // @todo implement it correctly
        None
    });

    let claim_ids = state
        .player_to_claim_id_cache
        .get(&(player.entity_id as u64))
        .map_or(vec![], |ids| ids.iter().map(|id| *id).collect());

    Ok(axum_codec::Codec(FindPlayerByIdResponse {
        entity_id: player.entity_id,
        time_played: player.time_played,
        session_start_timestamp: player.session_start_timestamp,
        time_signed_in: player.time_signed_in,
        sign_in_timestamp: player.sign_in_timestamp,
        signed_in: player.signed_in,
        teleport_location: player.teleport_location,
        traveler_tasks_expiration: player.traveler_tasks_expiration,
        username: player_username,
        deployables,
        player_location,
        claim_id,
        claim_ids,
        player_action_state,
        player_action_state2,
        current_action_state,
    }))
}
