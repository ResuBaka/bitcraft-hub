//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cargo_description")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: u64,
    pub name: String,
    pub description: String,
    pub volume: i32,
    pub secondary_knowledge_id: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub carried_model_asset_name: String,
    pub pick_up_animation_start: String,
    pub pick_up_animation_end: String,
    pub drop_animation_start: String,
    pub drop_animation_end: String,
    #[sea_orm(column_type = "Float")]
    pub pick_up_time: f32,
    #[sea_orm(column_type = "Float")]
    pub place_time: f32,
    pub animator_state: String,
    #[sea_orm(column_type = "Float")]
    pub movement_modifier: f32,
    pub blocks_path: bool,
    pub on_destroy_yield_cargos: Json,
    #[sea_orm(column_type = "Float")]
    pub despawn_time: f32,
    pub tier: i32,
    pub tag: String,
    pub rarity: Json,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
