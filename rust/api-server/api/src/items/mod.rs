use crate::config::Config;
use crate::{AppState, Params};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::item_desc;
use log::{debug, error, info};
use reqwest::Client;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, sea_query,
};
use serde_json::{Value, json};
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub async fn list_items(
    state: State<std::sync::Arc<AppState>>,
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
) -> anyhow::Result<Vec<item_desc::Model>> {
    let item_file = File::open(storage_path.join("State/InventoryState.json"))?;
    let inventory: Value = serde_json::from_reader(&item_file)?;
    let inventory: Vec<item_desc::Model> =
        serde_json::from_value(inventory.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(inventory)
}

pub(crate) async fn load_item_desc_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM item_desc")
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
    client: &Client,
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

    let mut buffer_before_insert: Vec<item_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));
    let mut json_stream_reader = JsonStreamReader::new(items.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(item_desc::Column::Id)
        .update_columns([
            item_desc::Column::Name,
            item_desc::Column::Description,
            item_desc::Column::Volume,
            item_desc::Column::Durability,
            item_desc::Column::ConvertToOnDurabilityZero,
            item_desc::Column::SecondaryKnowledgeId,
            item_desc::Column::ModelAssetName,
            item_desc::Column::IconAssetName,
            item_desc::Column::Tier,
            item_desc::Column::Tag,
            item_desc::Column::Rarity,
            item_desc::Column::CompendiumEntry,
            item_desc::Column::ItemListId,
        ])
        .to_owned();

    let mut items_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<item_desc::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let items_from_db = item_desc::Entity::find()
                .filter(
                    item_desc::Column::Id.is_in(
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
                .collect::<HashMap<i64, item_desc::Model>>();

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
                .collect::<Vec<item_desc::ActiveModel>>();

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

            let _ = item_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let items_from_db = item_desc::Entity::find()
            .filter(
                item_desc::Column::Id.is_in(
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
            .collect::<HashMap<i64, item_desc::Model>>();

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
            .collect::<Vec<item_desc::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} items", things_to_insert.len());
            item_desc::Entity::insert_many(things_to_insert)
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
        item_desc::Entity::delete_many()
            .filter(item_desc::Column::Id.is_in(items_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub async fn import_job_item_desc(temp_config: Config) -> () {
    let temp_config = temp_config.clone();
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_internal_items(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_items(config.clone(), conn, client);
    }
}

fn import_internal_items(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = load_state_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("Items imported");
                } else {
                    error!("Items import failed: {:?}", vehicle_state);
                }
            });
    });
}
