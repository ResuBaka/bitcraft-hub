use axum::extract::{Query, State};
use tower_cookies::Cookies;
use axum::Json;
use serde_json::{json, Value};
use axum::http::StatusCode;
use service::Query as QueryCore;
use crate::{AppState, Params};

pub(crate) async fn list_locations(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
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