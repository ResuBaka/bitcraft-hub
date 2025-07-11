//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use game_module::module_bindings::TravelerTaskState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, TS)]
#[sea_orm(table_name = "traveler_task_state")]
#[ts(rename = "TravelerTaskState")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    #[sea_orm(indexed)]
    pub player_entity_id: i64,
    #[sea_orm(indexed)]
    pub traveler_id: i32,
    #[sea_orm(indexed)]
    pub task_id: i32,
    #[sea_orm(indexed)]
    pub completed: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<TravelerTaskState> for Model {
    fn from(value: TravelerTaskState) -> Self {
        Model {
            entity_id: value.entity_id as i64,
            player_entity_id: value.player_entity_id as i64,
            traveler_id: value.traveler_id,
            task_id: value.task_id,
            completed: value.completed,
        }
    }
}
