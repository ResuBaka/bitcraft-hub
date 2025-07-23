use game_module::module_bindings::ClaimState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[ts(rename = "ClaimState")]
#[sea_orm(table_name = "claim_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub neutral: bool,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    owner_player_entity_id: i64,
    owner_building_entity_id: i64,
    name: String,
    neutral: bool,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: ClaimState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            owner_player_entity_id: value.owner_player_entity_id as i64,
            owner_building_entity_id: value.owner_building_entity_id as i64,
            name: value.name,
            neutral: value.neutral,
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
            owner_player_entity_id: self.owner_player_entity_id,
            owner_building_entity_id: self.owner_building_entity_id,
            name: self.name,
            neutral: self.neutral,
            region: self.region,
        }
    }
}
