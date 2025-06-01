use game_module::module_bindings::ClaimMemberState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "claim_member_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub claim_entity_id: i64,
    pub player_entity_id: i64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<ClaimMemberState> for crate::claim_member_state::Model {
    fn from(value: ClaimMemberState) -> Self {
        crate::claim_member_state::Model {
            entity_id: value.entity_id as i64,
            claim_entity_id: value.claim_entity_id as i64,
            player_entity_id: value.player_entity_id as i64,
            user_name: value.user_name,
            inventory_permission: value.inventory_permission,
            build_permission: value.build_permission,
            officer_permission: value.officer_permission,
            co_owner_permission: value.co_owner_permission,
        }
    }
}
