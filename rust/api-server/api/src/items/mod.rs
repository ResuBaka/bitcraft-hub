use crate::{AppState, Params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use entity::item;
use log::{debug, error, info};
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter,
};
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

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

#[allow(dead_code)]
pub(crate) async fn load_item_desc_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<item::Model>> {
    let item_file = File::open(storage_path.join("State/InventoryState.json"))?;
    let inventory: Value = serde_json::from_reader(&item_file)?;
    let inventory: Vec<item::Model> =
        serde_json::from_value(inventory.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(inventory)
}

pub(crate) async fn load_item_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM ItemDesc")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".to_string());
        }
    };

    Ok(json)
}

pub(crate) async fn load_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_item_desc_from_spacetimedb(client, domain, protocol, database).await?;

    import_items(&conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}

pub(crate) async fn import_items(
    conn: &DatabaseConnection,
    items: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<item::Model> = Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(items.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(item::Column::Id)
        .update_columns([
            item::Column::Name,
            item::Column::Description,
            item::Column::Volume,
            item::Column::Durability,
            item::Column::SecondaryKnowledgeId,
            item::Column::ModelAssetName,
            item::Column::IconAssetName,
            item::Column::Tier,
            item::Column::Tag,
            item::Column::Rarity,
            item::Column::CompendiumEntry,
            item::Column::ItemListId,
        ])
        .to_owned();

    let mut items_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<item::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let items_from_db = item::Entity::find()
                .filter(
                    item::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|item| item.id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if items_from_db.len() != buffer_before_insert.len() {
                items_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|item| {
                            !items_from_db
                                .iter()
                                .any(|item_from_db| item_from_db.id == item.id)
                        })
                        .map(|item| item.id),
                );
            }

            let items_from_db_map = items_from_db
                .into_iter()
                .map(|item| (item.id, item))
                .collect::<HashMap<i64, item::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|item| {
                    match items_from_db_map.get(&item.id) {
                        Some(item_from_db) => {
                            if item_from_db != *item {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|item| item.clone().into_active_model())
                .collect::<Vec<item::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} items", things_to_insert.len());
            }

            for item in &things_to_insert {
                let item_in = items_to_delete.iter().position(|id| id == item.id.as_ref());
                if item_in.is_some() {
                    items_to_delete.remove(item_in.unwrap());
                }
            }

            let _ = item::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let items_from_db = item::Entity::find()
            .filter(
                item::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|item| item.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let items_from_db_map = items_from_db
            .into_iter()
            .map(|item| (item.id, item))
            .collect::<HashMap<i64, item::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|item| {
                match items_from_db_map.get(&item.id) {
                    Some(item_from_db) => {
                        if item_from_db != *item {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|item| item.clone().into_active_model())
            .collect::<Vec<item::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} items", things_to_insert.len());
            item::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("item last batch imported");
    }
    info!("Importing item finished in {}s", start.elapsed().as_secs());

    if items_to_delete.len() > 0 {
        info!("item's to delete: {:?}", items_to_delete);
        item::Entity::delete_many()
            .filter(item::Column::Id.is_in(items_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}
