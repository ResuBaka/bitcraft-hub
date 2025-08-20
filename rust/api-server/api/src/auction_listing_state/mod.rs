pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::building_state;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use std::collections::HashMap;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new().route(
        "/market",
        axum_codec::routing::get(find_market_place_order).into(),
    )
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct MarketOrdersResponse {
    buy_orders: HashMap<String, Vec<entity::auction_listing_state::AuctionListingState>>,
    sell_orders: HashMap<String, Vec<entity::auction_listing_state::AuctionListingState>>,
}

#[derive(Deserialize)]
pub(crate) struct BuildingStatesParams {
    page: Option<u64>,
    per_page: Option<u64>,
    item_id: Option<i64>,
    item_type: Option<String>,
}

pub(crate) async fn find_market_place_order(
    state: State<AppState>,
    Query(params): Query<BuildingStatesParams>,
) -> Result<axum_codec::Codec<MarketOrdersResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);
    let search = params.item_id;
    let item_type = params.item_type;

    Ok(axum_codec::Codec(MarketOrdersResponse {
        buy_orders: state
            .buy_order_state
            .iter()
            .fold(HashMap::new(), |mut acc, a| {
                let key = format!("{}:{}", a.item_type, a.item_id);
                acc.entry(key).or_default().push(a.clone());
                acc
            }),
        sell_orders: state
            .sell_order_state
            .iter()
            .fold(HashMap::new(), |mut acc, a| {
                let key = format!("{}:{}", a.item_type, a.item_id);
                acc.entry(key).or_default().push(a.clone());
                acc
            }),
    }))
}

pub(crate) async fn find_building_state(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<building_state::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_state_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingState not found"));
    }

    Ok(axum_codec::Codec(posts.unwrap()))
}
