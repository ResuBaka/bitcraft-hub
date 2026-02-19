use crate::crafting_recipe::ToolRequirement;
use crate::shared::probabilistic_item_stack::ProbabilisticItemStack;
use game_module::module_bindings::ExtractionRecipeDesc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[ts(rename = "ExtractionRecipeDesc")]
#[sea_orm(table_name = "extraction_recipe_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub resource_id: i32,
    #[sea_orm(column_type = "Json")]
    pub extracted_item_stacks: Vec<ProbabilisticItemStack>,
    #[sea_orm(column_type = "Json")]
    pub tool_requirements: Vec<ToolRequirement>,
    pub allow_use_hands: bool,
    pub time_requirement: f32,
    pub stamina_requirement: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<ExtractionRecipeDesc> for Model {
    fn from(value: ExtractionRecipeDesc) -> Self {
        Self {
            id: value.id,
            resource_id: value.resource_id,
            extracted_item_stacks: value
                .extracted_item_stacks
                .into_iter()
                .map(Into::into)
                .collect(),
            tool_requirements: value
                .tool_requirements
                .into_iter()
                .map(Into::into)
                .collect(),
            allow_use_hands: value.allow_use_hands,
            time_requirement: value.time_requirement,
            stamina_requirement: value.stamina_requirement,
        }
    }
}
