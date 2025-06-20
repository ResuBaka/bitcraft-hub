use axum::{extract::{Path, Query, State}, Router};
use entity::deployable_desc;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use service::Query as QueryCore;
use crate::{AppRouter, AppState};



pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/deployable_desc",
            axum_codec::routing::get(list_deployable_descs).into(),
        )
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeployableDescsResponse {
    pub deployables: Vec<deployable_desc::Model>,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    pub total: u64,
    pub page: u64,
}

#[derive(Deserialize)]
pub struct ListDeployableDescParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
}

pub async fn list_deployable_descs(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<ListDeployableDescParams>,
) -> Result<axum_codec::Codec<DeployableDescsResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (deployables, num_pages) =
        QueryCore::find_deployable_descs(&state.conn, page, posts_per_page, search)
            .await
            .expect("Cannot find player_state in page");

    Ok(axum_codec::Codec(DeployableDescsResponse {
        deployables: deployables,
        per_page: posts_per_page,
        total: num_pages.number_of_items,
        page,
    }))
}