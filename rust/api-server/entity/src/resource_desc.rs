use crate::shared::item_stack::ItemStack;
use crate::shared::Rarity;
use game_module::module_bindings::ResourceDesc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[ts(rename = "ResourceDesc")]
#[sea_orm(table_name = "resource_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub description: String,
    pub tier: i32,
    pub tag: String,
    pub rarity: Rarity,
    #[sea_orm(column_type = "Json")]
    pub on_destroy_yield: Vec<ItemStack>,
    pub on_destroy_yield_resource_id: i32,
    pub icon_asset_name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<ResourceDesc> for Model {
    fn from(value: ResourceDesc) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            tier: value.tier,
            tag: value.tag,
            rarity: value.rarity.into(),
            on_destroy_yield: value.on_destroy_yield.into_iter().map(Into::into).collect(),
            on_destroy_yield_resource_id: value.on_destroy_yield_resource_id,
            icon_asset_name: value.icon_asset_name,
        }
    }
}
