//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

// use sea_orm::entity::prelude::*;
use game_module::module_bindings::LocationState;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "Location")]
// #[sea_orm(table_name = "location")]
pub struct Model {
    // #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub chunk_index: i64,
    pub x: i32,
    pub z: i32,
    pub dimension: i32,
}

// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
// pub enum Relation {}

// impl ActiveModelBehavior for ActiveModel {}

impl From<LocationState> for Model {
    fn from(state: LocationState) -> Self {
        Self {
            entity_id: state.entity_id as i64,
            chunk_index: state.chunk_index as i64,
            x: state.x,
            z: state.z,
            dimension: state.dimension as i32,
        }
    }
}
