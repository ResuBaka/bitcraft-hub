use crate::vault_state_collectibles::RawVaultStateCollectibles;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "vault_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub shards: i32,
    pub region: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for crate::vault_state::ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawVaultState {
    pub entity_id: i64,
    pub collectibles: Vec<RawVaultStateCollectibles>,
    pub shards: i32,
}
