//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct ConsumedItemStack {
    pub item_id: i64,
    pub quantity: i64,
    pub item_type: serde_json::Value,
    pub discovery_score: i64,
    pub consumption_chance: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsumedItemStackWithInner {
    pub item_id: i64,
    pub quantity: i64,
    pub item_type: serde_json::Value,
    pub discovery_score: i64,
    pub consumption_chance: f32,
    pub inner: Option<Vec<Vec<Self>>>,
}

impl From<ConsumedItemStack> for ConsumedItemStackWithInner {
    fn from(value: ConsumedItemStack) -> Self {
        Self {
            item_id: value.item_id,
            quantity: value.quantity,
            item_type: value.item_type,
            discovery_score: value.discovery_score,
            consumption_chance: value.consumption_chance,
            inner: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct CraftedItemStack {
    pub item_id: i64,
    pub quantity: i64,
    pub item_type: serde_json::Value,
    pub durability: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CraftedItemStackWithInner {
    pub item_id: i64,
    pub quantity: i64,
    pub item_type: serde_json::Value,
    pub durability: serde_json::Value,
    pub inner: Option<Vec<Vec<Self>>>,
}

impl From<CraftedItemStack> for CraftedItemStackWithInner {
    fn from(value: CraftedItemStack) -> Self {
        Self {
            item_id: value.item_id,
            quantity: value.quantity,
            item_type: value.item_type,
            durability: value.durability,
            inner: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CraftingRecipeWithInner {
    pub id: i64,
    pub name: String,
    //@TODO change to f32
    pub time_requirement: f32,
    //@TODO change to f32
    pub stamina_requirement: f32,
    pub building_requirement: Json,
    pub level_requirements: Json,
    pub tool_requirements: Json,
    pub consumed_item_stacks: Vec<ConsumedItemStackWithInner>,
    pub discovery_triggers: Json,
    pub required_knowledges: Json,
    pub required_claim_tech_id: i32,
    pub full_discovery_score: i32,
    pub experience_per_progress: Json,
    pub allow_use_hands: bool,
    pub crafted_item_stacks: Vec<CraftedItemStackWithInner>,
    pub is_passive: bool,
    pub actions_required: i32,
    pub tool_mesh_index: i32,
    pub recipe_performance_id: i32,
}

impl From<Model> for CraftingRecipeWithInner {
    fn from(value: Model) -> Self {
        Self {
            id: value.id,
            name: value.name,
            time_requirement: value.time_requirement,
            stamina_requirement: value.stamina_requirement,
            building_requirement: value.building_requirement,
            level_requirements: value.level_requirements,
            tool_requirements: value.tool_requirements,
            consumed_item_stacks: value
                .consumed_item_stacks
                .iter()
                .map(|x| x.clone().into())
                .collect(),
            discovery_triggers: value.discovery_triggers,
            required_knowledges: value.required_knowledges,
            required_claim_tech_id: value.required_claim_tech_id,
            full_discovery_score: value.full_discovery_score,
            experience_per_progress: value.experience_per_progress,
            allow_use_hands: value.allow_use_hands,
            crafted_item_stacks: value
                .crafted_item_stacks
                .iter()
                .map(|x| x.clone().into())
                .collect(),
            is_passive: value.is_passive,
            actions_required: value.actions_required,
            tool_mesh_index: value.tool_mesh_index,
            recipe_performance_id: value.recipe_performance_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "crafting_recipe")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub name: String,
    pub time_requirement: f32,
    pub stamina_requirement: f32,
    pub tool_durability_lost: i32,
    pub building_requirement: Json,
    pub level_requirements: Json,
    pub tool_requirements: Json,
    #[sea_orm(column_type = "Json")]
    pub consumed_item_stacks: Vec<ConsumedItemStack>,
    pub discovery_triggers: Json,
    pub required_knowledges: Json,
    pub required_claim_tech_id: i32,
    pub full_discovery_score: i32,
    pub experience_per_progress: Json,
    pub allow_use_hands: bool,
    #[sea_orm(column_type = "Json")]
    pub crafted_item_stacks: Vec<CraftedItemStack>,
    pub is_passive: bool,
    pub actions_required: i32,
    pub tool_mesh_index: i32,
    pub recipe_performance_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
