use super::shared::location;
use crate::shared::location::Location;
use game_module::module_bindings::ClaimLocalState;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "claim_local_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub entity_id: i64,
    pub supplies: i32,
    pub building_maintenance: f32,
    pub num_tiles: i32,
    pub num_tile_neighbors: i32,
    #[sea_orm(column_type = "Json")]
    pub location: Option<location::Location>,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub supplies_purchase_threshold: i32,
    pub supplies_purchase_price: f32,
    pub building_description_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelRaw {
    pub entity_id: i64,
    pub supplies: i32,
    pub building_maintenance: f32,
    pub num_tiles: i32,
    pub num_tile_neighbors: i32,
    pub location: serde_json::Value,
    pub treasury: i32,
    pub xp_gained_since_last_coin_minting: i32,
    pub supplies_purchase_threshold: i32,
    pub supplies_purchase_price: f32,
    pub building_description_id: i32,
}

impl From<ModelRaw> for Model {
    fn from(model: ModelRaw) -> Self {
        let location = if model.location[0].as_i64().unwrap() == 1 {
            None
        } else {
            Some(serde_json::from_value::<Location>(model.location[1].clone()).unwrap())
        };

        Model {
            entity_id: model.entity_id,
            supplies: model.supplies,
            building_maintenance: model.building_maintenance,
            num_tiles: model.num_tiles,
            num_tile_neighbors: model.num_tile_neighbors,
            location,
            treasury: model.treasury,
            xp_gained_since_last_coin_minting: model.xp_gained_since_last_coin_minting,
            supplies_purchase_threshold: model.supplies_purchase_threshold,
            supplies_purchase_price: model.supplies_purchase_price,
            building_description_id: model.building_description_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_model_parse() {
        let raw_event_data = r#"{
  "entity_id": 72057594046084220,
  "supplies": 0,
  "building_maintenance": 0.0,
  "num_tiles": 12481,
  "num_tile_neighbors": 74112,
  "location": [
    0,
    {
      "x": 3250,
      "z": 4383,
      "dimension": 1
    }
  ],
  "treasury": 0,
  "xp_gained_since_last_coin_minting": 0,
  "supplies_purchase_threshold": 0,
  "supplies_purchase_price": 1.0,
  "building_description_id": 292245080
}"#;
        let parsed_event_data: Model = serde_json::from_str(raw_event_data).unwrap();
    }
}

impl From<ClaimLocalState> for crate::claim_local_state::Model {
    fn from(value: ClaimLocalState) -> Self {
        let mut location: Option<crate::shared::location::Location> = None;
        if let Some(loc) = value.location {
            location = Some(loc.into())
        }
        crate::claim_local_state::Model {
            entity_id: value.entity_id as i64,
            supplies: value.supplies,
            building_maintenance: value.building_maintenance,
            num_tiles: value.num_tiles,
            num_tile_neighbors: value.num_tile_neighbors as i32,
            treasury: value.treasury as i32,
            location: location,
            xp_gained_since_last_coin_minting: value.xp_gained_since_last_coin_minting as i32,
            supplies_purchase_threshold: value.supplies_purchase_threshold as i32,
            supplies_purchase_price: value.supplies_purchase_price,
            building_description_id: value.building_description_id,
        }
    }
}
