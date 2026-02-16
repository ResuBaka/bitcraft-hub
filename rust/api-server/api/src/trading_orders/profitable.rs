use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::inventory::ItemType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

/// Default pocket volume used for stack size calculation.
const DEFAULT_POCKET_VOLUME: i32 = 6000;

/// Parameters for teleportation energy cost calculation.
/// TODO: Replace with live values from AppState once ParametersDesc is synced.
const TELEPORTATION_BASE_ENERGY_COST: f64 = 50.0;
const TELEPORTATION_COST_PER_LARGE_TILE: f64 = 1.0;
const TELEPORTATION_FULL_INVENTORY_MULTIPLIER: f64 = 2.0;

/// Default number of inventory pockets a player has.
const DEFAULT_NUM_INVENTORY_POCKETS: i32 = 20;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/api/bitcraft/trading-orders/profitable",
            axum_codec::routing::get(get_profitable_trades).into(),
        )
        .route(
            "/api/bitcraft/trading-orders/profitable/debug",
            axum::routing::get(get_profitable_trades_debug),
        )
}

// ─── Debug endpoint ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct DebugResponse {
    total_orders: usize,
    location_state_count: usize,
    traveler_orders_skipped: usize,
    cargo_orders_skipped: usize,
    zero_stock_skipped: usize,
    eligible_orders: usize,
    /// Unique item IDs found in offer_items across all eligible orders.
    unique_offered_item_ids: Vec<i32>,
    /// Unique item IDs found in required_items across all eligible orders.
    unique_required_item_ids: Vec<i32>,
    /// Item IDs that appear in BOTH offer and required positions (these are matchable).
    overlapping_item_ids: Vec<i32>,
    item_matches_found: usize,
    location_misses: usize,
    sample_orders: Vec<DebugOrder>,
}

#[derive(Serialize, Deserialize)]
struct DebugOrder {
    entity_id: i64,
    shop_entity_id: i64,
    remaining_stock: i32,
    has_traveler_id: bool,
    offer_cargo_ids: Vec<i32>,
    required_cargo_ids: Vec<i32>,
    offer_items: Vec<DebugItem>,
    required_items: Vec<DebugItem>,
    has_location: bool,
}

#[derive(Serialize, Deserialize)]
struct DebugItem {
    item_id: i32,
    quantity: i32,
    item_type: String,
}

async fn get_profitable_trades_debug(
    state: State<AppState>,
) -> Result<axum::Json<DebugResponse>, (StatusCode, &'static str)> {
    let orders: Vec<entity::trade_order::Model> = state
        .trade_order_state
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    let total_orders = orders.len();
    let mut traveler_orders_skipped = 0;
    let mut cargo_orders_skipped = 0;
    let mut zero_stock_skipped = 0;

    let mut eligible: Vec<&entity::trade_order::Model> = Vec::new();
    let mut offered_ids: HashSet<i32> = HashSet::new();
    let mut required_ids: HashSet<i32> = HashSet::new();

    for order in &orders {
        if order.traveler_trade_order_id.is_some() {
            traveler_orders_skipped += 1;
            continue;
        }
        if !order.offer_cargo_id.is_empty() || !order.required_cargo_id.is_empty() {
            cargo_orders_skipped += 1;
            continue;
        }
        if order.remaining_stock <= 0 {
            zero_stock_skipped += 1;
            continue;
        }
        for item in &order.offer_items {
            if item.item_type == ItemType::Item {
                offered_ids.insert(item.item_id);
            }
        }
        for item in &order.required_items {
            if item.item_type == ItemType::Item {
                required_ids.insert(item.item_id);
            }
        }
        eligible.push(order);
    }

    let overlapping: HashSet<i32> = offered_ids.intersection(&required_ids).copied().collect();

    // Count item matches and location misses
    let mut item_matches_found = 0;
    let mut location_misses = 0;

    for i in 0..eligible.len() {
        for j in (i + 1)..eligible.len() {
            let a = eligible[i];
            let b = eligible[j];

            // A offers -> B requires
            for offer in &a.offer_items {
                for req in &b.required_items {
                    if offer.item_id == req.item_id && offer.item_type == req.item_type {
                        item_matches_found += 1;
                        if state.location_state.get(&a.shop_entity_id).is_none()
                            || state.location_state.get(&b.shop_entity_id).is_none()
                        {
                            location_misses += 1;
                        }
                    }
                }
            }
            // B offers -> A requires
            for offer in &b.offer_items {
                for req in &a.required_items {
                    if offer.item_id == req.item_id && offer.item_type == req.item_type {
                        item_matches_found += 1;
                        if state.location_state.get(&a.shop_entity_id).is_none()
                            || state.location_state.get(&b.shop_entity_id).is_none()
                        {
                            location_misses += 1;
                        }
                    }
                }
            }
        }
    }

    let sample_orders: Vec<DebugOrder> = orders
        .iter()
        .take(10)
        .map(|o| DebugOrder {
            entity_id: o.entity_id,
            shop_entity_id: o.shop_entity_id,
            remaining_stock: o.remaining_stock,
            has_traveler_id: o.traveler_trade_order_id.is_some(),
            offer_cargo_ids: o.offer_cargo_id.clone(),
            required_cargo_ids: o.required_cargo_id.clone(),
            offer_items: o
                .offer_items
                .iter()
                .map(|i| DebugItem {
                    item_id: i.item_id,
                    quantity: i.quantity,
                    item_type: format!("{:?}", i.item_type),
                })
                .collect(),
            required_items: o
                .required_items
                .iter()
                .map(|i| DebugItem {
                    item_id: i.item_id,
                    quantity: i.quantity,
                    item_type: format!("{:?}", i.item_type),
                })
                .collect(),
            has_location: state.location_state.get(&o.shop_entity_id).is_some(),
        })
        .collect();

    let mut offered_vec: Vec<i32> = offered_ids.into_iter().collect();
    offered_vec.sort();
    let mut required_vec: Vec<i32> = required_ids.into_iter().collect();
    required_vec.sort();
    let mut overlap_vec: Vec<i32> = overlapping.into_iter().collect();
    overlap_vec.sort();

    Ok(axum::Json(DebugResponse {
        total_orders,
        location_state_count: state.location_state.len(),
        traveler_orders_skipped,
        cargo_orders_skipped,
        zero_stock_skipped,
        eligible_orders: eligible.len(),
        unique_offered_item_ids: offered_vec,
        unique_required_item_ids: required_vec,
        overlapping_item_ids: overlap_vec,
        item_matches_found,
        location_misses,
        sample_orders,
    }))
}

// ─── Profitable trades endpoint ──────────────────────────────────────────────

#[derive(Deserialize)]
struct PaginationParams {
    page: Option<u64>,
    per_page: Option<u64>,
}

/// A single profitable trade opportunity between two barter stall orders.
#[derive(Serialize, Deserialize, TS, Clone, Debug)]
#[ts(export)]
pub struct ProfitableTrade {
    /// The order you buy FROM (this order offers the item you want).
    pub sell_order_entity_id: i64,
    pub sell_shop_entity_id: i64,

    /// The order you sell TO (this order requires the item you have).
    pub buy_order_entity_id: i64,
    pub buy_shop_entity_id: i64,

    /// The item being traded.
    pub item_id: i32,
    pub item_name: String,

    /// How many of the item the sell order offers per trade.
    pub sell_quantity_per_trade: i32,
    /// What the sell order requires in exchange (the "cost").
    pub sell_required_items: Vec<TradeItem>,

    /// How many of the item the buy order requires per trade.
    pub buy_quantity_per_trade: i32,
    /// What the buy order offers in exchange (the "revenue").
    pub buy_offered_items: Vec<TradeItem>,

    /// Maximum number of trades possible (limited by remaining stock).
    pub max_trades: i32,

    /// Distance between the two shops in game units.
    pub distance: f64,

    /// TP energy cost for one trip (one-way).
    pub tp_energy_cost: f64,

    /// Number of trips needed to move all items.
    pub trips_needed: i32,

    /// Total TP energy for all trips.
    pub total_tp_energy: f64,

    /// Money made per trade (revenue - cost).
    pub money_made_per_trade: f64,

    /// Total money made for all possible trades.
    pub total_money_made: f64,

    /// Profit ratio: money made / TP energy used.
    pub profit_ratio: f64,

    /// Total value of items you receive from buy order (what you're getting)
    pub total_buy_value: f64,

    /// Total value of items you give to sell order (what you're paying)
    pub total_sell_cost: f64,

    /// The sell order's region.
    pub sell_region: String,
    /// The buy order's region.
    pub buy_region: String,
}

/// A simplified item reference for the response.
#[derive(Serialize, Deserialize, TS, Clone, Debug)]
#[ts(export)]
pub struct TradeItem {
    pub item_id: i32,
    pub item_name: String,
    pub quantity: i32,
    pub item_type: String,
    /// Market price per individual item
    pub price_per_item: f64,
    /// Total value of this item stack (quantity * price_per_item)
    pub total_value: f64,
}

/// Response for the profitable trades endpoint.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ProfitableTradesResponse {
    pub trades: Vec<ProfitableTrade>,
    pub total: usize,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
}

fn calculate_distance(x1: i64, z1: i64, x2: i64, z2: i64) -> f64 {
    let dx = (x2 - x1) as f64;
    let dz = (z2 - z1) as f64;
    (dx * dx + dz * dz).sqrt()
}

fn calculate_tp_energy(distance: f64, inventory_is_full: bool) -> f64 {
    let base = TELEPORTATION_BASE_ENERGY_COST + (distance * TELEPORTATION_COST_PER_LARGE_TILE);
    if inventory_is_full {
        base * TELEPORTATION_FULL_INVENTORY_MULTIPLIER
    } else {
        base
    }
}

fn max_items_per_trip(item_volume: i32) -> i32 {
    if item_volume <= 0 {
        return 0;
    }
    let stack_size = DEFAULT_POCKET_VOLUME / item_volume;
    if stack_size <= 0 {
        return 0;
    }
    DEFAULT_NUM_INVENTORY_POCKETS * stack_size
}

fn resolve_item_name(state: &AppState, item_id: i32, item_type: &ItemType) -> String {
    match item_type {
        ItemType::Item => state
            .item_desc
            .get(&item_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Item #{}", item_id)),
        ItemType::Cargo => state
            .cargo_desc
            .get(&item_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Cargo #{}", item_id)),
    }
}

fn calculate_item_value(state: &AppState, item_id: i32, item_type: &ItemType, quantity: i32, all_orders: &[entity::trade_order::Model]) -> f64 {
    if quantity <= 0 {
        return 0.0;
    }
    
    // Calculate what I can sell these items for right now
    let mut total_sell_value = 0.0;
    let mut total_demand_quantity: i64 = 0;
    
    for order in all_orders {
        // Skip traveler orders and cargo orders
        if order.traveler_trade_order_id.is_some() || 
           !order.offer_cargo_id.is_empty() || 
           !order.required_cargo_id.is_empty() ||
           order.remaining_stock <= 0 {
            continue;
        }
        
        // Look for orders that require this item (demand for what I want to sell)
        for req_item in &order.required_items {
            if req_item.item_id == item_id && req_item.item_type == *item_type && req_item.quantity > 0 {
                // This order wants to buy my item, calculate what they offer per unit
                let offer_value_per_unit: f64 = order.offer_items
                    .iter()
                    .filter(|offer| offer.quantity > 0)
                    .map(|offer| calculate_simple_item_value(state, offer.item_id, &offer.item_type, offer.quantity) / offer.quantity as f64)
                    .sum();
                
                let demand_quantity = req_item.quantity.min(order.remaining_stock);
                total_sell_value += offer_value_per_unit * demand_quantity as f64;
                total_demand_quantity = total_demand_quantity.saturating_add(demand_quantity as i64);
            }
        }
    }
    
    // Return the average sell price per unit times the quantity I have
    if total_demand_quantity > 0 {
        let avg_sell_price_per_unit = total_sell_value / total_demand_quantity as f64;
        avg_sell_price_per_unit * quantity as f64
    } else {
        // If no one wants to buy this item, it has no resale value
        0.0
    }
}

fn calculate_simple_item_value(state: &AppState, item_id: i32, item_type: &ItemType, quantity: i32) -> f64 {
    if quantity <= 0 {
        return 0.0;
    }
    
    let base_value = match item_type {
        ItemType::Item => {
            if let Some(desc) = state.item_desc.get(&item_id) {
                // Use tier and rarity as a proxy for item value
                let tier_multiplier = desc.tier as f64;
                let rarity_multiplier = match desc.rarity {
                    entity::shared::JsonRarity::Default => 1.0,
                    entity::shared::JsonRarity::Common => 1.0,
                    entity::shared::JsonRarity::Uncommon => 1.5,
                    entity::shared::JsonRarity::Rare => 2.0,
                    entity::shared::JsonRarity::Epic => 3.0,
                    entity::shared::JsonRarity::Legendary => 5.0,
                    entity::shared::JsonRarity::Mythic => 10.0,
                };
                tier_multiplier * rarity_multiplier
            } else {
                1.0 // Default value for unknown items
            }
        }
        ItemType::Cargo => {
            // For cargo items, use a simple base value
            if let Some(desc) = state.cargo_desc.get(&item_id) {
                desc.volume as f64 // Use volume as a simple proxy for cargo value
            } else {
                1.0
            }
        }
    };
    
    base_value * quantity as f64
}

fn build_trade_items(
    state: &AppState,
    items: &[entity::inventory::ItemStack],
    orders: &[entity::trade_order::Model],
) -> Vec<TradeItem> {
    items
        .iter()
        .map(|item| {
            let price_per_item = calculate_item_value(state, item.item_id, &item.item_type, 1, orders);
            let total_value = price_per_item * item.quantity as f64;
            TradeItem {
                item_id: item.item_id,
                item_name: resolve_item_name(state, item.item_id, &item.item_type),
                quantity: item.quantity,
                item_type: format!("{:?}", item.item_type),
                price_per_item,
                total_value,
            }
        })
        .collect()
}

/// Build a ProfitableTrade from a source order (you buy from) and destination order (you sell to),
/// for a specific item that the source offers and the destination requires.
fn build_profitable_trade(
    state: &AppState,
    source: &entity::trade_order::Model,
    dest: &entity::trade_order::Model,
    offer_item: &entity::inventory::ItemStack,
    req_item: &entity::inventory::ItemStack,
    orders: &[entity::trade_order::Model],
) -> Option<ProfitableTrade> {
    let item_id = offer_item.item_id;

    let (item_name, item_volume) = match &offer_item.item_type {
        ItemType::Item => {
            if let Some(desc) = state.item_desc.get(&item_id) {
                (desc.name.clone(), desc.volume)
            } else {
                (format!("Item #{}", item_id), 1)
            }
        }
        ItemType::Cargo => return None, // Shouldn't happen but guard anyway
    };

    let loc_src = state.location_state.get(&source.shop_entity_id);
    let loc_dst = state.location_state.get(&dest.shop_entity_id);

    let (distance, sell_region, buy_region) = match (loc_src, loc_dst) {
        (Some(a), Some(b)) => (
            calculate_distance(a.x, a.z, b.x, b.z),
            a.region.clone(),
            b.region.clone(),
        ),
        _ => (
            0.0,
            source.region.clone(),
            dest.region.clone(),
        ),
    };

    let capacity_per_trip = max_items_per_trip(item_volume);
    if capacity_per_trip <= 0 {
        return None;
    }

    let quantity_per_trade = offer_item.quantity.min(req_item.quantity);
    
    // Skip if quantity per trade is 0 to prevent division by zero
    if quantity_per_trade <= 0 {
        return None;
    }
    
    // Calculate max trades based on the specific item's stock availability
    // For the source (sell) order: how many times can it offer this item?
    let source_item_stock = if let Some(source_offer) = source.offer_items.iter().find(|item| item.item_id == item_id && item.item_type == offer_item.item_type) {
        source_offer.quantity
    } else {
        0
    };
    
    // For the destination (buy) order: how many times can it require this item?
    let dest_item_stock = if let Some(dest_req) = dest.required_items.iter().find(|item| item.item_id == item_id && item.item_type == offer_item.item_type) {
        dest_req.quantity
    } else {
        0
    };
    
    let max_trades_from_source = (source_item_stock / quantity_per_trade).max(0);
    let max_trades_from_dest = (dest_item_stock / quantity_per_trade).max(0);
    let max_trades = max_trades_from_source.min(max_trades_from_dest);
    
    let total_items = quantity_per_trade as i64 * max_trades as i64;

    let trips_needed = ((total_items as f64) / (capacity_per_trip as f64)).ceil().max(1.0) as i32;

    let items_in_last_trip = (total_items % capacity_per_trip as i64) as i32;
    let full_trips = if items_in_last_trip == 0 {
        trips_needed
    } else {
        trips_needed - 1
    };
    let partial_trip = if items_in_last_trip == 0 { 0 } else { 1 };

    let energy_full = full_trips as f64 * calculate_tp_energy(distance, true) * 2.0;
    let energy_partial = partial_trip as f64 * calculate_tp_energy(distance, false) * 2.0;
    let total_tp_energy = energy_full + energy_partial;

    // Calculate money made per trade
    let revenue_per_trade: f64 = dest.offer_items
        .iter()
        .map(|item| {
            if item.item_id == item_id && item.item_type == offer_item.item_type {
                calculate_item_value(state, item.item_id, &item.item_type, item.quantity, &orders)
            } else {
                0.0
            }
        })
        .sum();
    
    let cost_per_trade: f64 = source.required_items
        .iter()
        .map(|item| {
            calculate_item_value(state, item.item_id, &item.item_type, item.quantity, &orders)
        })
        .sum();
    
    let money_made_per_trade = revenue_per_trade - cost_per_trade;
    let total_money_made = money_made_per_trade * max_trades as f64;
    
    // Calculate total values for all trades
    let total_buy_value = revenue_per_trade * max_trades as f64;
    let total_sell_cost = cost_per_trade * max_trades as f64;
    
    // Calculate profit ratio (money made / TP energy used)
    let profit_ratio = if total_tp_energy > 0.0 {
        total_money_made / total_tp_energy
    } else {
        0.0
    };

    Some(ProfitableTrade {
        sell_order_entity_id: source.entity_id,
        sell_shop_entity_id: source.shop_entity_id,
        buy_order_entity_id: dest.entity_id,
        buy_shop_entity_id: dest.shop_entity_id,
        item_id,
        item_name,
        sell_quantity_per_trade: offer_item.quantity,
        sell_required_items: build_trade_items(state, &source.required_items, orders),
        buy_quantity_per_trade: req_item.quantity,
        buy_offered_items: build_trade_items(state, &dest.offer_items, orders),
        max_trades,
        distance,
        tp_energy_cost: calculate_tp_energy(distance, false),
        trips_needed,
        total_tp_energy,
        money_made_per_trade,
        total_money_made,
        profit_ratio,
        total_buy_value,
        total_sell_cost,
        sell_region,
        buy_region,
    })
}

async fn get_profitable_trades(
    state: State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<axum_codec::Codec<ProfitableTradesResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(100).min(500);

    // Collect all trade orders.
    let orders: Vec<entity::trade_order::Model> = state
        .trade_order_state
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    // Index orders by offered item_id and required item_id for fast matching.
    // Key: item_id, Value: indices into orders vec.
    let mut offers_by_item: HashMap<i32, Vec<usize>> = HashMap::new();
    let mut requires_by_item: HashMap<i32, Vec<usize>> = HashMap::new();

    for (idx, order) in orders.iter().enumerate() {
        // Skip traveler orders.
        if order.traveler_trade_order_id.is_some() {
            continue;
        }
        // Skip orders with cargo.
        if !order.offer_cargo_id.is_empty() || !order.required_cargo_id.is_empty() {
            continue;
        }
        // Skip depleted orders.
        if order.remaining_stock <= 0 {
            continue;
        }

        for item in &order.offer_items {
            if item.item_type == ItemType::Item {
                offers_by_item.entry(item.item_id).or_default().push(idx);
            }
        }
        for item in &order.required_items {
            if item.item_type == ItemType::Item {
                requires_by_item.entry(item.item_id).or_default().push(idx);
            }
        }
    }

    let mut profitable_trades: Vec<ProfitableTrade> = Vec::new();

    // For each item that is both offered and required, pair up the orders.
    for (item_id, offer_indices) in &offers_by_item {
        if let Some(require_indices) = requires_by_item.get(item_id) {
            for &src_idx in offer_indices {
                for &dst_idx in require_indices {
                    // Don't match an order with itself.
                    if src_idx == dst_idx {
                        continue;
                    }

                    let source = &orders[src_idx];
                    let dest = &orders[dst_idx];

                    // Find the specific offer and requirement items.
                    let offer_item = source
                        .offer_items
                        .iter()
                        .find(|i| i.item_id == *item_id && i.item_type == ItemType::Item);
                    let req_item = dest
                        .required_items
                        .iter()
                        .find(|i| i.item_id == *item_id && i.item_type == ItemType::Item);

                    if let (Some(offer), Some(req)) = (offer_item, req_item) {
                        if let Some(trade) = build_profitable_trade(&state, source, dest, offer, req, &orders) {
                            profitable_trades.push(trade);
                        }
                    }
                }
            }
        }
    }

    // Sort by highest total money made
    profitable_trades.sort_by(|a, b| {
        b.total_money_made
            .partial_cmp(&a.total_money_made)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = profitable_trades.len();

    // Paginate.
    let start = ((page - 1) * per_page) as usize;
    let trades: Vec<ProfitableTrade> = profitable_trades
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();

    Ok(axum_codec::Codec(ProfitableTradesResponse {
        trades,
        total,
        page,
        per_page,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_distance() {
        assert_eq!(calculate_distance(0, 0, 3, 4), 5.0);
        assert_eq!(calculate_distance(0, 0, 0, 0), 0.0);
        assert!((calculate_distance(1, 1, 4, 5) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_tp_energy_not_full() {
        let energy = calculate_tp_energy(100.0, false);
        assert_eq!(energy, 150.0);
    }

    #[test]
    fn test_tp_energy_full_inventory() {
        let energy = calculate_tp_energy(100.0, true);
        assert_eq!(energy, 300.0);
    }

    #[test]
    fn test_max_items_per_trip() {
        assert_eq!(max_items_per_trip(100), 1200);
        assert_eq!(max_items_per_trip(6000), 20);
        assert_eq!(max_items_per_trip(0), 0);
        assert_eq!(max_items_per_trip(7000), 0);
    }
}
