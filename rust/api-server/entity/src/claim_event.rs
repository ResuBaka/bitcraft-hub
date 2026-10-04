use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[ts(export)]
pub enum ClaimEventType {
    TreasuryDeposit = 0,
    TreasuryWithdrawal = 1,
    ResearchCompleted = 2,
    BuildingPlaced = 3,
    BuildingRemoved = 4,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[sea_orm(table_name = "claim_event")]
#[ts(export, rename = "ClaimEvent")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub claim_entity_id: i64,
    pub region: crate::shared::Region,
    pub timestamp: DateTime<Utc>,
    pub event_type: ClaimEventType,
    pub actor_entity_id: Option<i64>,
    pub amount: Option<i64>,
    pub treasury_after: Option<i64>,
    pub research_id: Option<i32>,
    pub building_entity_id: Option<i64>,
    pub building_description_id: Option<i32>,
    pub subject_name: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
