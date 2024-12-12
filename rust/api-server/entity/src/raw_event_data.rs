use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "raw_event_data")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub timestamp: TimeDateTimeWithTimeZone,
    pub request_id: i64,
    pub reducer_id: i64,
    pub reducer_name: String,
    #[sea_orm(column_type = "VarBinary(StringLen::Max)")]
    pub event_data: Vec<u8>,
    pub user_id: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for super::raw_event_data::ActiveModel {}
