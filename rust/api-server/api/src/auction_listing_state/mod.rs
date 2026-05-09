pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::inventory::ItemType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/market",
            axum_codec::routing::get(get_market_order_stats).into(),
        )
        .route(
            "/market/orders",
            axum_codec::routing::get(find_market_place_order).into(),
        )
        .route(
            "/market/item_cargo_desc",
            axum_codec::routing::get(market_item_cargo_desc).into(),
        )
}

#[derive(Debug, Deserialize)]
pub(crate) struct MarketOrdersParams {
    items: Option<String>,
    return_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub(crate) struct MarketOrderStats {
    buy: u64,
    sell: u64,
    total: u64,
    buy_price_highest: Option<i32>,
    buy_price_lowest: Option<i32>,
    buy_amount_lowest: Option<i32>,
    buy_amount_highest: Option<i32>,
    sell_price_highest: Option<i32>,
    sell_price_lowest: Option<i32>,
    sell_amount_lowest: Option<i32>,
    sell_amount_highest: Option<i32>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct MarketOrderStatsResponse {
    order_counts: HashMap<String, MarketOrderStats>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct MarketOrdersResponse {
    buy_orders: HashMap<String, Vec<entity::auction_listing_state::AuctionListingState>>,
    sell_orders: HashMap<String, Vec<entity::auction_listing_state::AuctionListingState>>,
}

fn parse_selected_item_keys(items: Option<&str>) -> HashSet<String> {
    items
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) async fn get_market_order_stats(
    state: State<AppState>,
) -> Result<axum_codec::Codec<MarketOrderStatsResponse>, (StatusCode, &'static str)> {
    let mut order_counts = HashMap::<String, MarketOrderStats>::new();

    for order in state.buy_order_state.iter() {
        let key = format!("{}:{}", order.item_type, order.item_id);
        let entry = order_counts.entry(key).or_default();
        entry.buy += 1;
        entry.total += 1;

        if let Some(buy_price_highest) = entry.buy_price_highest {
            if order.price_threshold > buy_price_highest {
                entry.buy_price_highest = Some(order.price_threshold);
                entry.buy_amount_highest = Some(order.quantity);
            }
        } else {
            entry.buy_price_highest = Some(order.price_threshold);
            entry.buy_amount_highest = Some(order.quantity);
        }

        if let Some(buy_price_lowest) = entry.buy_price_lowest {
            if order.price_threshold < buy_price_lowest {
                entry.buy_price_lowest = Some(order.price_threshold);
                entry.buy_amount_lowest = Some(order.quantity);
            }
        } else {
            entry.buy_price_lowest = Some(order.price_threshold);
            entry.buy_amount_lowest = Some(order.quantity);
        }
    }

    for order in state.sell_order_state.iter() {
        let key = format!("{}:{}", order.item_type, order.item_id);
        let entry = order_counts.entry(key).or_default();
        entry.sell += 1;
        entry.total += 1;

        if let Some(sell_price_highest) = entry.sell_price_highest {
            if order.price_threshold > sell_price_highest {
                entry.sell_price_highest = Some(order.price_threshold);
                entry.sell_amount_highest = Some(order.quantity);
            }
        } else {
            entry.sell_price_highest = Some(order.price_threshold);
            entry.sell_amount_highest = Some(order.quantity);
        }

        if let Some(sell_price_lowest) = entry.sell_price_lowest {
            if order.price_threshold < sell_price_lowest {
                entry.sell_price_lowest = Some(order.price_threshold);
                entry.sell_amount_lowest = Some(order.quantity);
            }
        } else {
            entry.sell_price_lowest = Some(order.price_threshold);
            entry.sell_amount_lowest = Some(order.quantity);
        }
    }

    Ok(axum_codec::Codec(MarketOrderStatsResponse { order_counts }))
}

pub(crate) async fn find_market_place_order(
    state: State<AppState>,
    Query(params): Query<MarketOrdersParams>,
) -> Result<axum_codec::Codec<MarketOrdersResponse>, (StatusCode, &'static str)> {
    let selected_item_keys = parse_selected_item_keys(params.items.as_deref());

    if let Some(item) = params.return_all {
        if item {
            return Ok(axum_codec::Codec(MarketOrdersResponse {
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
            }));
        }
    }

    if selected_item_keys.is_empty() {
        return Ok(axum_codec::Codec(MarketOrdersResponse {
            buy_orders: HashMap::new(),
            sell_orders: HashMap::new(),
        }));
    }

    Ok(axum_codec::Codec(MarketOrdersResponse {
        buy_orders: state
            .buy_order_state
            .iter()
            .fold(HashMap::new(), |mut acc, a| {
                let key = format!("{}:{}", a.item_type, a.item_id);
                if selected_item_keys.contains(&key) {
                    acc.entry(key).or_default().push(a.clone());
                }
                acc
            }),
        sell_orders: state
            .sell_order_state
            .iter()
            .fold(HashMap::new(), |mut acc, a| {
                let key = format!("{}:{}", a.item_type, a.item_id);
                if selected_item_keys.contains(&key) {
                    acc.entry(key).or_default().push(a.clone());
                }
                acc
            }),
    }))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
pub struct ItemOption {
    pub label: String,
    pub name: String,
    pub key: String,
    pub id: i32,
    pub item_type: ItemType,
    pub tag: String,
    pub icon_asset_name: String,
    pub tier: i32,
    pub rarity: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub(crate) struct MarketItemCargoDescResponse {
    pub items_grouped: HashMap<String, Vec<ItemOption>>,
    pub item_name_by_key: HashMap<String, String>,
}

pub(crate) async fn market_item_cargo_desc(
    state: State<AppState>,
) -> Result<axum_codec::Codec<MarketItemCargoDescResponse>, (StatusCode, &'static str)> {
    let mut items_grouped = HashMap::new();
    let mut item_name_by_key = HashMap::new();

    for item_desc in state.item_desc.iter() {
        if item_desc.item_list_id > 0 {
            continue;
        }

        let groupe = items_grouped
            .entry(item_desc.tag.clone())
            .or_insert(Vec::new());

        groupe.push(ItemOption {
            label: format!(
                "{} - {} - T{}",
                item_desc.name, item_desc.rarity, item_desc.tier
            ),
            name: item_desc.name.clone(),
            key: format!("0:{}", item_desc.id),
            id: item_desc.id,
            item_type: ItemType::Item,
            tag: item_desc.tag.clone(),
            icon_asset_name: item_desc.icon_asset_name.clone(),
            tier: item_desc.tier,
            rarity: item_desc.rarity.to_string(),
        });
        item_name_by_key.insert(format!("0:{}", item_desc.id), item_desc.name.clone());
    }

    for cargo_desc in state.cargo_desc.iter() {
        let groupe = items_grouped
            .entry(cargo_desc.tag.clone())
            .or_insert(Vec::new());

        groupe.push(ItemOption {
            label: format!(
                "{} - {} - T{}",
                cargo_desc.name, cargo_desc.rarity, cargo_desc.tier
            ),
            name: cargo_desc.name.clone(),
            key: format!("1:{}", cargo_desc.id),
            id: cargo_desc.id,
            item_type: ItemType::Cargo,
            tag: cargo_desc.tag.clone(),
            icon_asset_name: cargo_desc.icon_asset_name.clone(),
            tier: cargo_desc.tier,
            rarity: cargo_desc.rarity.to_string(),
        });
        item_name_by_key.insert(format!("1:{}", cargo_desc.id), cargo_desc.name.clone());
    }

    Ok(axum_codec::Codec::<MarketItemCargoDescResponse>(
        MarketItemCargoDescResponse {
            items_grouped,
            item_name_by_key,
        },
    ))
}
