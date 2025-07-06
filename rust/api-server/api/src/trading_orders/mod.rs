use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::trade_order;
use futures::StreamExt;
use log::error;
use rayon::prelude::*;
use sea_orm::{EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;

pub(crate) fn get_routes() -> AppRouter {
    Router::new().route(
        "/api/bitcraft/trade_orders/get_trade_orders",
        axum_codec::routing::get(get_trade_orders).into(),
    )
}

#[derive(Deserialize, Debug)]
struct TradeOrdersQuery {
    search: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

async fn get_trade_orders(
    state: State<AppState>,
    Query(query): Query<TradeOrdersQuery>,
) -> Result<axum_codec::Codec<TradeOrdersResponse>, (StatusCode, &'static str)> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(24);
    let search = query.search;

    let items_ids = if search.is_some() {
        QueryCore::search_items_desc_ids(&state.conn, &search)
            .await
            .map_err(|error| {
                error!("Error: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            })?
    } else {
        vec![]
    };

    let cargo_ids = if search.is_some() {
        QueryCore::search_cargo_desc_ids(&state.conn, &search)
            .await
            .map_err(|error| {
                error!("Error: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            })?
    } else {
        vec![]
    };

    let items_ids = if !items_ids.is_empty() {
        Some(items_ids)
    } else {
        None
    };

    let cargo_ids = if !cargo_ids.is_empty() {
        Some(cargo_ids)
    } else {
        None
    };

    if items_ids.is_none() && cargo_ids.is_none() && search.is_some() {
        return Ok(axum_codec::Codec(TradeOrdersResponse {
            trade_orders: vec![],
            total: 0,
            page: 1,
            per_page: 0,
        }));
    }

    let total;

    let filtered_trade_orders = if items_ids.is_some() || cargo_ids.is_some() {
        let trade_orders = trade_order::Entity::find()
            .order_by_asc(trade_order::Column::EntityId)
            .stream(&state.conn)
            .await
            .map_err(|error| {
                error!("Error: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            })?;

        let mut filtered_trade_orders = Vec::new();
        let mut chunks = trade_orders.chunks(5000);

        while let Some(chunk) = chunks.next().await {
            let resolved_chunk = chunk
                .into_iter()
                .map(|trade_order| trade_order.unwrap())
                .collect::<Vec<trade_order::Model>>();

            let local_filtered_trade_orders = resolved_chunk
                .into_par_iter()
                .filter(|trade_order| {
                    if let Some(items_ids) = &items_ids {
                        let trade_order_items = trade_order
                            .offer_items
                            .par_iter()
                            .map(|item| item.item_id)
                            .collect::<Vec<i64>>();

                        if items_ids
                            .par_iter()
                            .any(|item_id| trade_order_items.contains(item_id))
                        {
                            return true;
                        }
                    }

                    if let Some(cargo_ids) = &cargo_ids {
                        let trade_order_cargo_ids: Vec<i64> =
                            serde_json::from_value(trade_order.offer_cargo_id.clone()).unwrap();

                        if cargo_ids
                            .par_iter()
                            .any(|cargo_id| trade_order_cargo_ids.contains(cargo_id))
                        {
                            return true;
                        }
                    }

                    false
                })
                .collect::<Vec<trade_order::Model>>();

            filtered_trade_orders.extend(local_filtered_trade_orders);
        }

        let (start, end) = (((page - 1) * per_page) as usize, (page * per_page) as usize);

        total = filtered_trade_orders.len() as u64;

        match filtered_trade_orders.len() {
            x if x > end => filtered_trade_orders[start..end].to_vec(),
            x if x < end => filtered_trade_orders[start..].to_vec(),
            _ => vec![],
        }
    } else {
        let (trade_orders, num_pages) =
            QueryCore::load_trade_order_paginated(&state.conn, page, per_page)
                .await
                .map_err(|error| {
                    error!("Error: {error}");
                    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                })?;

        total = num_pages.number_of_items;
        trade_orders
    };

    Ok(axum_codec::Codec(TradeOrdersResponse {
        trade_orders: filtered_trade_orders,
        total,
        page,
        per_page,
    }))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct TradeOrdersResponse {
    trade_orders: Vec<trade_order::Model>,
    total: u64,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}
