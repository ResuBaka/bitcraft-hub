use game_module::module_bindings::{PassiveCraftState, PassiveCraftStatus};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "passive_craft_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub owner_entity_id: i64,
    pub recipe_id: i32,
    pub building_entity_id: i64,
    pub timestamp: DateTimeWithTimeZone,
    pub status: i16,
    pub slot: Option<i64>,
    pub region: crate::shared::Region,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn from_wire(row: PassiveCraftState, region: crate::shared::Region) -> Self {
        Self {
            entity_id: row.entity_id as i64,
            owner_entity_id: row.owner_entity_id as i64,
            recipe_id: row.recipe_id,
            building_entity_id: row.building_entity_id as i64,
            timestamp: chrono::DateTime::from_timestamp_micros(
                row.timestamp.to_micros_since_unix_epoch(),
            )
            .expect("invalid passive craft timestamp")
            .into(),
            status: match row.status {
                PassiveCraftStatus::Queued => 0,
                PassiveCraftStatus::Processing => 1,
                PassiveCraftStatus::Complete => 2,
            },
            slot: row.slot.map(i64::from),
            region,
        }
    }
}
