#![allow(warnings)]

use crate::config::Config;
use crate::{AppRouter, AppState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use dashmap::DashMap;
use entity::crafting_recipe::ConsumedItemStackWithInner;
use entity::crafting_recipe::{self, CraftingRecipeWithInner};
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::ops::Add;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/api/bitcraft/recipes/needed_in_crafting/{id}",
            get(get_needed_in_crafting),
        )
        .route(
            "/api/bitcraft/recipes/produced_in_crafting/{id}",
            get(get_produced_in_crafting),
        )
        .route(
            "/api/bitcraft/recipes/needed_to_craft/{id}",
            get(get_needed_to_craft),
        )
        .route(
            "/recipes/needed_in_crafting/{id}",
            get(get_needed_in_crafting),
        )
        .route(
            "/recipes/produced_in_crafting/{id}",
            get(get_produced_in_crafting),
        )
        .route("/recipes/needed_to_craft/{id}", get(get_needed_to_craft))
}

pub(crate) async fn get_needed_in_crafting(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
    return Ok(Json(vec![]));

    let recipes = QueryCore::load_all_recipes(&state.conn).await;

    let recipes = recipes
        .iter()
        .filter(|res| {
            res.consumed_item_stacks
                .iter()
                .filter(|cis| cis.item_id == id as i64)
                .count()
                > 0
        })
        .map(|x| x.clone())
        .collect();

    Ok(Json(recipes))
}

pub(crate) async fn get_produced_in_crafting(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
    return Ok(Json(vec![]));
    let recipes = QueryCore::load_all_recipes(&state.conn).await;

    let recipes = recipes
        .iter()
        .filter(|res| {
            res.crafted_item_stacks
                .iter()
                .filter(|cis| cis.item_id == id as i64)
                .count()
                > 0
        })
        .map(|x| x.clone())
        .collect();

    Ok(Json(recipes))
}

pub(crate) async fn get_needed_to_craft(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<Vec<Vec<ConsumedItemStackWithInner>>>, (StatusCode, &'static str)> {
    let recipes = &state.crafting_recipe_desc;
    let mut consumed_item: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut crafted_item: HashMap<i64, Vec<i64>> = HashMap::new();
    for recipe in recipes.iter() {
        let model = recipe.value();
        for item in model.consumed_item_stacks.iter() {
            if let Some(consumed_item) = consumed_item.get_mut(&item.item_id) {
                consumed_item.push(model.id.clone())
            } else {
                consumed_item.insert(item.item_id, vec![model.id]);
            }
        }
        for item in model.crafted_item_stacks.iter() {
            if let Some(crafted_item) = crafted_item.get_mut(&item.item_id) {
                crafted_item.push(model.id.clone())
            } else {
                crafted_item.insert(item.item_id, vec![model.id]);
            }
        }
    }
    return Ok(Json(get_all_consumed_items_from_item(
        recipes,
        id as i64,
        &crafted_item,
    )));
}

fn get_all_consumed_items_from_item(
    rows: &Arc<DashMap<i64, entity::crafting_recipe::Model>>,
    item_id: i64,
    crafted_item: &HashMap<i64, Vec<i64>>,
) -> Vec<Vec<ConsumedItemStackWithInner>> {
    let mut list = Vec::new();

    let posibilities: Vec<Vec<ConsumedItemStackWithInner>> = vec![];
    let crafted_items = crafted_item.get(&item_id);
    if let Some(crafted_items) = crafted_items {
        for crafted_item_id in crafted_items {
            list.push(get_all_consumed_items_from_stack(
                rows,
                &mut rows.get(&crafted_item_id).unwrap().value().clone().into(),
                &crafted_item,
                vec![crafted_item_id.clone()],
            ));
        }
    }

    list
}

fn get_all_consumed_items_from_stack(
    rows: &Arc<DashMap<i64, entity::crafting_recipe::Model>>,
    item: &mut crafting_recipe::CraftingRecipeWithInner,
    crafted_item: &HashMap<i64, Vec<i64>>,
    already_used: Vec<i64>,
) -> Vec<ConsumedItemStackWithInner> {
    for itemstack in item.consumed_item_stacks.iter_mut() {
        let mut posibilities: Vec<entity::crafting_recipe::CraftingRecipeWithInner> = vec![];

        let crafted_items = crafted_item.get(&itemstack.item_id);
        if let Some(crafted_items) = crafted_items {
            for crafted_item_id in crafted_items {
                posibilities.push(rows.get(&crafted_item_id).unwrap().value().clone().into());
            }
        }

        let mut list = Vec::new();

        for posibilitie in posibilities.iter_mut() {
            let mut posibilitie = posibilitie.clone();
            if already_used.contains(&posibilitie.id) {
                continue;
            }

            let mut temp = already_used.clone();
            temp.push(posibilitie.id.clone());

            list.push(get_all_consumed_items_from_stack(
                rows,
                &mut posibilitie,
                &crafted_item,
                temp,
            ));
        }
        itemstack.inner = Some(list);
    }

    item.consumed_item_stacks.clone()
}

#[allow(dead_code)]
pub(crate) async fn load_crafting_recipe_desc_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<crafting_recipe::Model>> {
    let crafting_recipe_desc_file = File::open(storage_path.join("Desc/CraftingRecipeDesc.json"))?;
    let crafting_recipe_desc: Value = serde_json::from_reader(&crafting_recipe_desc_file)?;
    let crafting_recipe_desc: Vec<crafting_recipe::Model> = serde_json::from_value(
        crafting_recipe_desc
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )?;

    Ok(crafting_recipe_desc)
}

pub(crate) async fn load_crafting_recipe_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM crafting_recipe_desc")
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

pub(crate) async fn load_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_crafting_recipe_desc_from_spacetimedb(client, domain, protocol, database).await?;

    import_crafting_recipe_descs(&conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}

pub async fn import_job_recipes_desc(temp_config: Config) -> () {
    let temp_config = temp_config.clone();
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_internal_recipes_desc(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_recipes_desc(config.clone(), conn, client);
    }
}
fn import_internal_recipes_desc(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("recipes imported");
                } else {
                    error!("recipes import failed: {:?}", vehicle_state);
                }
            });
    });
}

pub(crate) async fn import_crafting_recipe_descs(
    conn: &DatabaseConnection,
    crafting_recipe_descs: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<crafting_recipe::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(crafting_recipe_descs.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
        .update_columns([
            crafting_recipe::Column::Name,
            crafting_recipe::Column::TimeRequirement,
            crafting_recipe::Column::StaminaRequirement,
            crafting_recipe::Column::ToolDurabilityLost,
            crafting_recipe::Column::BuildingRequirement,
            crafting_recipe::Column::LevelRequirements,
            crafting_recipe::Column::ToolRequirements,
            crafting_recipe::Column::ConsumedItemStacks,
            crafting_recipe::Column::DiscoveryTriggers,
            crafting_recipe::Column::RequiredKnowledges,
            crafting_recipe::Column::RequiredClaimTechId,
            crafting_recipe::Column::FullDiscoveryScore,
            crafting_recipe::Column::ExperiencePerProgress,
            crafting_recipe::Column::AllowUseHands,
            crafting_recipe::Column::CraftedItemStacks,
            crafting_recipe::Column::IsPassive,
            crafting_recipe::Column::ActionsRequired,
            crafting_recipe::Column::ToolMeshIndex,
            crafting_recipe::Column::RecipePerformanceId,
        ])
        .to_owned();

    let mut crafting_recipe_descs_to_delete = Vec::new();
    let err = while let Ok(value) = json_stream_reader.deserialize_next::<crafting_recipe::Model>()
    {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let crafting_recipe_descs_from_db = crafting_recipe::Entity::find()
                .filter(
                    crafting_recipe::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|crafting_recipe_desc| crafting_recipe_desc.id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if crafting_recipe_descs_from_db.len() != buffer_before_insert.len() {
                crafting_recipe_descs_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|crafting_recipe_desc| {
                            !crafting_recipe_descs_from_db.iter().any(
                                |crafting_recipe_desc_from_db| {
                                    crafting_recipe_desc_from_db.id == crafting_recipe_desc.id
                                },
                            )
                        })
                        .map(|crafting_recipe_desc| crafting_recipe_desc.id),
                );
            }

            let crafting_recipe_descs_from_db_map = crafting_recipe_descs_from_db
                .into_iter()
                .map(|crafting_recipe_desc| (crafting_recipe_desc.id, crafting_recipe_desc))
                .collect::<HashMap<i64, crafting_recipe::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|crafting_recipe_desc| {
                    match crafting_recipe_descs_from_db_map.get(&crafting_recipe_desc.id) {
                        Some(crafting_recipe_desc_from_db) => {
                            if crafting_recipe_desc_from_db != *crafting_recipe_desc {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|crafting_recipe_desc| crafting_recipe_desc.clone().into_active_model())
                .collect::<Vec<crafting_recipe::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} crafting_recipe_descs", things_to_insert.len());
            }

            for crafting_recipe_desc in &things_to_insert {
                let crafting_recipe_desc_in = crafting_recipe_descs_to_delete
                    .iter()
                    .position(|id| id == crafting_recipe_desc.id.as_ref());
                if crafting_recipe_desc_in.is_some() {
                    crafting_recipe_descs_to_delete.remove(crafting_recipe_desc_in.unwrap());
                }
            }

            let _ = crafting_recipe::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    };

    if buffer_before_insert.len() > 0 {
        let crafting_recipe_descs_from_db = crafting_recipe::Entity::find()
            .filter(
                crafting_recipe::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|crafting_recipe_desc| crafting_recipe_desc.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let crafting_recipe_descs_from_db_map = crafting_recipe_descs_from_db
            .into_iter()
            .map(|crafting_recipe_desc| (crafting_recipe_desc.id, crafting_recipe_desc))
            .collect::<HashMap<i64, crafting_recipe::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|crafting_recipe_desc| {
                match crafting_recipe_descs_from_db_map.get(&crafting_recipe_desc.id) {
                    Some(crafting_recipe_desc_from_db) => {
                        if crafting_recipe_desc_from_db != *crafting_recipe_desc {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|crafting_recipe_desc| crafting_recipe_desc.clone().into_active_model())
            .collect::<Vec<crafting_recipe::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} crafting_recipe_descs", things_to_insert.len());
            crafting_recipe::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("crafting_recipe_desc last batch imported");
    }
    info!(
        "Importing crafting_recipe_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if crafting_recipe_descs_to_delete.len() > 0 {
        info!(
            "crafting_recipe_desc's to delete: {:?}",
            crafting_recipe_descs_to_delete
        );
        crafting_recipe::Entity::delete_many()
            .filter(crafting_recipe::Column::Id.is_in(crafting_recipe_descs_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

// async fn get_known_crafting_recipe_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
//     let known_crafting_recipe_ids: Vec<i64> = crafting_recipe::Entity::find()
//         .select_only()
//         .column(crafting_recipe::Column::Id)
//         .into_tuple()
//         .all(conn)
//         .await?;

//     let known_crafting_recipe_ids = known_crafting_recipe_ids.into_iter().collect::<HashSet<i64>>();
//     Ok(known_crafting_recipe_ids)
// }
// async fn db_delete_crafting_recipes(
//     conn: &DatabaseConnection,
//     known_crafting_recipe_ids: HashSet<i64>,
// ) -> anyhow::Result<()> {
//     info!(
//         "crafting_recipe's ({}) to delete: {:?}",
//         known_crafting_recipe_ids.len(),
//         known_crafting_recipe_ids
//     );
//     crafting_recipe::Entity::delete_many()
//         .filter(crafting_recipe::Column::Id.is_in(known_crafting_recipe_ids))
//         .exec(conn)
//         .await?;
//     Ok(())
// }

// async fn db_insert_crafting_recipe_state(
//     conn: &DatabaseConnection,
//     buffer_before_insert: &mut Vec<Model>,
//     on_conflict: &OnConflict,
// ) -> anyhow::Result<()> {
//     let crafting_recipes_from_db = crafting_recipe::Entity::find()
//         .filter(
//             crafting_recipe::Column::Id.is_in(
//                 buffer_before_insert
//                     .iter()
//                     .map(|crafting_recipe| crafting_recipe.id)
//                     .collect::<Vec<i64>>(),
//             ),
//         )
//         .all(conn)
//         .await?;

//     let crafting_recipes_from_db_map = crafting_recipes_from_db
//         .into_iter()
//         .map(|recipe| (recipe.id, recipe))
//         .collect::<HashMap<i64, crafting_recipe::Model>>();

//     let things_to_insert = buffer_before_insert
//         .iter()
//         .filter(|recipe| {
//             match inventorys_from_db_map.get(&recipe.id) {
//                 Some(crafting_recipes_from_db) => {
//                     if crafting_recipes_from_db != *recipe {
//                         return true;
//                     }
//                 }
//                 None => {
//                     return true;
//                 }
//             }

//             return false;
//         })
//         .map(|recipe| recipe.clone().into_active_model())
//         .collect::<Vec<crafting_recipe::ActiveModel>>();

//     if things_to_insert.len() == 0 {
//         debug!("Nothing to insert");
//         buffer_before_insert.clear();
//         return Ok(());
//     } else {
//         debug!("Inserting {} crafting_recipes", things_to_insert.len());
//     }

//     let _ = crafting_recipe::Entity::insert_many(things_to_insert)
//         .on_conflict(on_conflict.clone())
//         .exec(conn)
//         .await?;

//     buffer_before_insert.clear();

//     Ok(())
// }

// pub(crate) async fn handle_initial_subscription(
//     database_connection: &DatabaseConnection,
//     table: &Table,
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
//     .update_columns([
//         crafting_recipe::Column::Name,
//         crafting_recipe::Column::TimeRequirement,
//         crafting_recipe::Column::StaminaRequirement,
//         crafting_recipe::Column::ToolDurabilityLost,
//         crafting_recipe::Column::BuildingRequirement,
//         crafting_recipe::Column::LevelRequirements,
//         crafting_recipe::Column::ToolRequirements,
//         crafting_recipe::Column::ConsumedItemStacks,
//         crafting_recipe::Column::DiscoveryTriggers,
//         crafting_recipe::Column::RequiredKnowledges,
//         crafting_recipe::Column::RequiredClaimTechId,
//         crafting_recipe::Column::FullDiscoveryScore,
//         crafting_recipe::Column::ExperiencePerProgress,
//         crafting_recipe::Column::AllowUseHands,
//         crafting_recipe::Column::CraftedItemStacks,
//         crafting_recipe::Column::IsPassive,
//         crafting_recipe::Column::ActionsRequired,
//         crafting_recipe::Column::ToolMeshIndex,
//         crafting_recipe::Column::RecipePerformanceId,
//     ])
//     .to_owned();

//     let chunk_size = Some(5000);
//     let mut buffer_before_insert: Vec<crafting_recipe::Model> = vec![];

//     let mut known_inventory_ids = get_known_crafting_recipe_ids(database_connection).await?;

//     for row in table.inserts.iter() {
//         match serde_json::from_str::<crafting_recipe::Model>(row.text.as_ref()) {
//             Ok(crafting_recipe_state) => {
//                 if known_inventory_ids.contains(&crafting_recipe_state.id) {
//                     known_inventory_ids.remove(&crafting_recipe_state.id);
//                 }
//                 buffer_before_insert.push(crafting_recipe_state);
//                 if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
//                     db_insert_crafting_recipe_state(
//                         database_connection,
//                         &mut buffer_before_insert,
//                         &on_conflict,
//                     )
//                     .await?;
//                 }
//             }
//             Err(error) => {
//                 error!("InitialSubscription Insert Inventory Error: {error}");
//             }
//         }
//     }

//     if buffer_before_insert.len() > 0 {
//         for buffer_chnk in buffer_before_insert.chunks(5000) {
//             db_insert_crafting_recipe_state(database_connection, &mut buffer_chnk.to_vec(), &on_conflict)
//                 .await?;
//         }
//     }

//     if known_inventory_ids.len() > 0 {
//         db_delete_crafting_recipes(database_connection, known_inventory_ids).await?;
//     }

//     Ok(())
// }

// pub(crate) async fn handle_transaction_update(
//     database_connection: &DatabaseConnection,
//     tables: &Vec<TableWithOriginalEventTransactionUpdate>,
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
//         .update_columns([
//             crafting_recipe::Column::Name,
//             crafting_recipe::Column::TimeRequirement,
//             crafting_recipe::Column::StaminaRequirement,
//             crafting_recipe::Column::ToolDurabilityLost,
//             crafting_recipe::Column::BuildingRequirement,
//             crafting_recipe::Column::LevelRequirements,
//             crafting_recipe::Column::ToolRequirements,
//             crafting_recipe::Column::ConsumedItemStacks,
//             crafting_recipe::Column::DiscoveryTriggers,
//             crafting_recipe::Column::RequiredKnowledges,
//             crafting_recipe::Column::RequiredClaimTechId,
//             crafting_recipe::Column::FullDiscoveryScore,
//             crafting_recipe::Column::ExperiencePerProgress,
//             crafting_recipe::Column::AllowUseHands,
//             crafting_recipe::Column::CraftedItemStacks,
//             crafting_recipe::Column::IsPassive,
//             crafting_recipe::Column::ActionsRequired,
//             crafting_recipe::Column::ToolMeshIndex,
//             crafting_recipe::Column::RecipePerformanceId,
//         ])
//         .to_owned();

//     let mut buffer_before_insert = HashMap::new();
//     let mut potential_deletes = HashSet::new();
//     // let mut inventory_changes = vec![];

//     for p1 in tables.iter() {
//         let event_type = if p1.inserts.len() > 0 && p1.deletes.len() > 0 {
//             "update"
//         } else if p1.inserts.len() > 0 && p1.deletes.len() == 0 {
//             "insert"
//         } else if p1.deletes.len() > 0 && p1.inserts.len() == 0 {
//             "delete"
//         } else {
//             "unknown"
//         };

//         if event_type == "unknown" {
//             error!("Unknown event type {:?}", p1);
//             continue;
//         }

//         if event_type == "delete" {
//             for row in p1.deletes.iter() {
//                 match serde_json::from_str::<crafting_recipe::Model>(row.text.as_ref()) {
//                     Ok(crafting_recipe) => {
//                         potential_deletes.insert(crafting_recipe.id);
//                     }
//                     Err(error) => {
//                         error!("Event: {event_type} Error: {error} for row: {:?}", row.text);
//                     }
//                 }
//             }
//         } else if event_type == "update" {
//             let mut delete_parsed = HashMap::new();
//             for row in p1.deletes.iter() {
//                 let parsed = serde_json::from_str::<crafting_recipe::Model>(row.text.as_ref());

//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse delete crafting_recipe: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row.text
//                     );
//                 } else {
//                     let parsed = parsed.unwrap();
//                     delete_parsed.insert(parsed.entity_id, parsed.clone());
//                     potential_deletes.remove(&parsed.entity_id);
//                 }
//             }

//             for row in p1.inserts.iter().enumerate() {
//                 let parsed = serde_json::from_str::<crafting_recipe::Model>(row.1.text.as_ref());

//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse insert crafting_recipe: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row.1.text
//                     );
//                     continue;
//                 }

//                 let parsed = parsed.unwrap();
//                 let id = parsed.id;

//                 match (parsed, delete_parsed.get(&id)) {
//                     (new_inventory, Some(_old_inventory)) => {
//                         potential_deletes.remove(&new_inventory.id);
//                         buffer_before_insert.insert(new_inventory.id, new_inventory);
//                     }
//                     (new_inventory, None) => {
//                         potential_deletes.remove(&new_inventory.id);
//                         buffer_before_insert.insert(new_inventory.id, new_inventory);
//                     }
//                 }
//             }
//         } else if event_type == "insert" {
//         } else {
//             error!("Unknown event type {:?}", p1);
//             continue;
//         }
//     }

//     if buffer_before_insert.len() > 0 {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<crafting_recipe::Model>>();
//         db_insert_crafting_recipe_state(
//             database_connection,
//             &mut buffer_before_insert_vec,
//             &on_conflict,
//         )
//         .await?;
//         buffer_before_insert.clear();
//     }

//     if potential_deletes.len() > 0 {
//         db_delete_crafting_recipes(database_connection, potential_deletes).await?;
//     }

//     Ok(())
// }
