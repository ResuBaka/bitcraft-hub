//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "building_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_type = "Json")]
    pub functions: Vec<Function>,
    pub name: String,
    pub description: String,
    pub rested_buff_duration: i32,
    pub light_radius: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub unenterable: bool,
    pub wilderness: bool,
    pub footprint: Json,
    pub max_health: i32,
    pub defense_level: i32,
    pub decay: f32,
    pub maintenance: f32,
    pub build_permission: Json,
    pub interact_permission: Json,
    pub has_action: bool,
    pub show_in_compendium: bool,
    pub is_ruins: bool,
    pub not_deconstructible: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Function {
    pub function_type: i32,
    pub level: i32,
    pub crafting_slots: i32,
    pub storage_slots: i32,
    pub cargo_slots: i32,
    pub refining_slots: i32,
    pub refining_cargo_slots: i32,
    pub item_slot_size: i32,
    pub cargo_slot_size: i32,
    pub trade_orders: i32,
    pub allowed_item_id_per_slot: Vec<i32>,
    pub buff_ids: Vec<i32>,
    pub concurrent_crafts_per_player: i32,
    pub terraform: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponse {
    pub id: i64,
    pub functions: Vec<Function>,
    pub name: String,
    pub description: String,
    pub rested_buff_duration: i32,
    pub light_radius: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub unenterable: bool,
    pub wilderness: bool,
    pub footprint: Json,
    pub max_health: i32,
    pub decay: f32,
    pub maintenance: f32,
    pub has_action: bool,
    pub show_in_compendium: bool,
    pub is_ruins: bool,
    pub count: i32,
}
