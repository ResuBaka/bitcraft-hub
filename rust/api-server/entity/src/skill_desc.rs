//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "skill_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub skill: i32,
    pub name: String,
    pub description: String,
    pub icon_asset_name: String,
    pub title: String,
    pub skill_category: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillDescRaw {
    pub id: i64,
    pub skill: i32,
    pub name: String,
    pub description: String,
    pub icon_asset_name: String,
    pub title: String,
    pub skill_category: serde_json::Value,
}

impl SkillDescRaw {
    pub fn to_model(&self) -> anyhow::Result<Model> {
        let skill_category = self.skill_category.as_array().unwrap()[0].as_i64().unwrap() as i32;

        Ok(Model {
            id: self.id,
            skill: self.skill,
            name: self.name.clone(),
            description: self.description.clone(),
            icon_asset_name: self.icon_asset_name.clone(),
            title: self.title.clone(),
            skill_category: skill_category,
        })
    }
}
