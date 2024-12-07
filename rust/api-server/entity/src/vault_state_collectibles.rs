use crate::collectible_desc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "vault_state_collectibles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub id: i32,
    pub activated: bool,
    pub count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for crate::vault_state_collectibles::ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawVaultStateCollectibles {
    pub id: i32,
    pub activated: bool,
    pub count: i32,
}

impl RawVaultStateCollectibles {
    pub fn to_model(&self, entity_id: i64) -> Model {
        Model {
            entity_id,
            id: self.id,
            activated: self.activated,
            count: self.count,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VaultStateCollectibleWithDesc {
    pub entity_id: i64,
    pub id: i32,
    pub activated: bool,
    pub count: i32,
    pub collectible_desc: collectible_desc::Model,
}
