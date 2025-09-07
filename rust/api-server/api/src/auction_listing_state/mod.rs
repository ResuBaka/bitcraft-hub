pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
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
    Query(_params): Query<BuildingStatesParams>,
) -> Result<axum_codec::Codec<MarketOrdersResponse>, (StatusCode, &'static str)> {
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
