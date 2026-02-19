pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use service::Query as QueryCore;
use ts_rs::TS;

pub(crate) fn get_routes() -> AppRouter {
    axum::Router::new().route(
        "/api/bitcraft/extractionRecipes/all",
        axum_codec::routing::get(get_all).into(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct ExtractionRecipeResponse {
    pub id: i32,
    pub resource_id: i32,
    pub extracted_item_stacks:
        Vec<entity::shared::probabilistic_item_stack::ProbabilisticItemStack>,
    pub tool_requirements: Vec<entity::crafting_recipe::ToolRequirement>,
    pub allow_use_hands: bool,
    pub time_requirement: f32,
    pub stamina_requirement: f32,
}

pub(crate) async fn get_all(
    state: State<AppState>,
) -> Result<axum_codec::Codec<Vec<ExtractionRecipeResponse>>, (StatusCode, &'static str)> {
    let recipes = QueryCore::all_extraction_recipe_desc(&state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot find extraction recipes",
            )
        })?;

    let response: Vec<ExtractionRecipeResponse> = recipes
        .into_iter()
        .map(|recipe| ExtractionRecipeResponse {
            id: recipe.id,
            resource_id: recipe.resource_id,
            extracted_item_stacks: recipe.extracted_item_stacks,
            tool_requirements: recipe.tool_requirements,
            allow_use_hands: recipe.allow_use_hands,
            time_requirement: recipe.time_requirement,
            stamina_requirement: recipe.stamina_requirement,
        })
        .collect();

    Ok(axum_codec::Codec(response))
}
