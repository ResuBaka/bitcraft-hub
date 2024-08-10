use crate::{AppState, Params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use entity::item;
use log::info;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::fs::File;
use std::path::PathBuf;

pub async fn list_items(
    state: State<AppState>,
    Query(params): Query<Params>,
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
    storage_path: &PathBuf,
) -> anyhow::Result<()> {
    info!("Importing items");
    let item_file = File::open(storage_path.join("Desc/ItemDesc.json")).unwrap();
    let item: Value = serde_json::from_reader(&item_file).unwrap();
    let item: Vec<item::Model> =
        serde_json::from_value(item.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = item.len();
    let db_count = item::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let item: Vec<item::ActiveModel> = item.into_iter().map(|x| x.into_active_model()).collect();

    for item in item.chunks(2000) {
        let _ = item::Entity::insert_many(item.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
