// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::item_stack_type::ItemStack;
use super::level_requirement_type::LevelRequirement;
use super::npc_type_type::NpcType;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct TravelerTradeOrderDesc {
    pub id: i32,
    pub starting_stock: i32,
    pub always_offered: bool,
    pub traveler: NpcType,
    pub offer_items: Vec<ItemStack>,
    pub offer_cargo_id: Vec<i32>,
    pub required_items: Vec<ItemStack>,
    pub required_cargo_id: Vec<i32>,
    pub level_requirements: Vec<LevelRequirement>,
    pub achievement_requirements: Vec<i32>,
    pub hide_if_requirements_are_not_met: bool,
    pub required_knowledges: Vec<i32>,
    pub hide_without_required_knowledge: bool,
    pub blocking_knowledges: Vec<i32>,
    pub hide_with_blocking_knowledges: bool,
}

impl __sdk::InModule for TravelerTradeOrderDesc {
    type Module = super::RemoteModule;
}
