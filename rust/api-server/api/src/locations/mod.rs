use crate::{AppState, Params};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use service::Query as QueryCore;

pub(crate) async fn list_locations(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);

    let (posts, num_pages) = QueryCore::find_locations(&state.conn, page, posts_per_page)
        .await
        .expect("Cannot find posts in page");

    Ok(Json(json!({
        "posts": posts,
        "perPage": posts_per_page,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}
