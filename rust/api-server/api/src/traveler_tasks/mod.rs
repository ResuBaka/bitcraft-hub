use std::collections::HashMap;

use axum::{Router, extract::State};
use entity::{npc_desc, traveler_task_desc};
use hyper::StatusCode;

use crate::{AppRouter, AppState};

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/traveler_tasks", axum_codec::routing::get(get_all).into())
        .route("/npc", axum_codec::routing::get(get_npc_all).into())
}

pub(crate) async fn get_all(
    state: State<AppState>,
) -> Result<axum_codec::Codec<HashMap<i32, traveler_task_desc::Model>>, (StatusCode, &'static str)>
{
    Ok(axum_codec::Codec(
        state
            .traveler_task_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
    ))
}

pub(crate) async fn get_npc_all(
    state: State<AppState>,
) -> Result<axum_codec::Codec<HashMap<i32, npc_desc::Model>>, (StatusCode, &'static str)> {
    Ok(axum_codec::Codec(
        state
            .npc_desc
            .iter()
            .map(|value| (*value.key(), value.clone()))
            .collect(),
    ))
}
