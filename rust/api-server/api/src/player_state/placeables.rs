use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use serde::Serialize;

#[derive(serde::Deserialize)]
pub(super) struct SummaryQuery {
    #[serde(default)]
    farming: bool,
}

#[derive(Serialize, FromQueryResult)]
pub(super) struct Stage {
    owner_id: String,
    outcome_id: i32,
    outcome_name: String,
    outcome_tier: Option<i32>,
    stage_id: i32,
    name: String,
    quantity: i64,
    ready: bool,
    growing: bool,
    ends_at_ms: Option<i64>,
    interactions: Vec<String>,
}

// Collection can reset a trap/net to its empty state. It is a reward action,
// not a step towards producing the *next* collection in the current cycle.
const COLLECTION: &[&str] = &[
    "Harvest",
    "Chop",
    "Fish",
    "Mine",
    "Gather",
    "Collect",
    "Chisel Bricks",
    "Brush",
];

pub(super) async fn summary(
    State(state): State<AppState>,
    Path(owner): Path<i64>,
    Query(query): Query<SummaryQuery>,
) -> Result<axum_codec::Codec<Vec<Stage>>, StatusCode> {
    Stage::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        include_str!("placeable_summary.sql"),
        [
            owner.into(),
            query.farming.into(),
            COLLECTION
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .into(),
        ],
    ))
    .all(&state.conn)
    .await
    .map(axum_codec::Codec)
    .map_err(|error| {
        tracing::error!(%error, "Could not load placeable outcome groups");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(Serialize, FromQueryResult)]
pub(super) struct Placeable {
    id: String,
    owner_id: String,
    name: String,
    tier: Option<i32>,
    growing: bool,
    ends_at_ms: Option<i64>,
    interactions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantity: Option<i64>,
}

async fn owned(
    db: &DatabaseConnection,
    owner: i64,
    farming: bool,
) -> Result<Vec<Placeable>, DbErr> {
    // Crops and rice are owner-scoped placeables. One descriptor identifies a
    // specific stage, including what can be done to it before the next timer.
    let listing = if farming {
        "SELECT d.id::text || ':' || (t.entity_id IS NOT NULL)::text AS id,
                MIN(p.entity_id) AS sort_id, p.owner_entity_id, d.id AS placeable_id, d.name, d.tier,
                t.entity_id IS NOT NULL AS growing, MIN(t.ends_at_ms) AS ends_at_ms, COUNT(*) AS quantity
         FROM placeable_state p JOIN placeable_desc d ON d.id = p.placeable_id
         LEFT JOIN placeable_growth_timer t ON t.entity_id = p.entity_id
         WHERE p.owner_entity_id = $1 AND d.tag IN ('Farmed Crop', 'Rice Farming')
         GROUP BY p.owner_entity_id, d.id, (t.entity_id IS NOT NULL)"
    } else {
        "SELECT p.entity_id::text AS id, p.entity_id AS sort_id, p.owner_entity_id, p.placeable_id,
                COALESCE(d.name, 'Placeable #' || p.placeable_id) AS name, d.tier,
                t.entity_id IS NOT NULL AS growing, t.ends_at_ms, NULL::bigint AS quantity
         FROM placeable_state p LEFT JOIN placeable_desc d ON d.id = p.placeable_id
         LEFT JOIN placeable_growth_timer t ON t.entity_id = p.entity_id
         WHERE p.owner_entity_id = $1"
    };
    Placeable::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!("
        SELECT p.id, p.owner_entity_id::text AS owner_id, p.name, p.tier, p.growing, p.ends_at_ms, p.quantity,
               ARRAY(SELECT DISTINCT i.verb_phrase FROM placeable_interaction_desc i
                     WHERE i.placeable_id = p.placeable_id AND i.verb_phrase <> ''
                     ORDER BY i.verb_phrase) AS interactions
        FROM ({listing}) p
        ORDER BY p.ends_at_ms NULLS LAST, p.name, p.sort_id
    "),
        [owner.into()],
    ))
    .all(db)
    .await
}

pub(super) async fn player_placeables(
    State(state): State<AppState>,
    Path(owner): Path<i64>,
) -> Result<axum_codec::Codec<Vec<Placeable>>, StatusCode> {
    owned(&state.conn, owner, false)
        .await
        .map(axum_codec::Codec)
        .map_err(|error| {
            tracing::error!(%error, "Could not load owned placeables");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub(super) async fn player_farming(
    State(state): State<AppState>,
    Path(owner): Path<i64>,
) -> Result<axum_codec::Codec<Vec<Placeable>>, StatusCode> {
    owned(&state.conn, owner, true)
        .await
        .map(axum_codec::Codec)
        .map_err(|error| {
            tracing::error!(%error, "Could not load owned crops");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
