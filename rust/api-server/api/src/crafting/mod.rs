pub(crate) mod bitcraft;

use crate::AppRouter;
use axum::Router;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
}
