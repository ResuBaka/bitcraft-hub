use game_module::module_bindings;
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Deserialize, Serialize, TS)]

pub enum SurfaceType {
    Ground,
    Lake,
    River,
    Ocean,
    OceanBiome,
    Swamp,
}

impl From<module_bindings::SurfaceType> for SurfaceType {
    fn from(value: module_bindings::SurfaceType) -> Self {
        match value {
            module_bindings::SurfaceType::Ground => SurfaceType::Ground,
            module_bindings::SurfaceType::Lake => SurfaceType::Lake,
            module_bindings::SurfaceType::River => SurfaceType::River,
            module_bindings::SurfaceType::Ocean => SurfaceType::Ocean,
            module_bindings::SurfaceType::OceanBiome => SurfaceType::OceanBiome,
            module_bindings::SurfaceType::Swamp => SurfaceType::Swamp,
        }
    }
}
