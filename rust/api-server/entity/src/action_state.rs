use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(rename = "ActionState")]
pub struct Model {
    pub entity_id: u64,
    pub owner_entity_id: u64,
    pub action_id: i32,
    #[ts(type = "any")]
    pub cooldown: serde_json::Value,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
