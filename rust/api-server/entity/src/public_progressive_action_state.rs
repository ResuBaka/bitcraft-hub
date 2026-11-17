use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "public_progressive_action_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub building_entity_id: i64,
    pub owner_entity_id: i64,
    pub region: crate::shared::Region,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn from_wire(
        value: game_module::module_bindings::PublicProgressiveActionState,
        region: crate::shared::Region,
    ) -> Self {
        Self {
            entity_id: value.entity_id as i64,
            building_entity_id: value.building_entity_id as i64,
            owner_entity_id: value.owner_entity_id as i64,
            region,
        }
    }
}
