use axum::extract::{Query, State};
use tower_cookies::Cookies;
use axum::Json;
use serde_json::{json, Value};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use std::fs::File;
use entity::item;
use crate::{AppState, Params};

pub async fn list_items(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
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

    Ok(Json(json!({
        "items": items,
        "tiers": tiers,
        "tags": tags,
        "perPage": posts_per_page,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}

pub(crate) async fn import_items(
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/Desc/ItemDesc.json").unwrap();
    let item: Value = serde_json::from_reader(&item_file).unwrap();
    let item: Vec<item::Model> = serde_json::from_value(item.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = item.len();
    let db_count = item::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    for item in item {
        let _ = item.into_active_model().insert(conn).await;
    }

    Ok(())
}