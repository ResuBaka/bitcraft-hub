//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use sea_orm::{FromJsonQueryResult, JsonValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade_order")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub building_entity_id: i64,
    pub remaining_stock: i64,
    #[sea_orm(column_type = "Json")]
    pub offer_items: Vec<TradeOrderOfferItem>,
    pub offer_cargo_id: JsonValue,
    #[sea_orm(column_type = "Json")]
    pub required_items: Vec<TradeOrderOfferItem>,
    pub required_cargo_id: JsonValue,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Deserialize, Serialize)]
pub struct TradeOrderOfferItem {
    pub item_id: i64,
    pub quantity: i64,
    pub item_type: JsonValue,
    pub durability: JsonValue,
}
