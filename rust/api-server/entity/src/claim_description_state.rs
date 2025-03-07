//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::{FromJsonQueryResult, entity::prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Member {
    pub player_entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "claim_description_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub supplies: i64,
    #[sea_orm(column_type = "Float")]
    pub building_maintenance: f32,
    #[sea_orm(column_type = "Json")]
    pub members: Vec<Member>,
    pub num_tiles: i32,
    pub num_tile_neighbors: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: Json,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub supplies_purchase_threshold: i32,
    #[sea_orm(column_type = "Float")]
    pub supplies_purchase_price: f32,
    pub building_description_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
