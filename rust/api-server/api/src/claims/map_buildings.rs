use super::map_details::Bounds;
use crate::AppState;
use crate::world_map::RegionFilter;
use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use entity::building_desc::Function;
use sea_orm::{DbBackend, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAX_MARKERS: usize = 2_500;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Group {
    Crafting,
    Refining,
    Trading,
    Housing,
    Storage,
    Other,
}

fn building_metadata(functions: &[Function]) -> (Group, Option<i32>) {
    use Group::*;
    // A building belongs to one group. Prefer its specialized use over
    // storage, which is also present on many stations.
    let group = if functions.iter().any(|f| f.crafting_slots > 0) {
        Crafting
    } else if functions
        .iter()
        .any(|f| f.refining_slots > 0 || f.refining_cargo_slots > 0)
    {
        Refining
    } else if functions.iter().any(|f| f.trade_orders > 0) {
        Trading
    } else if functions.iter().any(|f| f.housing_slots > 0) {
        Housing
    } else if functions
        .iter()
        .any(|f| f.storage_slots > 0 || f.cargo_slots > 0)
    {
        Storage
    } else {
        Other
    };
    let tier = functions
        .iter()
        .map(|f| f.level)
        .filter(|&level| level > 0)
        .max();
    (group, tier)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Params {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    groups: String,
    regions: Option<String>,
}

#[derive(FromQueryResult)]
struct Row {
    building_description_id: i32,
    x: i64,
    z: i64,
}

#[derive(Serialize)]
struct Marker {
    group: Group,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<i32>,
    x: i64,
    z: i64,
}

#[derive(Serialize)]
struct Buildings {
    markers: Vec<Marker>,
    truncated: bool,
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
    if !bounds.valid(false) || params.groups.len() > 64 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let regions = RegionFilter::parse(params.regions.as_deref())?;
    use serde::de::value::{Error, StrDeserializer};
    let groups: HashSet<Group> = params
        .groups
        .split(',')
        .map(|group| Group::deserialize(StrDeserializer::<Error>::new(group)))
        .collect::<Result<_, _>>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let metadata: HashMap<_, _> = state
        .building_desc
        .iter()
        .filter_map(|desc| {
            let id = i32::try_from(*desc.key()).ok()?;
            let (group, tier) = building_metadata(&desc.functions);
            groups.contains(&group).then_some((id, (group, tier)))
        })
        .collect();

    let rows = if metadata.is_empty() || regions.ids().is_some_and(|ids| ids.is_empty()) {
        Vec::new()
    } else {
        // location_state stores overworld offset cells, like the claim border index.
        Row::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT b.building_description_id, l.x, l.z
             FROM location_state l JOIN building_state b
               ON b.entity_id = l.entity_id AND b.region = l.region
             WHERE l.dimension = 1 AND l.x BETWEEN $1 AND $2 AND l.z BETWEEN $3 AND $4
                AND b.building_description_id = ANY($5::integer[])
                AND (cardinality($7::smallint[]) = 0 OR l.region = ANY($7::smallint[]))
             ORDER BY l.x, l.z, l.entity_id LIMIT $6",
            [
                i64::from(bounds.min_x).into(),
                i64::from(bounds.max_x).into(),
                i64::from(bounds.min_z).into(),
                i64::from(bounds.max_z).into(),
                metadata.keys().copied().collect::<Vec<_>>().into(),
                ((MAX_MARKERS + 1) as i64).into(),
                regions.ids().unwrap_or_default().to_vec().into(),
            ],
        ))
        .all(&state.conn)
        .await
        .map_err(|err| {
            tracing::warn!(%err, "Map building lookup failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };
    let truncated = rows.len() > MAX_MARKERS;
    let markers = rows
        .into_iter()
        .take(MAX_MARKERS)
        .map(|row| {
            let (group, tier) = metadata[&row.building_description_id];
            Marker {
                group,
                tier,
                x: row.x,
                z: row.z,
            }
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=15")],
        Json(Buildings { markers, truncated }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(level: i32, crafting_slots: i32, storage_slots: i32) -> Function {
        Function {
            function_type: 0,
            level,
            crafting_slots,
            storage_slots,
            cargo_slots: 0,
            refining_slots: 0,
            refining_cargo_slots: 0,
            item_slot_size: 0,
            cargo_slot_size: 0,
            trade_orders: 0,
            allowed_item_id_per_slot: vec![],
            concurrent_crafts_per_player: 0,
            terraform: false,
            housing_slots: 0,
            housing_income: 0,
        }
    }

    #[test]
    fn classifies_by_capability_and_uses_highest_positive_level() {
        assert_eq!(
            building_metadata(&[function(2, 0, 1), function(5, 1, 1)]),
            (Group::Crafting, Some(5))
        );
        assert_eq!(
            building_metadata(&[function(0, 0, 1)]),
            (Group::Storage, None)
        );
        assert_eq!(building_metadata(&[]), (Group::Other, None));
    }

    #[tokio::test]
    async fn building_route_validates_viewports_and_returns_group_and_tier() {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        use entity::building_desc::{BuildingInteractionLevel::All, Model};
        use sea_orm::{MockDatabase, Value};
        use std::collections::BTreeMap;
        use tower::ServiceExt;

        let conn = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([[BTreeMap::from([
                ("building_description_id", Value::from(17_i32)),
                ("x", Value::from(13_i64)),
                ("z", Value::from(11_i64)),
            ])]])
            .into_connection();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(conn, tx, Default::default());
        state.building_desc.insert(
            17,
            Model {
                id: 17,
                functions: vec![function(5, 1, 0)],
                name: "Test Workshop".into(),
                description: String::new(),
                rested_buff_duration: 0,
                light_radius: 0,
                model_asset_name: String::new(),
                icon_asset_name: String::new(),
                unenterable: false,
                wilderness: false,
                footprint: vec![],
                max_health: 0,
                ignore_damage: false,
                defense_level: 0,
                decay: 0.,
                maintenance: 0.,
                build_permission: All,
                interact_permission: All,
                has_action: true,
                show_in_compendium: true,
                is_ruins: false,
                not_deconstructible: false,
                destroy_on_unclaim: false,
            },
        );
        let app = super::super::get_routes().with_state(state);
        for (max_x, group, status, markers) in [
            (
                9000,
                "crafting",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
            (
                25,
                "unknown",
                StatusCode::BAD_REQUEST,
                serde_json::Value::Null,
            ),
            (25, "storage", StatusCode::OK, serde_json::json!([])),
            (
                25,
                "crafting",
                StatusCode::OK,
                serde_json::json!([{"group":"crafting","tier":5,"x":13,"z":11}]),
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!(
                            "/world-map/buildings?minX=0&minZ=0&maxX={max_x}&maxZ=25&groups={group}"
                        ))
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
                    serde_json::json!({"markers": markers, "truncated": false})
                );
            }
        }
    }
}
