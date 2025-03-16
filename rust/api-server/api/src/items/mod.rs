use crate::config::Config;
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use crate::{AppState, Params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use entity::item_desc;
use log::{debug, error, info};
use reqwest::Client;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
    sea_query,
};
use serde_json::{Value, json};
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub async fn list_items(
    state: State<std::sync::Arc<AppState>>,
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

#[allow(dead_code)]
pub(crate) async fn load_item_desc_from_file(
    storage_path: &std::path::Path,
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

    import_items(conn, claim_descriptions, Some(3000)).await?;

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
                .filter(|item| match items_from_db_map.get(&item.id) {
                    Some(item_from_db) => item_from_db != *item,
                    None => true,
                })
                .map(|item| item.clone().into_active_model())
                .collect::<Vec<item_desc::ActiveModel>>();

            if things_to_insert.is_empty() {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} items", things_to_insert.len());
            }

            for item in &things_to_insert {
                let item_in = items_to_delete.iter().position(|id| id == item.id.as_ref());
                if let Some(item_in) = item_in {
                    items_to_delete.remove(item_in);
                }
            }

            let _ = item_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if !buffer_before_insert.is_empty() {
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
            .filter(|item| match items_from_db_map.get(&item.id) {
                Some(item_from_db) => item_from_db != *item,
                None => true,
            })
            .map(|item| item.clone().into_active_model())
            .collect::<Vec<item_desc::ActiveModel>>();

        if things_to_insert.is_empty() {
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

    if !items_to_delete.is_empty() {
        info!("item's to delete: {:?}", items_to_delete);
        item_desc::Entity::delete_many()
            .filter(item_desc::Column::Id.is_in(items_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub async fn import_job_item_desc(temp_config: Config) {
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

async fn get_known_item_desc_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_item_desc_ids: Vec<i64> = item_desc::Entity::find()
        .select_only()
        .column(item_desc::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;

    let known_item_desc_ids = known_item_desc_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_item_desc_ids)
}

async fn db_insert_item_descs(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<item_desc::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let item_descs_from_db = item_desc::Entity::find()
        .filter(
            item_desc::Column::Id.is_in(
                buffer_before_insert
                    .iter()
                    .map(|item_desc| item_desc.id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let item_descs_from_db_map = item_descs_from_db
        .into_iter()
        .map(|item_desc| (item_desc.id, item_desc))
        .collect::<HashMap<i64, item_desc::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(
            |item_desc| match item_descs_from_db_map.get(&item_desc.id) {
                Some(item_desc_from_db) => item_desc_from_db != *item_desc,
                None => true,
            },
        )
        .map(|item_desc| item_desc.clone().into_active_model())
        .collect::<Vec<item_desc::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} item_descs", things_to_insert.len());
    }

    let _ = item_desc::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_item_desc(
    conn: &DatabaseConnection,
    known_item_desc_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "item_desc's ({}) to delete: {:?}",
        known_item_desc_ids.len(),
        known_item_desc_ids
    );
    item_desc::Entity::delete_many()
        .filter(item_desc::Column::Id.is_in(known_item_desc_ids))
        .exec(conn)
        .await?;
    Ok(())
}

pub(crate) async fn handle_initial_subscription_item_desc(
    app_state: &Arc<AppState>,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<item_desc::Model> = Vec::with_capacity(chunk_size);

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

    let mut known_item_desc_ids = get_known_item_desc_ids(&app_state.conn).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<item_desc::Model>(row.as_ref()) {
                Ok(item_desc) => {
                    if known_item_desc_ids.contains(&item_desc.id) {
                        known_item_desc_ids.remove(&item_desc.id);
                    }
                    app_state.item_desc.insert(item_desc.id, item_desc.clone());
                    app_state.item_tiers.insert(item_desc.tier as i64);
                    app_state.item_tags.insert(item_desc.tag.clone());
                    buffer_before_insert.push(item_desc);
                    if buffer_before_insert.len() == chunk_size {
                        info!("ItemDesc insert");
                        db_insert_item_descs(
                            &app_state.conn,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert ItemDesc Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        info!("ItemDesc insert");
        db_insert_item_descs(&app_state.conn, &mut buffer_before_insert, &on_conflict).await?;
    }

    if !known_item_desc_ids.is_empty() {
        for item_desc_id in &known_item_desc_ids {
            app_state.item_desc.remove(item_desc_id);
        }
        delete_item_desc(&app_state.conn, known_item_desc_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update_item_desc(
    app_state: &Arc<AppState>,
    tables: &[TableWithOriginalEventTransactionUpdate],
) -> anyhow::Result<()> {
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

    let chunk_size = 5000;
    let mut buffer_before_insert = HashMap::new();

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<item_desc::Model>(row.as_ref()) {
                Ok(item_desc) => {
                    app_state.item_desc.insert(item_desc.id, item_desc.clone());
                    app_state.item_tiers.insert(item_desc.tier as i64);
                    app_state.item_tags.insert(item_desc.tag.clone());
                    found_in_inserts.insert(item_desc.id);
                    buffer_before_insert.insert(item_desc.id, item_desc);

                    if buffer_before_insert.len() == chunk_size {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<item_desc::Model>>();

                        db_insert_item_descs(
                            &app_state.conn,
                            &mut buffer_before_insert_vec,
                            &on_conflict,
                        )
                        .await?;
                        buffer_before_insert.clear();
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert ItemDesc Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<item_desc::Model>>();

        db_insert_item_descs(&app_state.conn, &mut buffer_before_insert_vec, &on_conflict).await?;
        buffer_before_insert.clear();
    }

    let mut item_descs_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<item_desc::Model>(row.as_ref()) {
                Ok(item_desc) => {
                    if !found_in_inserts.contains(&item_desc.id) {
                        app_state.item_desc.remove(&item_desc.id);
                        item_descs_to_delete.insert(item_desc.id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete ItemDesc Error: {error}");
                }
            }
        }
    }

    if !item_descs_to_delete.is_empty() {
        delete_item_desc(&app_state.conn, item_descs_to_delete).await?;
    }

    Ok(())
}
