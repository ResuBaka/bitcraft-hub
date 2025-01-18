use crate::{AppRouter, AppState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::cargo_desc;
use entity::item_desc;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;

pub(crate) fn get_routes() -> AppRouter {
    Router::new().route("/api/bitcraft/itemsAndCargo", get(list_items_and_cargo))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ItemCargo {
    Item(item_desc::Model),
    Cargo(cargo_desc::Model),
}

#[derive(Deserialize)]
pub(crate) struct ItemsAndCargoParams {
    page: Option<u64>,
    per_page: Option<u64>,
    search: Option<String>,
    tier: Option<i32>,
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ItemsAndCargoResponse {
    items: Vec<ItemCargo>,
    tags: Vec<String>,
    tiers: Vec<i32>,
    per_page: u64,
    total: u64,
    page: u64,
    pages: u64,
}

pub(crate) async fn list_items_and_cargo(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<ItemsAndCargoParams>,
) -> Result<Json<ItemsAndCargoResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = match params.search {
        Some(search) => Some(search.to_lowercase()),
        None => None,
    };
    let tier = params.tier;
    let tag = params.tag;

    let (items, items_tags, items_tiers, cargos, cargos_tags, cargos_tiers) = tokio::join!(
        QueryCore::search_items_desc(&state.conn, &search, &tier, &tag),
        QueryCore::find_unique_item_tags(&state.conn),
        QueryCore::find_unique_item_tiers(&state.conn),
        QueryCore::search_cargos_desc(&state.conn, &search, &tier, &tag),
        QueryCore::find_unique_cargo_tags(&state.conn),
        QueryCore::find_unique_cargo_tiers(&state.conn),
    );

    let items = items.expect("Cannot find items");
    let items_tags = items_tags.expect("Cannot find tags");
    let items_tiers = items_tiers.expect("Cannot find tiers");

    let cargos = cargos.expect("Cannot find cargos");
    let cargos_tags = cargos_tags.expect("Cannot find tags");
    let cargos_tiers = cargos_tiers.expect("Cannot find tiers");

    let mut merged_tags = merge_tags(items_tags, cargos_tags);
    let mut merged_tiers = merge_tiers(items_tiers, cargos_tiers);
    let merged_items_and_cargo = merge_items_and_cargo(items, cargos);

    let (start, end) = (
        ((page - 1) * posts_per_page) as usize,
        (page * posts_per_page) as usize,
    );

    let items = match merged_items_and_cargo.len() {
        x if x > end => merged_items_and_cargo[start..end].to_vec(),
        x if x < end => merged_items_and_cargo[start..].to_vec(),
        _ => vec![],
    };

    merged_tiers.sort();
    merged_tags.sort();
    Ok(Json(ItemsAndCargoResponse {
        items,
        tiers: merged_tiers,
        tags: merged_tags,
        per_page: posts_per_page,
        total: merged_items_and_cargo.len() as u64,
        page,
        pages: merged_items_and_cargo.len() as u64 / posts_per_page,
    }))
}

fn merge_tags(items_tags: Vec<String>, cargo_tags: Vec<String>) -> Vec<String> {
    let mut merged_tags = items_tags;
    for tag in cargo_tags {
        if !merged_tags.contains(&tag) {
            merged_tags.push(tag);
        }
    }
    merged_tags
}

fn merge_tiers(items_tiers: Vec<i32>, cargo_tiers: Vec<i32>) -> Vec<i32> {
    let mut merged_tiers = items_tiers;
    for tier in cargo_tiers {
        if !merged_tiers.contains(&tier) {
            merged_tiers.push(tier);
        }
    }
    merged_tiers
}

fn merge_items_and_cargo(
    items: Vec<item_desc::Model>,
    cargo: Vec<cargo_desc::Model>,
) -> Vec<ItemCargo> {
    let mut merged_items = Vec::new();
    for item in items {
        merged_items.push(ItemCargo::Item(item));
    }
    for cargo in cargo {
        merged_items.push(ItemCargo::Cargo(cargo));
    }
    merged_items
}
