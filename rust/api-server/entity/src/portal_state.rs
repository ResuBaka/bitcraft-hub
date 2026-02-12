//! `SeaORM` Entity, `portal_state`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "portal_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub target_building_entity_id: i64,
    pub destination_x: i32,
    pub destination_z: i32,
    pub destination_dimension: i32,
    pub enabled: bool,
    pub allow_deployables: bool,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    target_building_entity_id: i64,
    destination_x: i32,
    destination_z: i32,
    destination_dimension: i32,
    enabled: bool,
    allow_deployables: bool,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::PortalState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            target_building_entity_id: value.target_building_entity_id as i64,
            destination_x: value.destination_x,
            destination_z: value.destination_z,
            destination_dimension: value.destination_dimension as i32,
            enabled: value.enabled,
            allow_deployables: value.allow_deployables,
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
            target_building_entity_id: self.target_building_entity_id,
            destination_x: self.destination_x,
            destination_z: self.destination_z,
            destination_dimension: self.destination_dimension,
            enabled: self.enabled,
            allow_deployables: self.allow_deployables,
            region: self.region,
        }
    }
}
