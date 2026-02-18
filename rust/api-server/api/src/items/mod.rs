use crate::{AppState, Params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::item_desc;
use serde_json::{Value, json};
use service::Query as QueryCore;
use std::collections::HashMap;

pub(crate) mod bitcraft;

pub async fn list_items(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<axum_codec::Codec<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (items, tags, tiers) = tokio::join!(
        QueryCore::find_items(&state.conn, page, posts_per_page, search),
        QueryCore::find_unique_item_tags(&state.conn),
        QueryCore::find_unique_item_tiers(&state.conn),
    );

    let (items, num_pages) = items.expect("Cannot find items");
    let tags = tags.expect("Cannot find tags");
    let tiers = tiers.expect("Cannot find tiers");

    Ok(axum_codec::Codec(json!({
        "items": items,
        "tiers": tiers,
        "tags": tags,
        "perPage": posts_per_page,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}

pub async fn list_world_items(
    state: State<AppState>,
) -> Result<axum_codec::Codec<HashMap<i32, item_desc::Model>>, (StatusCode, &'static str)> {
    let items: HashMap<i32, item_desc::Model> = state
        .item_desc
        .iter()
        .map(|value| (*value.key(), value.clone()))
        .collect();

    Ok(axum_codec::Codec(items))
}
