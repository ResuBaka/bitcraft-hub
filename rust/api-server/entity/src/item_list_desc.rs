//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use crate::shared::item_stack::ItemStack;
use game_module::module_bindings::item_list_desc_type;
use sea_orm::{FromJsonQueryResult, entity::prelude::*};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, TS)]
#[ts(rename = "ItemListDesc")]
#[sea_orm(table_name = "item_list_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    #[sea_orm(column_type = "Json")]
    pub possibilities: Vec<ItemListPossibility>,
}

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Deserialize, Serialize, TS)]
pub struct ItemListPossibility {
    pub probability: f32,
    pub items: Vec<ItemStack>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<game_module::module_bindings::ItemListPossibility> for ItemListPossibility {
    fn from(value: game_module::module_bindings::ItemListPossibility) -> Self {
        ItemListPossibility {
            probability: value.probability,
            items: value.items.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<item_list_desc_type::ItemListDesc> for Model {
    fn from(value: item_list_desc_type::ItemListDesc) -> Self {
        Model {
            id: value.id,
            name: value.name,
            possibilities: value.possibilities.into_iter().map(Into::into).collect(),
        }
    }
}
