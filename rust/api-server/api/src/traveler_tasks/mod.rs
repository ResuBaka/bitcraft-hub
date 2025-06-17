use std::collections::HashMap;

use axum::{extract::State, Router};
use entity::traveler_task_desc;
use hyper::StatusCode;

use crate::{AppRouter, AppState};


pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/traveler_tasks", axum_codec::routing::get(get_all).into())
}


pub(crate) async fn get_all(
    state: State<std::sync::Arc<AppState>>,
    
) -> Result<axum_codec::Codec<HashMap<i32,traveler_task_desc::Model>>, (StatusCode, &'static str)> {

    return Ok(axum_codec::Codec(state 
            .traveler_task_desc
            .iter()
            .map(|value| (value.key().clone(), value.clone()))
            .collect()))
}