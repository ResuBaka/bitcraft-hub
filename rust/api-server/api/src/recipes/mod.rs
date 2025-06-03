#![allow(warnings)]

use crate::config::Config;
use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use entity::crafting_recipe;
use entity::crafting_recipe::ConsumedItemStackWithInner;
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/api/bitcraft/recipes/needed_in_crafting/{id}",
            axum_codec::routing::get(get_needed_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/produced_in_crafting/{id}",
            axum_codec::routing::get(get_produced_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/needed_to_craft/{id}",
            axum_codec::routing::get(get_needed_to_craft).into(),
        )
        .route(
            "/recipes/needed_in_crafting/{id}",
            axum_codec::routing::get(get_needed_in_crafting).into(),
        )
        .route(
            "/recipes/produced_in_crafting/{id}",
            axum_codec::routing::get(get_produced_in_crafting).into(),
        )
        .route(
            "/recipes/needed_to_craft/{id}",
            axum_codec::routing::get(get_needed_to_craft).into(),
        )
}

pub(crate) async fn get_needed_in_crafting(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
    return Ok(axum_codec::Codec(vec![]));

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

    Ok(axum_codec::Codec(recipes))
}

pub(crate) async fn get_produced_in_crafting(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
    return Ok(axum_codec::Codec(vec![]));
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

    Ok(axum_codec::Codec(recipes))
}

pub(crate) async fn get_needed_to_craft(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<axum_codec::Codec<Vec<Vec<ConsumedItemStackWithInner>>>, (StatusCode, &'static str)> {
    if state.crafting_recipe_desc.is_empty() {
        let recipes = QueryCore::load_all_recipes(&state.conn).await;

        for recipe in recipes {
            state.crafting_recipe_desc.insert(recipe.id, recipe);
        }
    }
    let recipes: Vec<crafting_recipe::CraftingRecipeWithInner> = state
        .crafting_recipe_desc
        .iter()
        .map(|x| x.to_owned().into())
        .collect();

    Ok(axum_codec::Codec(get_all_consumed_items_from_item(
        &recipes, id as i64,
    )))
}

fn get_all_consumed_items_from_item(
    rows: &Vec<crafting_recipe::CraftingRecipeWithInner>,
    item_id: i64,
) -> Vec<Vec<ConsumedItemStackWithInner>> {
    let posibilities = rows.iter().filter(|recipe| {
        recipe
            .crafted_item_stacks
            .iter()
            .any(|cis| cis.item_id == item_id)
    });

    let mut list = Vec::new();
    for posibilitie in posibilities {
        list.push(get_all_consumed_items_from_stack(
            rows,
            &mut posibilitie.clone(),
            vec![posibilitie.id],
        ));
    }

    list
}

fn get_all_consumed_items_from_stack(
    rows: &Vec<crafting_recipe::CraftingRecipeWithInner>,
    item: &mut crafting_recipe::CraftingRecipeWithInner,
    already_used: Vec<i64>,
) -> Vec<ConsumedItemStackWithInner> {
    for itemstack in item.consumed_item_stacks.iter_mut() {
        let mut posibilities = rows
            .iter()
            .filter(|recipe| {
                recipe
                    .crafted_item_stacks
                    .iter()
                    .any(|cis| cis.item_id == itemstack.item_id)
            })
            .collect::<Vec<&crafting_recipe::CraftingRecipeWithInner>>();

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
                temp,
            ));
        }
        itemstack.inner = Some(list);
    }

    item.consumed_item_stacks.clone()
}

// export function getAllConsumedItemsFromItem(
//   rows: CraftingRecipeRow[],
//   item_id: number,
// ): ItemStackWithInner[][] {
//   const posibilities = rows.filter(
//     (recipe) =>
//       recipe.crafted_item_stacks.filter((cis) => {
//         return cis.item_id == item_id;
//       }).length > 0,
//   );
//
//   const list: ItemStackWithInner[][] = [];
//
//   for (const posibilitie of posibilities) {
//     list.push(
//       getAllConsumedItemsFromStack(rows, posibilitie, [posibilitie.id]),
//     );
//   }
//
//   return list;
// }

#[allow(dead_code)]
pub(crate) async fn load_crafting_recipe_desc_from_file(
    storage_path: &std::path::Path,
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
        .post(format!("{protocol}{domain}/v1/database/{database}/sql"))
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

pub async fn import_job_recipes_desc(temp_config: Config) {
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

            if things_to_insert.is_empty() {
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

    if !buffer_before_insert.is_empty() {
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

        if things_to_insert.is_empty() {
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

    if !crafting_recipe_descs_to_delete.is_empty() {
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
