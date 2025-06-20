use game_module::module_bindings;
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::shared::surface_type::SurfaceType;

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Deserialize, Serialize, TS)]
pub struct MovementSpeed {
    pub surface_type: SurfaceType,
    pub speed: f32,
}

impl From<module_bindings::MovementSpeed> for MovementSpeed {
    fn from(value: module_bindings::MovementSpeed) -> Self {
        MovementSpeed {
            surface_type: value.surface_type.into(),
            speed: value.speed,
        }
    }
}
