use crate::module_bindings::SkillCategory;

pub mod module_bindings;

impl From<SkillCategory> for i32 {
    fn from(value: SkillCategory) -> Self {
        match &value {
            SkillCategory::None => 0,
            SkillCategory::Adventure => 2,
            SkillCategory::Profession => 1,
        }
    }
}
