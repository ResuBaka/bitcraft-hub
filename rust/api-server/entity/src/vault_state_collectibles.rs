use crate::collectible_desc;
use game_module::module_bindings::{VaultCollectible, VaultState, vault_state_type};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "vault_state_collectibles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub id: i32,
    pub activated: bool,
    pub count: i32,
    pub region: String,
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
    pub fn to_model(&self, entity_id: i64, region: String) -> Model {
        Model {
            entity_id,
            id: self.id,
            activated: self.activated,
            count: self.count,
            region,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
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
    pub fn to_model_collectibles(
        &self,
        region: String,
    ) -> Vec<crate::vault_state_collectibles::Model> {
        self.collectibles
            .iter()
            .map(|collectible| collectible.to_model(self.entity_id, region.clone()))
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
        crate::vault_state_collectibles::RawVaultState {
            entity_id: value.entity_id as i64,
            collectibles: value.collectibles.into_iter().map(Into::into).collect(),
        }
    }
}
