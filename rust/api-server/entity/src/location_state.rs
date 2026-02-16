//! `SeaORM` Entity, `location_state`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[sea_orm(table_name = "location_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub chunk_index: i64,
    pub x: i64,
    pub z: i64,
    pub dimension: i64,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    chunk_index: i64,
    x: i64,
    z: i64,
    dimension: i64,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::LocationState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            chunk_index: value.chunk_index as i64,
            x: value.x as i64,
            z: value.z as i64,
            dimension: value.dimension as i64,
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
            chunk_index: self.chunk_index,
            x: self.x,
            z: self.z,
            dimension: self.dimension,
            region: self.region,
        }
    }
}
