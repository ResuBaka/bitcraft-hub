use game_module::module_bindings::ClaimState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "claim_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub owner_player_entity_id: i64,
    pub owner_building_entity_id: i64,
    pub name: String,
    pub neutral: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<ClaimState> for crate::claim_state::Model {
    fn from(value: ClaimState) -> Self {
        crate::claim_state::Model {
            entity_id: value.entity_id as i64,
            owner_player_entity_id: value.owner_player_entity_id as i64,
            owner_building_entity_id: value.owner_building_entity_id as i64,
            name: value.name,
            neutral: value.neutral,
        }
    }
}
