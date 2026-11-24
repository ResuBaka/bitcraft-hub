use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use serde::Serialize;

#[derive(Serialize, FromQueryResult)]
pub(super) struct Placeable {
    id: String,
    owner_id: String,
    name: String,
    tier: Option<i32>,
    growing: bool,
    ends_at_ms: Option<i64>,
    interactions: Vec<String>,
}

async fn owned(db: &DatabaseConnection, owner: i64) -> Result<Vec<Placeable>, DbErr> {
    Placeable::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "
        SELECT p.entity_id::text AS id, p.owner_entity_id::text AS owner_id,
               COALESCE(d.name, 'Placeable #' || p.placeable_id) AS name, d.tier,
               t.entity_id IS NOT NULL AS growing, t.ends_at_ms,
               ARRAY(SELECT DISTINCT i.verb_phrase FROM placeable_interaction_desc i
                     WHERE i.placeable_id = p.placeable_id AND i.verb_phrase <> ''
                     ORDER BY i.verb_phrase) AS interactions
        FROM placeable_state p
        LEFT JOIN placeable_desc d ON d.id = p.placeable_id
        LEFT JOIN placeable_growth_timer t ON t.entity_id = p.entity_id
        WHERE p.owner_entity_id = $1
        ORDER BY t.ends_at_ms NULLS LAST, name, p.entity_id
    ",
        [owner.into()],
    ))
    .all(db)
    .await
}

pub(super) async fn player_placeables(
    State(state): State<AppState>,
    Path(owner): Path<i64>,
) -> Result<axum_codec::Codec<Vec<Placeable>>, StatusCode> {
    owned(&state.conn, owner)
        .await
        .map(axum_codec::Codec)
        .map_err(|error| {
            tracing::error!(%error, "Could not load owned placeables");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
