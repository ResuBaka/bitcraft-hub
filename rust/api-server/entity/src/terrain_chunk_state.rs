use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use game_module::module_bindings::TerrainChunkState;
use sea_orm::{FromJsonQueryResult, JsonValue};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "terrain_chunk_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chunk_index: i64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub dimension: i32,
    #[sea_orm(column_type = "Json")]
    pub biomes: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub biome_density: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub elevations: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub water_levels: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub water_body_types: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub zoning_types: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub original_elevations: JsonValue,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<TerrainChunkState> for Model {
    fn from(value: TerrainChunkState) -> Self {
        Model {
            chunk_index: value.chunk_index as i64,
            chunk_x: value.chunk_x,
            chunk_z: value.chunk_z,
            dimension: value.dimension as i32,
            biomes: serde_json::to_value(value.biomes).unwrap(),
            biome_density: serde_json::to_value(value.biome_density).unwrap(),
            elevations: serde_json::to_value(value.elevations).unwrap(),
            water_levels: serde_json::to_value(value.water_levels).unwrap(),
            water_body_types: serde_json::to_value(value.water_body_types).unwrap(),
            zoning_types: serde_json::to_value(value.zoning_types).unwrap(),
            original_elevations: serde_json::to_value(value.original_elevations).unwrap(),
        }
    }
}
