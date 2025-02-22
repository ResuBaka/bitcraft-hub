use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_changelog")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub user_id: Option<i64>,
    pub item_id: i64,
    pub amount: i64,
    pub type_of_change: TypeOfChange,
    #[sea_orm(column_type = "JsonBinary")]
    pub event_data: InventoryChangelogEventData,
    pub timestamp: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum TypeOfChange {
    Add = 0,
    Remove = 1,
    Update = 2,
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
