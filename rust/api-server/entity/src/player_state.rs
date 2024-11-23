//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "player_state")]
pub struct Model {
    pub teleport_location: Json,
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub time_played: i32,
    pub session_start_timestamp: i32,
    pub time_signed_in: i32,
    pub sign_in_timestamp: i32,
    pub last_shared_claim: i32,
    pub signed_in: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::player_username_state::Entity",
        from = "super::player_state::Column::EntityId",
        to = "super::player_username_state::Column::EntityId"
    )]
    PlayerUsernameState,
}

impl Related<super::player_username_state::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlayerUsernameState.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerStateMerged {
    pub teleport_location: Json,
    pub entity_id: i64,
    pub time_played: i32,
    pub session_start_timestamp: i32,
    pub time_signed_in: i32,
    pub sign_in_timestamp: i32,
    pub last_shared_claim: i32,
    pub signed_in: bool,
    pub username: String,
}