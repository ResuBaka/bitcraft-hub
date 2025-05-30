//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "item_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub name: String,
    pub description: String,
    pub volume: i32,
    pub durability: i32,
    pub convert_to_on_durability_zero: i32,
    pub secondary_knowledge_id: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub tier: i32,
    pub tag: String,
    pub rarity: Json,
    pub compendium_entry: bool,
    pub item_list_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
