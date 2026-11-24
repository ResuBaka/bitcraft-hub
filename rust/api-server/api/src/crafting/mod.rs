pub(crate) mod bitcraft;
pub(crate) mod events;
pub(crate) mod passive;
pub(crate) mod claim;
pub(crate) mod notifications;
pub(crate) mod public_progressive_action;
pub(crate) mod station_projects;

use crate::AppRouter;
use axum::Router;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
}
