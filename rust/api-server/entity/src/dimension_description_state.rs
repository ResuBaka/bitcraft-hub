//! `SeaORM` Entity, `dimension_description_state`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "dimension_description_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub dimension_network_entity_id: i64,
    pub collapse_timestamp: i64,
    pub interior_instance_id: i32,
    pub dimension_position_large_x: i64,
    pub dimension_position_large_z: i64,
    pub dimension_size_large_x: i64,
    pub dimension_size_large_z: i64,
    pub dimension_id: i64,
    pub dimension_type: i32,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    dimension_network_entity_id: i64,
    collapse_timestamp: i64,
    interior_instance_id: i32,
    dimension_position_large_x: i64,
    dimension_position_large_z: i64,
    dimension_size_large_x: i64,
    dimension_size_large_z: i64,
    dimension_id: i64,
    dimension_type: i32,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::DimensionDescriptionState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            dimension_network_entity_id: value.dimension_network_entity_id as i64,
            collapse_timestamp: value.collapse_timestamp as i64,
            interior_instance_id: value.interior_instance_id,
            dimension_position_large_x: value.dimension_position_large_x as i64,
            dimension_position_large_z: value.dimension_position_large_z as i64,
            dimension_size_large_x: value.dimension_size_large_x as i64,
            dimension_size_large_z: value.dimension_size_large_z as i64,
            dimension_id: value.dimension_id as i64,
            dimension_type: match value.dimension_type {
                game_module::module_bindings::DimensionType::Overworld => 0,
                game_module::module_bindings::DimensionType::BuildingInterior => 1,
                game_module::module_bindings::DimensionType::AncientRuin => 2,
                game_module::module_bindings::DimensionType::Dungeon => 3,
                game_module::module_bindings::DimensionType::Unknown => 4,
            },
            region: String::new(),
        }
    }

    pub fn with_region(mut self, region: String) -> Self {
        self.region = region;
        self
    }

    pub fn build(self) -> Model {
        Model {
            entity_id: self.entity_id,
            dimension_network_entity_id: self.dimension_network_entity_id,
            collapse_timestamp: self.collapse_timestamp,
            interior_instance_id: self.interior_instance_id,
            dimension_position_large_x: self.dimension_position_large_x,
            dimension_position_large_z: self.dimension_position_large_z,
            dimension_size_large_x: self.dimension_size_large_x,
            dimension_size_large_z: self.dimension_size_large_z,
            dimension_id: self.dimension_id,
            dimension_type: self.dimension_type,
            region: self.region,
        }
    }
}
