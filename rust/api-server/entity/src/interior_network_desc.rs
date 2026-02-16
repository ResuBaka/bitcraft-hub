//! `SeaORM` Entity, `interior_network_desc`

use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "interior_network_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub building_id: i32,
    pub dimension_type: i32,
    #[sea_orm(column_type = "Json")]
    pub child_interior_instances: ChildInteriorInstances,
    pub region: String,
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct ChildInteriorInstances(pub Vec<i32>);

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ModelBuilder {
    building_id: i32,
    dimension_type: i32,
    child_interior_instances: ChildInteriorInstances,
    region: String,
}

impl ModelBuilder {
    pub fn new(value: game_module::module_bindings::InteriorNetworkDesc) -> Self {
        ModelBuilder {
            building_id: value.building_id,
            dimension_type: match value.dimension_type {
                game_module::module_bindings::DimensionType::Overworld => 0,
                game_module::module_bindings::DimensionType::BuildingInterior => 1,
                game_module::module_bindings::DimensionType::AncientRuin => 2,
                game_module::module_bindings::DimensionType::Dungeon => 3,
                game_module::module_bindings::DimensionType::Unknown => 4,
            },
            child_interior_instances: ChildInteriorInstances(value.child_interior_instances),
            region: String::new(),
        }
    }

    pub fn with_region(mut self, region: String) -> Self {
        self.region = region;
        self
    }

    pub fn build(self) -> Model {
        Model {
            building_id: self.building_id,
            dimension_type: self.dimension_type,
            child_interior_instances: self.child_interior_instances,
            region: self.region,
        }
    }
}
