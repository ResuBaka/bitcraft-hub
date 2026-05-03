//! `SeaORM` Entity, `portal_state`

use crate::shared::timestamp;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "progressive_action_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub building_entity_id: i64,
    pub function_type: i32,
    pub progress: i32,
    pub recipe_id: i32,
    pub craft_count: i32,
    pub last_crit_outcome: i32,
    pub owner_entity_id: i64,
    #[sea_orm(column_type = "JsonBinary")]
    pub lock_expiration: timestamp::Timestamp,
    pub preparation: bool,
    pub region: crate::shared::Region,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    building_entity_id: i64,
    function_type: i32,
    progress: i32,
    recipe_id: i32,
    craft_count: i32,
    last_crit_outcome: i32,
    owner_entity_id: i64,
    lock_expiration: timestamp::Timestamp,
    preparation: bool,
    region: crate::shared::Region,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::ProgressiveActionState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            building_entity_id: value.building_entity_id as i64,
            function_type: value.function_type,
            progress: value.progress,
            recipe_id: value.recipe_id,
            craft_count: value.craft_count,
            last_crit_outcome: value.last_crit_outcome,
            owner_entity_id: value.owner_entity_id as i64,
            lock_expiration: value.lock_expiration.into(),
            preparation: value.preparation,
            region: 0,
        }
    }

    pub fn with_region(mut self, region: crate::shared::Region) -> Self {
        self.region = region;
        self
    }

    pub fn build(self) -> Model {
        Model {
            entity_id: self.entity_id,
            building_entity_id: self.building_entity_id,
            function_type: self.function_type,
            progress: self.progress,
            recipe_id: self.recipe_id,
            craft_count: self.craft_count,
            last_crit_outcome: self.last_crit_outcome,
            owner_entity_id: self.owner_entity_id,
            lock_expiration: self.lock_expiration,
            preparation: self.preparation,
            region: self.region,
        }
    }
}
