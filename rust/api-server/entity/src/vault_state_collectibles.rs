use crate::collectible_desc;
use game_module::module_bindings::{VaultCollectible, VaultState, vault_state_type};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultStateCollectibleWithDesc {
    pub entity_id: i64,
    pub id: i32,
    pub activated: bool,
    pub count: i32,
    pub collectible_desc: collectible_desc::Model,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawVaultState {
    pub entity_id: i64,
    pub collectibles: Vec<RawVaultStateCollectibles>,
}

impl RawVaultState {
    pub fn to_model_collectibles(&self) -> Vec<crate::vault_state_collectibles::Model> {
        self.collectibles
            .iter()
            .map(|collectible| collectible.to_model(self.entity_id))
            .collect()
    }
}

impl From<VaultCollectible> for crate::vault_state_collectibles::RawVaultStateCollectibles {
    fn from(value: VaultCollectible) -> Self {
        crate::vault_state_collectibles::RawVaultStateCollectibles {
            id: value.id,
            activated: value.activated,
            count: value.count,
        }
    }
}
impl From<vault_state_type::VaultState> for crate::vault_state_collectibles::RawVaultState {
    fn from(value: VaultState) -> Self {
        let collectibles: Vec<crate::vault_state_collectibles::RawVaultStateCollectibles> = value
            .collectibles
            .iter()
            .map(|content| content.clone().into())
            .collect();
        crate::vault_state_collectibles::RawVaultState {
            entity_id: value.entity_id as i64,
            collectibles,
        }
    }
}
