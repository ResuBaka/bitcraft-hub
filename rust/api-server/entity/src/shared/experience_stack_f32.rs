use game_module::module_bindings;
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromJsonQueryResult, TS)]
pub struct ExperienceStackF32 {
    pub skill_id: i32,
    pub quantity: f32,
}

impl From<module_bindings::ExperienceStackF32> for ExperienceStackF32 {
    fn from(value: module_bindings::ExperienceStackF32) -> Self {
        ExperienceStackF32 {
            quantity: value.quantity,
            skill_id: value.skill_id,
        }
    }
}
