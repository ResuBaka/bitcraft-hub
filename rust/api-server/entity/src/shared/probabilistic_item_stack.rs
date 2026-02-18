use crate::shared::item_stack::ItemStack;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, TS)]
pub struct ProbabilisticItemStack {
    pub item_stack: Option<ItemStack>,
    pub probability: f32,
}

impl From<game_module::module_bindings::ProbabilisticItemStack> for ProbabilisticItemStack {
    fn from(value: game_module::module_bindings::ProbabilisticItemStack) -> Self {
        Self {
            item_stack: value.item_stack.map(Into::into),
            probability: value.probability,
        }
    }
}
