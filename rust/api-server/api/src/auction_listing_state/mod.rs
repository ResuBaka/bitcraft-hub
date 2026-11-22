pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::Path;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::auction_listing_state::AuctionListingState;
use entity::inventory::ItemType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

pub(crate) type OrderMap = scc::HashMap<i64, AuctionListingState>;

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
        .route(
            "/market/claim/{claim_id}",
            axum_codec::routing::get(get_market_order_stats_by_claim).into(),
        )
        .route(
            "/market/orders/claim/{claim_id}",
            axum_codec::routing::get(find_market_place_order_by_claim).into(),
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

async fn scan_orders(
    orders: &OrderMap,
    claim_id: Option<u64>,
    mut visit: impl FnMut(&AuctionListingState),
) {
    // A concurrent SCC resize can revisit keys. Count/return each order at
    // most once, without implying a point-in-time snapshot.
    let mut seen = HashSet::new();
    orders
        .iter_async(|key, order| {
            if claim_id.is_none_or(|id| order.claim_entity_id == id) && seen.insert(*key) {
                visit(order);
            }
            true
        })
        .await;
}

pub(crate) async fn orders_in_claim(orders: &OrderMap, claim_id: u64) -> Vec<AuctionListingState> {
    let mut selected = Vec::new();
    scan_orders(orders, Some(claim_id), |order| selected.push(order.clone())).await;
    selected
}

impl MarketOrderStats {
    fn record(&mut self, order: &AuctionListingState, buy: bool) {
        self.total += 1;
        let (count, high_price, high_amount, low_price, low_amount) = if buy {
            (
                &mut self.buy,
                &mut self.buy_price_highest,
                &mut self.buy_amount_highest,
                &mut self.buy_price_lowest,
                &mut self.buy_amount_lowest,
            )
        } else {
            (
                &mut self.sell,
                &mut self.sell_price_highest,
                &mut self.sell_amount_highest,
                &mut self.sell_price_lowest,
                &mut self.sell_amount_lowest,
            )
        };
        *count += 1;
        if high_price.is_none_or(|price| order.price_threshold > price) {
            *high_price = Some(order.price_threshold);
            *high_amount = Some(order.quantity);
        }
        if low_price.is_none_or(|price| order.price_threshold < price) {
            *low_price = Some(order.price_threshold);
            *low_amount = Some(order.quantity);
        }
    }
}

async fn collect_market_order_stats(
    buy_orders: &OrderMap,
    sell_orders: &OrderMap,
    claim_id: Option<u64>,
) -> MarketOrderStatsResponse {
    let mut order_counts = HashMap::<String, MarketOrderStats>::new();
    for (orders, buy) in [(buy_orders, true), (sell_orders, false)] {
        scan_orders(orders, claim_id, |order| {
            order_counts
                .entry(format!("{}:{}", order.item_type, order.item_id))
                .or_default()
                .record(order, buy);
        })
        .await;
    }
    MarketOrderStatsResponse { order_counts }
}

async fn collect_market_orders(
    buy: &OrderMap,
    sell: &OrderMap,
    claim_id: Option<u64>,
    params: MarketOrdersParams,
) -> MarketOrdersResponse {
    let selected_item_keys = (!params.return_all.unwrap_or(false))
        .then(|| parse_selected_item_keys(params.items.as_deref()));
    let mut buy_orders = HashMap::<String, Vec<_>>::new();
    let mut sell_orders = HashMap::<String, Vec<_>>::new();
    if !selected_item_keys.as_ref().is_some_and(HashSet::is_empty) {
        for (orders, grouped) in [(buy, &mut buy_orders), (sell, &mut sell_orders)] {
            scan_orders(orders, claim_id, |order| {
                let key = format!("{}:{}", order.item_type, order.item_id);
                if selected_item_keys
                    .as_ref()
                    .is_none_or(|keys| keys.contains(&key))
                {
                    grouped.entry(key).or_default().push(order.clone());
                }
            })
            .await;
        }
    }
    MarketOrdersResponse {
        buy_orders,
        sell_orders,
    }
}

pub(crate) async fn get_market_order_stats(
    state: State<AppState>,
) -> Result<axum_codec::Codec<MarketOrderStatsResponse>, (StatusCode, &'static str)> {
    Ok(axum_codec::Codec(
        collect_market_order_stats(&state.buy_order_state, &state.sell_order_state, None).await,
    ))
}

pub(crate) async fn get_market_order_stats_by_claim(
    state: State<AppState>,
    Path(claim_id): Path<u64>,
) -> Result<axum_codec::Codec<MarketOrderStatsResponse>, (StatusCode, &'static str)> {
    let stats = collect_market_order_stats(
        &state.buy_order_state,
        &state.sell_order_state,
        Some(claim_id),
    )
    .await;
    Ok(axum_codec::Codec(stats))
}

pub(crate) async fn find_market_place_order(
    state: State<AppState>,
    Query(params): Query<MarketOrdersParams>,
) -> Result<axum_codec::Codec<MarketOrdersResponse>, (StatusCode, &'static str)> {
    let orders = collect_market_orders(
        &state.buy_order_state,
        &state.sell_order_state,
        None,
        params,
    )
    .await;
    Ok(axum_codec::Codec(orders))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(
        id: u64,
        claim: u64,
        item_type: i32,
        price: i32,
        quantity: i32,
    ) -> AuctionListingState {
        AuctionListingState {
            entity_id: id,
            owner_entity_id: 1,
            claim_entity_id: claim,
            item_id: 42,
            item_type,
            price_threshold: price,
            quantity,
            timestamp: entity::shared::timestamp::Timestamp {
                __timestamp_micros_since_unix_epoch__: chrono::DateTime::UNIX_EPOCH,
            },
            stored_coins: 0,
            region: 1,
        }
    }

    #[tokio::test]
    async fn scc_market_stats_follow_replacements_removals_and_claim_filters() {
        let buy = OrderMap::new();
        let sell = OrderMap::new();
        buy.upsert_async(1, order(1, 10, 0, 25, 2)).await;
        buy.upsert_async(1, order(1, 10, 0, 15, 3)).await;
        buy.upsert_async(2, order(2, 10, 0, 20, 4)).await;
        buy.upsert_async(3, order(3, 11, 0, 100, 9)).await;
        sell.upsert_async(4, order(4, 10, 0, 30, 5)).await;
        sell.upsert_async(5, order(5, 10, 0, 35, 6)).await;
        sell.remove_async(&5).await;

        let global = collect_market_order_stats(&buy, &sell, None).await;
        assert_eq!(global.order_counts["0:42"].buy_price_highest, Some(100));
        let local = collect_market_order_stats(&buy, &sell, Some(10)).await;
        let entry = &local.order_counts["0:42"];
        assert_eq!((entry.buy, entry.sell, entry.total), (2, 1, 3));
        assert_eq!(entry.buy_price_lowest, Some(15));
        assert_eq!(entry.buy_amount_lowest, Some(3));
        assert_eq!(entry.buy_price_highest, Some(20));
        assert_eq!(entry.buy_amount_highest, Some(4));
        assert_eq!(entry.sell_price_highest, Some(30));
        assert_eq!(entry.sell_amount_highest, Some(5));
        assert_eq!(orders_in_claim(&sell, 10).await, [order(4, 10, 0, 30, 5)]);
    }

    #[tokio::test]
    async fn scc_market_orders_filter_by_claim_and_typed_item_key() {
        let buy = OrderMap::new();
        let sell = OrderMap::new();
        let selected_order = order(1, 10, 0, 15, 3);
        buy.upsert_async(1, selected_order.clone()).await;
        buy.upsert_async(2, order(2, 11, 0, 20, 4)).await;
        sell.upsert_async(3, order(3, 10, 1, 30, 5)).await;

        let selected = collect_market_orders(
            &buy,
            &sell,
            Some(10),
            MarketOrdersParams {
                items: Some(" 0:42, ".into()),
                return_all: None,
            },
        )
        .await;
        assert_eq!(selected.buy_orders["0:42"], [selected_order]);
        assert!(selected.sell_orders.is_empty());

        let all = collect_market_orders(
            &buy,
            &sell,
            Some(10),
            MarketOrdersParams {
                items: None,
                return_all: Some(true),
            },
        )
        .await;
        assert_eq!(all.buy_orders["0:42"].len(), 1);
        assert_eq!(all.sell_orders["1:42"].len(), 1);

        let empty = collect_market_orders(
            &buy,
            &sell,
            None,
            MarketOrdersParams {
                items: None,
                return_all: None,
            },
        )
        .await;
        assert!(empty.buy_orders.is_empty() && empty.sell_orders.is_empty());
    }
}

pub(crate) async fn find_market_place_order_by_claim(
    state: State<AppState>,
    Path(claim_id): Path<u64>,
    Query(params): Query<MarketOrdersParams>,
) -> Result<axum_codec::Codec<MarketOrdersResponse>, (StatusCode, &'static str)> {
    Ok(axum_codec::Codec(
        collect_market_orders(
            &state.buy_order_state,
            &state.sell_order_state,
            Some(claim_id),
            params,
        )
        .await,
    ))
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
