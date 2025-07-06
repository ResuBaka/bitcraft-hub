use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use entity::{cargo_desc, crafting_recipe, item_desc, item_list_desc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    Router::new().route("/recipes/get_all", axum_codec::routing::get(get_all).into())
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct RecipesAllResponse {
    recipes: HashMap<i32, crafting_recipe::Model>,
    cargo_desc: HashMap<i32, cargo_desc::Model>,
    item_desc: HashMap<i32, item_desc::Model>,
    item_list_desc: HashMap<i32, item_list_desc::Model>,
}
pub(crate) async fn get_all(
    state: State<AppState>,
) -> Result<axum_codec::Codec<RecipesAllResponse>, (StatusCode, &'static str)> {
    Ok(axum_codec::Codec(RecipesAllResponse {
        recipes: state
            .crafting_recipe_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
        cargo_desc: state
            .cargo_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
        item_desc: state
            .item_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
        item_list_desc: state
            .item_list_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
    }))
}
