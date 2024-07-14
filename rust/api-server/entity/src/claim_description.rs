//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "claim_description")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: u64,
    pub owner_player_entity_id: u64,
    pub owner_building_entity_id: u64,
    pub name: String,
    #[sea_orm(column_type = "Float")]
    pub supplies: f32,
    #[sea_orm(column_type = "Float")]
    pub building_maintenance: f32,
    pub members: Json,
    pub tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: Json,
    pub treasury: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
