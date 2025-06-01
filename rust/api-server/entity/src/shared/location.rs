use game_module::module_bindings::OffsetCoordinatesSmallMessage;
use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, Default,
)]
pub struct Location {
    pub x: i32,
    pub z: i32,
    pub dimension: u32,
}

impl From<OffsetCoordinatesSmallMessage> for crate::shared::location::Location {
    fn from(value: OffsetCoordinatesSmallMessage) -> Self {
        crate::shared::location::Location {
            x: value.x,
            z: value.z,
            dimension: value.dimension,
        }
    }
}
