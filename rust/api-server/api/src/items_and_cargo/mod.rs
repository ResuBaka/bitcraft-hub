use std::collections::HashMap;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::cargo_desc;
use entity::item_desc;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/api/bitcraft/itemsAndCargo",
            axum_codec::routing::get(list_items_and_cargo).into(),
        )
        .route(
            "/api/bitcraft/itemsAndCargo/meta",
            axum_codec::routing::get(meta).into(),
        )
        .route(
            "/api/bitcraft/itemsAndCargo/all",
            axum_codec::routing::get(get_all).into(),
        )
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
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
    tiers: Vec<i64>,
    per_page: u64,
    total: u64,
    page: u64,
    pages: u64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct MetaResponse {
    tags: Vec<String>,
    tiers: Vec<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ItemsAndCargollResponse {
    cargo_desc: HashMap<i32, cargo_desc::Model>,
    item_desc: HashMap<i32, item_desc::Model>,
}
pub(crate) async fn get_all(
    state: State<std::sync::Arc<AppState>>,
) -> Result<axum_codec::Codec<ItemsAndCargollResponse>, (StatusCode, &'static str)> {
    return Ok(axum_codec::Codec(ItemsAndCargollResponse {
        cargo_desc: state
            .cargo_desc
            .iter()
            .map(|value| (value.key().clone(), value.clone()))
            .collect(),
        item_desc: state
            .item_desc
            .iter()
            .map(|value| (value.key().clone(), value.clone()))
            .collect(),
    }));
}

pub(crate) async fn list_items_and_cargo(
    state: State<std::sync::Arc<AppState>>,
    Query(params): Query<ItemsAndCargoParams>,
) -> Result<axum_codec::Codec<ItemsAndCargoResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search.map(|search| search.to_lowercase());
    let tier = params.tier;
    let tag = params.tag;

    if state.cargo_tags.is_empty()
        || state.cargo_tiers.is_empty()
        || state.item_tags.is_empty()
        || state.item_tiers.is_empty()
    {
        let (items_tags, items_tiers, cargos_tags, cargos_tiers) = tokio::join!(
            QueryCore::find_unique_item_tags(&state.conn),
            QueryCore::find_unique_item_tiers(&state.conn),
            QueryCore::find_unique_cargo_tags(&state.conn),
            QueryCore::find_unique_cargo_tiers(&state.conn),
        );

        let items_tags = items_tags.expect("Cannot find tags");
        let items_tiers = items_tiers.expect("Cannot find tiers");
        let cargos_tags = cargos_tags.expect("Cannot find tags");
        let cargos_tiers = cargos_tiers.expect("Cannot find tiers");

        for item_tag in items_tags {
            state.item_tags.insert(item_tag);
        }

        for item_tier in items_tiers {
            state.item_tiers.insert(item_tier as i64);
        }

        for cargo_tag in cargos_tags {
            state.cargo_tags.insert(cargo_tag);
        }

        for cargo_tier in cargos_tiers {
            state.cargo_tiers.insert(cargo_tier as i64);
        }
    }

    if state.item_desc.is_empty() || state.cargo_desc.is_empty() {
        let (items, cargos) = tokio::join!(
            QueryCore::search_items_desc(&state.conn, &search, &tier, &tag),
            QueryCore::search_cargos_desc(&state.conn, &search, &tier, &tag),
        );

        let items = items.expect("Cannot find items");
        let cargos = cargos.expect("Cannot find cargos");

        for cargo in cargos {
            state.cargo_desc.insert(cargo.id, cargo);
        }

        for item in items {
            state.item_desc.insert(item.id, item);
        }
    }

    let mut merged_tags = merge_tags(
        state.item_tags.iter().map(|tier| tier.to_owned()).collect(),
        state
            .cargo_tags
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
    );
    let mut merged_tiers = merge_tiers(
        state
            .item_tiers
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
        state
            .cargo_tiers
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
    );
    let mut merged_items_and_cargo = merge_items_and_cargo(
        state
            .item_desc
            .iter()
            .filter_map(|item| {
                if let Some(tier) = &tier {
                    if &item.tier != tier {
                        return None;
                    }
                };

                if let Some(tag) = &tag {
                    if &item.tag != tag {
                        return None;
                    }
                };

                if let Some(search) = &search {
                    if !item
                        .name
                        .to_lowercase()
                        .contains(search.to_lowercase().as_str())
                    {
                        return None;
                    }
                };

                Some(item.to_owned())
            })
            .collect(),
        state
            .cargo_desc
            .iter()
            .filter_map(|cargo| {
                if let Some(tier) = &tier {
                    if &cargo.tier != tier {
                        return None;
                    }
                };

                if let Some(tag) = &tag {
                    if &cargo.tag != tag {
                        return None;
                    }
                };

                if let Some(search) = &search {
                    if !cargo
                        .name
                        .to_lowercase()
                        .contains(search.to_lowercase().as_str())
                    {
                        return None;
                    }
                };

                Some(cargo.to_owned())
            })
            .collect(),
    );

    let (start, end) = (
        ((page - 1) * posts_per_page) as usize,
        (page * posts_per_page) as usize,
    );

    merged_items_and_cargo.sort_by(|a, b| {
        let (a_tier, b_tier, a_id, b_id) = match (a, b) {
            (ItemCargo::Cargo(a), ItemCargo::Cargo(b)) => (a.tier, b.tier, a.id, b.id),
            (ItemCargo::Cargo(a), ItemCargo::Item(b)) => (a.tier, b.tier, a.id, b.id),
            (ItemCargo::Item(a), ItemCargo::Cargo(b)) => (a.tier, b.tier, a.id, b.id),
            (ItemCargo::Item(a), ItemCargo::Item(b)) => (a.tier, b.tier, a.id, b.id),
        };

        if b_tier == a_tier {
            a_id.cmp(&b_id)
        } else {
            if a_tier <= 0 && b_tier > 0 {
                return std::cmp::Ordering::Greater;
            }

            if a_tier > 0 && b_tier <= 0 {
                return std::cmp::Ordering::Less;
            }

            a_tier.cmp(&b_tier)
        }
    });

    let items = match merged_items_and_cargo.len() {
        x if x > end => merged_items_and_cargo[start..end].to_vec(),
        x if x < end => merged_items_and_cargo[start..].to_vec(),
        _ => vec![],
    };

    merged_tiers.sort();
    merged_tags.sort();
    Ok(axum_codec::Codec(ItemsAndCargoResponse {
        items,
        tiers: merged_tiers,
        tags: merged_tags,
        per_page: posts_per_page,
        total: merged_items_and_cargo.len() as u64,
        page,
        pages: (merged_items_and_cargo.len() as u64).div_ceil(posts_per_page),
    }))
}

pub(crate) async fn meta(
    state: State<std::sync::Arc<AppState>>,
) -> Result<axum_codec::Codec<MetaResponse>, (StatusCode, &'static str)> {
    if state.cargo_tags.is_empty()
        || state.cargo_tiers.is_empty()
        || state.item_tags.is_empty()
        || state.item_tiers.is_empty()
    {
        let (items_tags, items_tiers, cargos_tags, cargos_tiers) = tokio::join!(
            QueryCore::find_unique_item_tags(&state.conn),
            QueryCore::find_unique_item_tiers(&state.conn),
            QueryCore::find_unique_cargo_tags(&state.conn),
            QueryCore::find_unique_cargo_tiers(&state.conn),
        );

        let items_tags = items_tags.expect("Cannot find tags");
        let items_tiers = items_tiers.expect("Cannot find tiers");
        let cargos_tags = cargos_tags.expect("Cannot find tags");
        let cargos_tiers = cargos_tiers.expect("Cannot find tiers");

        for item_tag in items_tags {
            state.item_tags.insert(item_tag);
        }

        for item_tier in items_tiers {
            state.item_tiers.insert(item_tier as i64);
        }

        for cargo_tag in cargos_tags {
            state.cargo_tags.insert(cargo_tag);
        }

        for cargo_tier in cargos_tiers {
            state.cargo_tiers.insert(cargo_tier as i64);
        }
    }

    let mut merged_tags = merge_tags(
        state.item_tags.iter().map(|tier| tier.to_owned()).collect(),
        state
            .cargo_tags
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
    );
    let mut merged_tiers = merge_tiers(
        state
            .item_tiers
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
        state
            .cargo_tiers
            .iter()
            .map(|tier| tier.to_owned())
            .collect(),
    );

    merged_tiers.sort();
    merged_tags.sort();
    Ok(axum_codec::Codec(MetaResponse {
        tiers: merged_tiers,
        tags: merged_tags,
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

fn merge_tiers(items_tiers: Vec<i64>, cargo_tiers: Vec<i64>) -> Vec<i64> {
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
