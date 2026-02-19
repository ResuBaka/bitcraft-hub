//! `SeaORM` Entity, `player_housing_state`

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "player_housing_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub entrance_building_entity_id: i64,
    pub network_entity_id: i64,
    pub exit_portal_entity_id: i64,
    pub rank: i32,
    pub locked_until: i64,
    pub is_empty: bool,
    pub region_index: i32,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "super::permission_state::Entity",
        from = "Column::EntranceBuildingEntityId",
        to = "super::permission_state::Column::OrdainedEntityId"
    )]
    Permissions,
}

impl Related<super::permission_state::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Permissions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    entrance_building_entity_id: i64,
    network_entity_id: i64,
    exit_portal_entity_id: i64,
    rank: i32,
    locked_until: i64,
    is_empty: bool,
    region_index: i32,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::PlayerHousingState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            entrance_building_entity_id: value.entrance_building_entity_id as i64,
            network_entity_id: value.network_entity_id as i64,
            exit_portal_entity_id: value.exit_portal_entity_id as i64,
            rank: value.rank,
            locked_until: value
                .locked_until
                .to_duration_since_unix_epoch()
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0),
            is_empty: value.is_empty,
            region_index: value.region_index as i32,
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
            entrance_building_entity_id: self.entrance_building_entity_id,
            network_entity_id: self.network_entity_id,
            exit_portal_entity_id: self.exit_portal_entity_id,
            rank: self.rank,
            locked_until: self.locked_until,
            is_empty: self.is_empty,
            region_index: self.region_index,
            region: self.region,
        }
    }
}
