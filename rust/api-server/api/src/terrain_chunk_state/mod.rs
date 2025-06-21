use crate::{AppRouter, AppState};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Router,
};
use entity::terrain_chunk_state;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::sync::Arc;

pub(crate) fn get_routes() -> AppRouter {
    Router::new().route(
        "/api/bitcraft/terrain_chunks",
        axum_codec::routing::get(list_terrain_chunks).into(),
    )
}

#[derive(Debug, Deserialize)]
pub struct ListTerrainChunksParams {
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TerrainChunksResponse {
    pub chunks: Vec<terrain_chunk_state::Model>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

pub async fn list_terrain_chunks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTerrainChunksParams>,
) -> Result<axum_codec::Codec<TerrainChunksResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);

    let (chunks, total) = QueryCore::find_terrain_chunks(&state.conn, page, per_page)
        .await
        .map_err(|err| {
            tracing::error!("Failed to fetch terrain chunks: {:?}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch terrain chunks")
        })?;

    Ok(axum_codec::Codec(TerrainChunksResponse {
        chunks,
        per_page,
        total: total.number_of_items,
        page,
    }))
}
