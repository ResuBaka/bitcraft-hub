use super::shared::location;
use game_module::module_bindings::ClaimLocalState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[ts(rename = "ClaimLocalState")]
#[sea_orm(table_name = "claim_local_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub supplies: i32,
    pub building_maintenance: f32,
    pub num_tiles: i32,
    pub num_tile_neighbors: i32,
    #[sea_orm(column_type = "Json")]
    pub location: Option<location::Location>,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub supplies_purchase_threshold: i32,
    pub supplies_purchase_price: f32,
    pub building_description_id: i32,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    supplies: i32,
    building_maintenance: f32,
    num_tiles: i32,
    num_tile_neighbors: i32,
    location: Option<location::Location>,
    treasury: i32,
    xp_gained_since_last_coin_minting: i32,
    supplies_purchase_threshold: i32,
    supplies_purchase_price: f32,
    building_description_id: i32,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: ClaimLocalState) -> Self {
        ModelBuilder {
            entity_id: value.entity_id as i64,
            supplies: value.supplies,
            building_maintenance: value.building_maintenance,
            num_tiles: value.num_tiles,
            num_tile_neighbors: value.num_tile_neighbors as i32,
            treasury: value.treasury as i32,
            location: value.location.map(|location| location.into()),
            xp_gained_since_last_coin_minting: value.xp_gained_since_last_coin_minting as i32,
            supplies_purchase_threshold: value.supplies_purchase_threshold as i32,
            supplies_purchase_price: value.supplies_purchase_price,
            building_description_id: value.building_description_id,
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
            supplies: self.supplies,
            building_maintenance: self.building_maintenance,
            num_tiles: self.num_tiles,
            num_tile_neighbors: self.num_tile_neighbors,
            location: self.location,
            treasury: self.treasury,
            xp_gained_since_last_coin_minting: self.xp_gained_since_last_coin_minting,
            supplies_purchase_threshold: self.supplies_purchase_threshold,
            supplies_purchase_price: self.supplies_purchase_price,
            building_description_id: self.building_description_id,
            region: self.region,
        }
    }
}
