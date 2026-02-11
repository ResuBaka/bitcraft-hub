//! `SeaORM` Entity, `permission_state`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "permission_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub ordained_entity_id: i64,
    pub allowed_entity_id: i64,
    pub group: i32,
    pub rank: i32,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    ordained_entity_id: i64,
    allowed_entity_id: i64,
    group: i32,
    rank: i32,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::PermissionState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            ordained_entity_id: value.ordained_entity_id as i64,
            allowed_entity_id: value.allowed_entity_id as i64,
            group: value.group,
            rank: value.rank,
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
            ordained_entity_id: self.ordained_entity_id,
            allowed_entity_id: self.allowed_entity_id,
            group: self.group,
            rank: self.rank,
            region: self.region,
        }
    }
}
