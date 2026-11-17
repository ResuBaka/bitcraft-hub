use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "craft_activity")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub region: crate::shared::Region,
    pub timestamp: DateTimeWithTimeZone,
    /// 0 = craft hit, 1 = craft completion.
    pub event_type: i16,
    pub actor_entity_id: i64,
    pub progressive_action_entity_id: i64,
    pub building_entity_id: Option<i64>,
    pub claim_entity_id: Option<i64>,
    pub recipe_id: Option<i32>,
    pub craft_count: Option<i32>,
    pub progress: Option<i32>,
    pub is_crit: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
