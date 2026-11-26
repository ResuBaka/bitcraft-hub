use crate::AppState;
use crate::world_map::RegionFilter;
use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use game_module::module_bindings::SmallHexTileMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MAX_WORLD_CELL: i32 = 96_000;
const MAX_SPAN: i32 = 2048;
const MAX_AREA: i32 = 1_500_000;
pub(super) type Cell = (i32, i32);

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl Bounds {
    pub(super) fn valid(self, buildings_only: bool) -> bool {
        if self.min_x < 0
            || self.min_z < 0
            || self.max_x > MAX_WORLD_CELL
            || self.max_z > MAX_WORLD_CELL
            || self.min_x > self.max_x
            || self.min_z > self.max_z
        {
            return false;
        }
        // Buildings are sparse cached points. Only a border query builds
        // a dense viewport's worth of tile occupancy and neighbor edges.
        if buildings_only {
            return true;
        }
        let width = self.max_x - self.min_x + 1;
        let height = self.max_z - self.min_z + 1;
        width <= MAX_SPAN && height <= MAX_SPAN && width * height <= MAX_AREA
    }

    fn with_neighbors(self) -> Self {
        Self {
            min_x: (self.min_x - 1).max(0),
            min_z: (self.min_z - 1).max(0),
            max_x: self.max_x + 1,
            max_z: self.max_z + 1,
        }
    }

    pub fn contains(self, (x, z): Cell) -> bool {
        (self.min_x..=self.max_x).contains(&x) && (self.min_z..=self.max_z).contains(&z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BuildingKind {
    Bank,
    RegionalBank,
    Market,
    Waystone,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct Building {
    kind: BuildingKind,
    pub(super) x: i32,
    pub(super) z: i32,
}

impl Building {
    pub(crate) fn from_game(
        kind: BuildingKind,
        claim_id: u64,
        coords: &SmallHexTileMessage,
    ) -> Option<Self> {
        // Service building tables carry axial coordinates. Claim tiles and
        // the map's marker projection use odd-row offset coordinates instead.
        (claim_id != 0 && coords.dimension == 1).then_some(Self {
            kind,
            x: coords.x.checked_add(coords.z / 2)?,
            z: coords.z,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Params {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    #[serde(default)]
    borders: bool,
    #[serde(default)]
    buildings: bool,
    regions: Option<String>,
}

#[derive(Serialize)]
struct Details {
    borders: Vec<(i32, i32, u8)>,
    buildings: Vec<Building>,
}

pub(super) async fn get(
    State(state): State<AppState>,
    Query(params): Query<Params>,
) -> Result<impl IntoResponse, StatusCode> {
    let bounds = Bounds {
        min_x: params.min_x,
        min_z: params.min_z,
        max_x: params.max_x,
        max_z: params.max_z,
    };
    if !bounds.valid(params.buildings && !params.borders) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let regions = RegionFilter::parse(params.regions.as_deref())?;
    let index = state.claim_locations.clone();
    // Keep large border requests off the runtime's async workers.
    let details = tokio::task::spawn_blocking(move || {
        let borders = if params.borders {
            let tiles = index.map_tiles(bounds.with_neighbors(), &regions);
            outlines(&tiles, bounds)
        } else {
            Vec::new()
        };
        let buildings = if params.buildings {
            index.map_buildings(bounds, &regions)
        } else {
            Vec::new()
        };
        Details { borders, buildings }
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=15")],
        Json(details),
    ))
}

fn outlines(tiles: &HashMap<Cell, Option<u64>>, bounds: Bounds) -> Vec<(i32, i32, u8)> {
    let mut borders = Vec::new();
    for (&cell @ (x, z), &claim) in tiles {
        let Some(claim) = claim.filter(|_| bounds.contains(cell)) else {
            continue;
        };
        let odd = z & 1;
        // Same order as the six vertices used by the frontend: E, NE, NW, W, SW, SE.
        let neighbors = [
            (x + 1, z),
            (x + odd, z + 1),
            (x + odd - 1, z + 1),
            (x - 1, z),
            (x + odd - 1, z - 1),
            (x + odd, z - 1),
        ];
        let mask = neighbors
            .into_iter()
            .enumerate()
            .fold(0, |mask, (i, neighbor)| {
                let draw = match tiles.get(&neighbor) {
                    None => true,
                    // A shared edge between two different claims needs one stroke.
                    Some(Some(other)) => {
                        *other != claim && (!bounds.contains(neighbor) || cell < neighbor)
                    }
                    Some(None) => false,
                };
                mask | (u8::from(draw) << i)
            });
        if mask != 0 {
            borders.push((x, z, mask));
        }
    }
    borders
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::get_routes;

    #[test]
    fn masks_shared_hex_edges_and_uses_tiles_just_outside_the_requested_bounds() {
        let bounds = Bounds {
            min_x: 10,
            max_x: 11,
            min_z: 10,
            max_z: 11,
        };
        let tiles = HashMap::from([
            ((10, 10), Some(1)),
            ((10, 11), Some(1)),
            ((11, 10), Some(2)),
            ((9, 10), Some(1)),
        ]);
        let masks = outlines(&tiles, bounds)
            .into_iter()
            .map(|(x, z, mask)| ((x, z), mask))
            .collect::<HashMap<_, _>>();
        assert_eq!(masks[&(10, 10)] & 0b001011, 1);
        assert_eq!(masks[&(11, 10)] & (1 << 3), 0);
        assert_eq!(masks[&(10, 11)] & (1 << 4), 0);
        assert!(!masks.contains_key(&(9, 10)));
    }

    #[test]
    fn only_maps_claim_buildings_in_the_overworld() {
        let mut coords = SmallHexTileMessage {
            x: 10,
            z: 5,
            dimension: 1,
        };
        let bank = Building::from_game(BuildingKind::RegionalBank, 3, &coords).unwrap();
        assert_eq!((bank.x, bank.z), (12, 5));
        assert!(Building::from_game(BuildingKind::Bank, 0, &coords).is_none());
        coords.dimension = 2;
        assert!(Building::from_game(BuildingKind::Waystone, 3, &coords).is_none());
    }

    #[tokio::test]
    async fn endpoint_accepts_only_bounded_viewports() {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        use game_module::module_bindings::{ClaimTileState, LocationState};
        use sea_orm::{DatabaseBackend, MockDatabase};
        use tower::ServiceExt;
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
            tx,
            Default::default(),
        );
        let index = &state.claim_locations;
        index.reset(7);
        index.claims_initial(
            7,
            &[
                ClaimTileState {
                    entity_id: 10,
                    claim_id: 1,
                },
                ClaimTileState {
                    entity_id: 11,
                    claim_id: 1,
                },
            ],
        );
        index.locations_initial(
            7,
            &[
                LocationState {
                    entity_id: 10,
                    chunk_index: 1,
                    x: 5,
                    z: 5,
                    dimension: 1,
                },
                LocationState {
                    entity_id: 11,
                    chunk_index: 1,
                    x: 6,
                    z: 5,
                    dimension: 1,
                },
            ],
        );
        let building = Building::from_game(
            BuildingKind::RegionalBank,
            1,
            &SmallHexTileMessage {
                x: 2,
                z: 5,
                dimension: 1,
            },
        );
        index.set_map_buildings(7, BuildingKind::RegionalBank, [(12, building)]);
        index.finish_initial(7);
        let app = get_routes().with_state(state);
        for (query, status, expected_borders) in [
            (
                "minX=1&minZ=2&maxX=5&maxZ=6&borders=true&buildings=true",
                StatusCode::OK,
                serde_json::json!([[5, 5, 62]]),
            ),
            (
                "minX=0&minZ=0&maxX=12000&maxZ=10000&buildings=true",
                StatusCode::OK,
                serde_json::json!([]),
            ),
            (
                "minX=1&minZ=2&maxX=5&maxZ=6&borders=true&buildings=true&regions=8",
                StatusCode::OK,
                serde_json::json!([]),
            ),
            (
                "minX=0&minZ=0&maxX=12000&maxZ=10000&buildings=true&borders=true",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
            (
                "minX=0&minZ=0&maxX=96001&maxZ=10000&buildings=true",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
            (
                "minX=-1&minZ=0&maxX=5&maxZ=6",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
            (
                "minX=0&minZ=0&maxX=2047&maxZ=2047&borders=true",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/world-map/claim-details?{query}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            if status == StatusCode::OK {
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                    serde_json::json!({
                        "borders": expected_borders,
                        "buildings": if query.contains("regions=8") {
                            serde_json::json!([])
                        } else {
                            serde_json::json!([{"kind":"regionalBank", "x":4, "z":5}])
                        }
                    }),
                );
            }
        }
    }
}
