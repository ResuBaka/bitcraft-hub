use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Router;
use axum_codec::Codec;
use entity::crafting_recipe;
use entity::crafting_recipe::ConsumedItemStackWithInner;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use serde_json::Value;
use service::Query as QueryCore;
use std::fs::File;
use std::path::PathBuf;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/bitcraft/recipes/needed_in_crafting/:id",
            axum_codec::routing::get(get_needed_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/produced_in_crafting/:id",
            axum_codec::routing::get(get_produced_in_crafting).into(),
        )
        .route(
            "/api/bitcraft/recipes/needed_to_craft/:id",
            axum_codec::routing::get(get_needed_to_craft).into(),
        )
        .route(
            "/recipes/needed_in_crafting/:id",
            axum_codec::routing::get(get_needed_in_crafting).into(),
        )
        .route(
            "/recipes/produced_in_crafting/:id",
            axum_codec::routing::get(get_produced_in_crafting).into(),
        )
        .route(
            "/recipes/needed_to_craft/:id",
            axum_codec::routing::get(get_needed_to_craft).into(),
        )
}

pub(crate) async fn get_needed_in_crafting(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
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

    Ok(Codec(recipes))
}

pub(crate) async fn get_produced_in_crafting(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<Vec<crafting_recipe::Model>>, (StatusCode, &'static str)> {
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

    Ok(Codec(recipes))
}

pub(crate) async fn get_needed_to_craft(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<Vec<Vec<ConsumedItemStackWithInner>>>, (StatusCode, &'static str)> {
    let recipes = QueryCore::load_all_recipes(&state.conn).await;

    let recipes = recipes.into_iter().map(|x| x.into()).collect();

    Ok(Codec(get_all_consumed_items_from_item(&recipes, id as i64)))
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

    return list;
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

    return item.consumed_item_stacks.clone();
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

pub(crate) async fn import_recipes(
    conn: &DatabaseConnection,
    storage_path: &PathBuf,
) -> anyhow::Result<()> {
    let item_file = File::open(storage_path.join("Desc/CraftingRecipeDesc.json")).unwrap();
    let crafting_recipes: Value = serde_json::from_reader(&item_file).unwrap();
    let crafting_recipes: Vec<crafting_recipe::Model> = serde_json::from_value(
        crafting_recipes
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let count = crafting_recipes.len();
    let db_count = crafting_recipe::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let item: Vec<crafting_recipe::ActiveModel> = crafting_recipes
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for item in item.chunks(5000) {
        let _ = crafting_recipe::Entity::insert_many(item.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
