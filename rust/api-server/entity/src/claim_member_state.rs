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
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    entity_id: i64,
    claim_entity_id: i64,
    player_entity_id: i64,
    user_name: String,
    inventory_permission: bool,
    build_permission: bool,
    officer_permission: bool,
    co_owner_permission: bool,
    region: String,
}

impl ModelBuilder {
    pub fn new(claim_state: ClaimMemberState) -> Self {
        ModelBuilder {
            entity_id: claim_state.entity_id as i64,
            claim_entity_id: claim_state.claim_entity_id as i64,
            player_entity_id: claim_state.player_entity_id as i64,
            user_name: claim_state.user_name,
            inventory_permission: claim_state.inventory_permission,
            build_permission: claim_state.build_permission,
            officer_permission: claim_state.officer_permission,
            co_owner_permission: claim_state.co_owner_permission,
            region: String::new(), // Default or uninitialized
        }
    }

    pub fn with_region(mut self, region: String) -> Self {
        self.region = region;
        self
    }

    pub fn build(self) -> crate::claim_member_state::Model {
        crate::claim_member_state::Model {
            entity_id: self.entity_id,
            claim_entity_id: self.claim_entity_id,
            player_entity_id: self.player_entity_id,
            user_name: self.user_name,
            inventory_permission: self.inventory_permission,
            build_permission: self.build_permission,
            officer_permission: self.officer_permission,
            co_owner_permission: self.co_owner_permission,
            region: self.region,
        }
    }
}
