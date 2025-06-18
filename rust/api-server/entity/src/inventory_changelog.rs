use chrono::{DateTime, Utc};
use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[ts(export)]
pub enum ItemType {
    Item = 0,
    Cargo = 1,
}

impl From<game_module::module_bindings::ItemType> for ItemType {
    fn from(value: game_module::module_bindings::ItemType) -> Self {
        match &value {
            game_module::module_bindings::ItemType::Cargo => ItemType::Cargo,
            game_module::module_bindings::ItemType::Item => ItemType::Item,
        }
    }
}
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[ts(rename = "InventoryChangelog")]
#[sea_orm(table_name = "inventory_changelog")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub entity_id: i64,
    pub user_id: Option<i64>,
    pub pocket_number: i32,
    pub old_item_type: Option<ItemType>,
    pub old_item_id: Option<i32>,
    pub old_item_quantity: Option<i32>,
    pub new_item_type: Option<ItemType>,
    pub new_item_id: Option<i32>,
    pub new_item_quantity: Option<i32>,
    pub type_of_change: TypeOfChange,
    pub timestamp: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum TypeOfChange {
    Add = 0,
    Remove = 1,
    Update = 2,
    AddAndRemove = 3,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub enum InventoryChangelogEventData {
    Update(UpdateEvent),
    Remove(RemoveEvent),
    Add(AddEvent),
    Swap(SwapEvent),
    Split(SplitEvent),
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct UpdateEvent {
    pub old_amount: i64,
    pub new_amount: i64,
    pub item_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct RemoveEvent {
    pub amount: i64,
    pub item_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct AddEvent {
    pub amount: i64,
    pub item_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct SwapEvent {
    pub amount: i64,
    pub item_id: i64,
    pub with_item_id: Option<i64>,
    pub with_amount: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct SplitEvent {
    pub old_amount: i64,
    pub new_amount: i64,
    pub item_id: i64,
}
