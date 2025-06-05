use crate::inventory::ItemType;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, TS)]
pub struct ItemStack {
    pub item_id: i32,
    pub quantity: i32,
    pub item_type: ItemType,
    pub durability: Option<i32>,
}

impl From<game_module::module_bindings::ItemStack> for ItemStack {
    fn from(value: game_module::module_bindings::ItemStack) -> Self {
        Self {
            item_id: value.item_id,
            quantity: value.quantity,
            item_type: value.item_type.into(),
            durability: value.durability,
        }
    }
}
