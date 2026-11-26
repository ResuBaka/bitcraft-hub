use crate::AppState;
use crate::world_map::RegionFilter;
use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use entity::{claim_tech_state, shared::Region};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const OVERWORLD: u32 = 1;
const BATCH_SIZE: usize = 4096;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimMarker {
    // Entity IDs exceed JavaScript's safe integer range.
    id: String,
    name: String,
    tier: i32,
    region: Region,
    // The claim flag's position in the small-hex offset grid.
    x: i32,
    z: i32,
    tiles: i32,
    members: usize,
}

#[derive(Deserialize)]
pub(super) struct Params {
    regions: Option<String>,
    claim: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct RosterParams {
    ids: String,
}

#[derive(Serialize)]
struct RosterMember {
    id: String,
    name: String,
}

pub(super) async fn rosters(
    State(state): State<AppState>,
    Query(params): Query<RosterParams>,
) -> Result<impl IntoResponse, StatusCode> {
    if params.ids.len() > 419 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ids = params
        .ids
        .split(',')
        .map(|id| {
            id.parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or(StatusCode::BAD_REQUEST)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.len() > 20 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let rosters: HashMap<_, _> = ids
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| {
            let owner = state.claim_state.get(&(id as i64))?.owner_player_entity_id as u64;
            let mut members = std::collections::BTreeMap::new();
            if let Some(rows) = state.claim_member_state.get(&id) {
                for row in rows.values().filter(|row| row.player_entity_id != 0) {
                    members.insert(row.player_entity_id as u64, row.user_name.clone());
                }
            }
            if owner != 0 {
                members.entry(owner).or_default();
            }
            Some((
                id.to_string(),
                members
                    .into_iter()
                    .map(|(id, name)| RosterMember {
                        id: id.to_string(),
                        name,
                    })
                    .collect::<Vec<_>>(),
            ))
        })
        .collect();
    Ok(Json(rosters))
}

pub(super) async fn list(
    State(state): State<AppState>,
    Query(params): Query<Params>,
) -> Result<impl IntoResponse, StatusCode> {
    let regions = RegionFilter::parse(params.regions.as_deref())?;
    let mut positions = state
        .claim_local_state
        .iter()
        .filter_map(|entry| {
            if !regions.includes(entry.region) && params.claim != Some(entry.entity_id as u64) {
                return None;
            }
            let location = entry
                .location
                .as_ref()
                .filter(|location| location.dimension == OVERWORLD)?;
            Some((
                entry.entity_id,
                entry.region,
                location.x,
                location.z,
                entry.num_tiles,
            ))
        })
        .collect::<Vec<_>>();
    positions.sort_unstable_by_key(|position| position.0);

    let upgrades = state
        .claim_tech_desc
        .iter()
        .map(|entry| entry.value().clone())
        .collect::<Vec<_>>();
    let starter = super::starter_claim_tier(&upgrades);
    let tier_upgrades = upgrades
        .iter()
        .filter(|upgrade| super::is_claim_tier_upgrade(&upgrade.tech_type))
        .map(|upgrade| (upgrade.id, upgrade.tier))
        .collect::<HashMap<_, _>>();

    // Project only the columns we need. No per-claim database requests or building lookups.
    let mut tiers = HashMap::with_capacity(positions.len());
    for batch in positions.chunks(BATCH_SIZE) {
        let learned = claim_tech_state::Entity::find()
            .select_only()
            .column(claim_tech_state::Column::EntityId)
            .column(claim_tech_state::Column::Learned)
            .filter(
                claim_tech_state::Column::EntityId.is_in(batch.iter().map(|position| position.0)),
            )
            .into_tuple::<(i64, Vec<i32>)>()
            .all(&state.conn)
            .await
            .map_err(|error| {
                tracing::error!(%error, "Could not load claim tech for world map");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        tiers.extend(learned.into_iter().map(|(id, learned)| {
            let tier = learned
                .iter()
                .filter_map(|upgrade| tier_upgrades.get(upgrade))
                .copied()
                .max()
                .unwrap_or(starter);
            (id, tier)
        }));
    }
    let markers = positions
        .into_iter()
        .filter_map(|(id, region, x, z, tiles)| {
            let name = state.claim_state.get(&id)?.name.clone();
            Some(ClaimMarker {
                id: (id as u64).to_string(),
                name,
                tier: tiers.get(&id).copied().unwrap_or(starter),
                region,
                x,
                z,
                tiles,
                members: state
                    .claim_member_state
                    .get(&(id as u64))
                    .map_or(0, |members| members.len()),
            })
        })
        .collect::<Vec<_>>();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=30")],
        Json(markers),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn roster_route_returns_current_unique_members_and_preserves_exact_ids() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let claim_id = 9_007_199_254_740_993i64;
        state.claim_state.insert(
            claim_id,
            entity::claim_state::Model {
                entity_id: claim_id,
                owner_player_entity_id: -1,
                owner_building_entity_id: 0,
                name: "Monitor test".into(),
                neutral: false,
                region: 7,
            },
        );
        for id in [1, 2] {
            state.add_claim_member(entity::claim_member_state::Model {
                entity_id: id,
                claim_entity_id: claim_id,
                player_entity_id: 42,
                user_name: "Bea".into(),
                inventory_permission: false,
                build_permission: false,
                officer_permission: false,
                co_owner_permission: false,
                region: 7,
            });
        }
        let app = super::super::get_routes().with_state(state);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/world-map/claim-rosters?ids={claim_id},1,{claim_id}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({ (claim_id.to_string()): [
                {"id": "42", "name": "Bea"},
                {"id": "18446744073709551615", "name": ""}
            ] })
        );
        let invalid = app
            .oneshot(
                Request::builder()
                    .uri("/world-map/claim-rosters?ids=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
}
