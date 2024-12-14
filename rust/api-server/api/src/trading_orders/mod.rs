#![allow(warnings)]

use crate::{AppRouter, AppState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Router;
use axum_codec::Codec;
use entity::trade_order;
use futures::StreamExt;
use log::{debug, error, info};
use migration::sea_query;
use rayon::prelude::*;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use serde::Deserialize;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

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
    state: State<std::sync::Arc<AppState>>,
    Query(query): Query<TradeOrdersQuery>,
) -> Result<Codec<TradeOrdersResponse>, (StatusCode, &'static str)> {
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

    let items_ids = if items_ids.len() > 0 {
        Some(items_ids)
    } else {
        None
    };

    let cargo_ids = if cargo_ids.len() > 0 {
        Some(cargo_ids)
    } else {
        None
    };

    if items_ids.is_none() && cargo_ids.is_none() && search.is_some() {
        return Ok(Codec(TradeOrdersResponse {
            trade_orders: vec![],
            total: 0,
            page: 1,
            per_page: 0,
        }));
    }

    let mut total = 0;

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

    Ok(Codec(TradeOrdersResponse {
        trade_orders: filtered_trade_orders,
        total,
        page,
        per_page,
    }))
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct TradeOrdersResponse {
    trade_orders: Vec<trade_order::Model>,
    total: u64,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

pub(crate) async fn load_trade_order_from_file(storage_path: &PathBuf) -> anyhow::Result<String> {
    Ok(std::fs::read_to_string(
        storage_path.join("State/TradeOrderState.json"),
    )?)
}

pub(crate) async fn load_trade_order_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM TradeOrderState")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".into());
        }
    };

    Ok(json)
}

pub(crate) async fn load_trade_order(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
    config: &crate::config::Config,
) -> anyhow::Result<()> {
    let trade_orders = match &config.import_type {
        crate::config::ImportType::File => {
            load_trade_order_from_file(&PathBuf::from(&config.storage_path)).await?
        }
        crate::config::ImportType::Game => {
            load_trade_order_from_spacetimedb(client, domain, protocol, database).await?
        }
    };
    import_trade_order(&conn, trade_orders, None).await?;
    Ok(())
}

pub(crate) async fn import_trade_order(
    conn: &DatabaseConnection,
    trade_orders: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<trade_order::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(trade_orders.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(trade_order::Column::EntityId)
        .update_columns([
            trade_order::Column::BuildingEntityId,
            trade_order::Column::RemainingStock,
            trade_order::Column::OfferItems,
            trade_order::Column::OfferCargoId,
            trade_order::Column::RequiredItems,
            trade_order::Column::RequiredCargoId,
        ])
        .to_owned();

    let mut trade_order_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<trade_order::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let trade_order_from_db = trade_order::Entity::find()
                .filter(
                    trade_order::Column::EntityId.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|trade_order| trade_order.entity_id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if trade_order_from_db.len() != buffer_before_insert.len() {
                trade_order_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|trade_order| {
                            !trade_order_from_db.iter().any(|trade_order_from_db| {
                                trade_order_from_db.entity_id == trade_order.entity_id
                            })
                        })
                        .map(|trade_order| trade_order.entity_id),
                );
            }

            let trade_order_from_db_map = trade_order_from_db
                .into_iter()
                .map(|trade_order| (trade_order.entity_id, trade_order))
                .collect::<HashMap<i64, trade_order::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|trade_order| {
                    match trade_order_from_db_map.get(&trade_order.entity_id) {
                        Some(trade_order_from_db) => {
                            if trade_order_from_db != *trade_order {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|trade_order| trade_order.clone().into_active_model())
                .collect::<Vec<trade_order::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} trade_order", things_to_insert.len());
            }

            for trade_order in &things_to_insert {
                let trade_order_in = trade_order_to_delete
                    .iter()
                    .position(|id| id == trade_order.entity_id.as_ref());
                if trade_order_in.is_some() {
                    trade_order_to_delete.remove(trade_order_in.unwrap());
                }
            }

            let _ = trade_order::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let trade_order_from_db = trade_order::Entity::find()
            .filter(
                trade_order::Column::EntityId.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|trade_order| trade_order.entity_id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let trade_order_from_db_map = trade_order_from_db
            .into_iter()
            .map(|trade_order| (trade_order.entity_id, trade_order))
            .collect::<HashMap<i64, trade_order::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|trade_order| {
                match trade_order_from_db_map.get(&trade_order.entity_id) {
                    Some(trade_order_from_db) => {
                        if trade_order_from_db != *trade_order {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|trade_order| trade_order.clone().into_active_model())
            .collect::<Vec<trade_order::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} trade_order", things_to_insert.len());
            trade_order::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("trade_order last batch imported");
    }
    info!(
        "Importing trade_order finished in {}s",
        start.elapsed().as_secs()
    );

    if trade_order_to_delete.len() > 0 {
        info!("trade_order's to delete: {:?}", trade_order_to_delete);
        trade_order::Entity::delete_many()
            .filter(trade_order::Column::EntityId.is_in(trade_order_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}
