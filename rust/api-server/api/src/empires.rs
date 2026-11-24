use crate::{AppRouter, AppState};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_codec::{Codec, routing::get};
use entity::empire::views::{EmpireDetail, EmpireSummary, WatchtowerDetail};
use serde::Deserialize;
use service::Query as QueryCore;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/empires", get(directory).into())
        .route("/empires/{id}", get(detail).into())
        .route("/api/bitcraft/empires", get(directory).into())
        .route("/api/bitcraft/empires/{id}", get(detail).into())
        .route("/api/bitcraft/watchtowers/{id}", get(watchtower).into())
        .route("/watchtowers/{id}", get(watchtower).into())
}

type ApiError = (StatusCode, &'static str);

fn db_error(err: sea_orm::DbErr) -> ApiError {
    tracing::error!(%err, "Could not read empire data");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Could not read empire data",
    )
}

async fn directory(State(state): State<AppState>) -> Result<Codec<Vec<EmpireSummary>>, ApiError> {
    QueryCore::empire_directory(&state.conn)
        .await
        .map(Codec)
        .map_err(db_error)
}

async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Codec<EmpireDetail>, ApiError> {
    QueryCore::empire_detail(&state.conn, id)
        .await
        .map_err(db_error)?
        .map(Codec)
        .ok_or((StatusCode::NOT_FOUND, "Empire not found"))
}

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<i64>,
}

async fn watchtower(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<HistoryQuery>,
) -> Result<Codec<WatchtowerDetail>, ApiError> {
    QueryCore::watchtower_detail(&state.conn, id, query.before)
        .await
        .map_err(db_error)?
        .map(Codec)
        .ok_or((StatusCode::NOT_FOUND, "Watchtower not found"))
}
