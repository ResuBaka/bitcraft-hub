use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod experience_stack_f32;
pub mod item_stack;
pub mod location;
pub mod probabilistic_item_stack;
pub mod timestamp;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Eq, TS, FromJsonQueryResult)]
pub enum JsonRarity {
    Default = 0,
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
    Mythic = 6,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Eq, TS, DeriveActiveEnum, EnumIter)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum Rarity {
    Default = 0,
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
    Mythic = 6,
}

impl From<game_module::module_bindings::Rarity> for Rarity {
    fn from(value: game_module::module_bindings::Rarity) -> Self {
        match value {
            game_module::module_bindings::Rarity::Default => Rarity::Default,
            game_module::module_bindings::Rarity::Common => Rarity::Common,
            game_module::module_bindings::Rarity::Uncommon => Rarity::Uncommon,
            game_module::module_bindings::Rarity::Rare => Rarity::Rare,
            game_module::module_bindings::Rarity::Epic => Rarity::Epic,
            game_module::module_bindings::Rarity::Legendary => Rarity::Legendary,
            game_module::module_bindings::Rarity::Mythic => Rarity::Mythic,
        }
    }
}
