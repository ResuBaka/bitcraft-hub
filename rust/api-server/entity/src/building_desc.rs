//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use game_module::module_bindings::{BuildingDesc, BuildingFunction};
use sea_orm::FromJsonQueryResult;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "building_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_type = "Json")]
    pub functions: Vec<Function>,
    pub name: String,
    pub description: String,
    pub rested_buff_duration: i32,
    pub light_radius: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub unenterable: bool,
    pub wilderness: bool,
    #[sea_orm(column_type = "Json")]
    pub footprint: Vec<FootprintTile>,
    pub max_health: i32,
    pub ignore_damage: bool,
    pub defense_level: i32,
    pub decay: f32,
    pub maintenance: f32,
    #[sea_orm(column_type = "Json")]
    pub build_permission: BuildingInteractionLevel,
    #[sea_orm(column_type = "Json")]
    pub interact_permission: BuildingInteractionLevel,
    pub has_action: bool,
    pub show_in_compendium: bool,
    pub is_ruins: bool,
    pub not_deconstructible: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Function {
    pub function_type: i32,
    pub level: i32,
    pub crafting_slots: i32,
    pub storage_slots: i32,
    pub cargo_slots: i32,
    pub refining_slots: i32,
    pub refining_cargo_slots: i32,
    pub item_slot_size: i32,
    pub cargo_slot_size: i32,
    pub trade_orders: i32,
    pub allowed_item_id_per_slot: Vec<i32>,
    pub buff_ids: Vec<i32>,
    pub concurrent_crafts_per_player: i32,
    pub terraform: bool,
    pub housing_slots: i32,
    pub housing_income: u32,
}

impl From<BuildingFunction> for Function {
    fn from(value: BuildingFunction) -> Self {
        Self {
            function_type: value.function_type,
            level: value.level,
            crafting_slots: value.crafting_slots,
            storage_slots: value.storage_slots,
            cargo_slots: value.cargo_slots,
            refining_slots: value.refining_slots,
            refining_cargo_slots: value.refining_cargo_slots,
            item_slot_size: value.item_slot_size,
            cargo_slot_size: value.cargo_slot_size,
            trade_orders: value.trade_orders,
            allowed_item_id_per_slot: value.allowed_item_id_per_slot,
            buff_ids: value.buff_ids,
            concurrent_crafts_per_player: value.concurrent_crafts_per_player,
            terraform: value.terraform,
            housing_slots: value.housing_slots,
            housing_income: value.housing_income,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct FootprintTile {
    pub x: i32,
    pub z: i32,
    pub footprint_type: FootprintType,
}

impl From<game_module::module_bindings::FootprintTile> for FootprintTile {
    fn from(value: game_module::module_bindings::FootprintTile) -> Self {
        Self {
            x: value.x,
            z: value.z,
            footprint_type: value.footprint_type.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Eq, Hash, Deserialize, Serialize)]
pub enum FootprintType {
    Hitbox,
    Walkable,
    Perimeter,
    WalkableResource,
}

impl From<game_module::module_bindings::FootprintType> for FootprintType {
    fn from(value: game_module::module_bindings::FootprintType) -> Self {
        match value {
            game_module::module_bindings::FootprintType::Hitbox => FootprintType::Hitbox,
            game_module::module_bindings::FootprintType::Walkable => FootprintType::Walkable,
            game_module::module_bindings::FootprintType::Perimeter => FootprintType::Perimeter,
            game_module::module_bindings::FootprintType::WalkableResource => {
                FootprintType::WalkableResource
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponse {
    pub id: i64,
    pub functions: Vec<Function>,
    pub name: String,
    pub description: String,
    pub rested_buff_duration: i32,
    pub light_radius: i32,
    pub model_asset_name: String,
    pub icon_asset_name: String,
    pub unenterable: bool,
    pub wilderness: bool,
    pub footprint: Vec<FootprintTile>,
    pub max_health: i32,
    pub decay: f32,
    pub maintenance: f32,
    pub has_action: bool,
    pub show_in_compendium: bool,
    pub is_ruins: bool,
    pub count: i32,
}

impl From<BuildingDesc> for Model {
    fn from(value: BuildingDesc) -> Self {
        Self {
            id: value.id as i64,
            functions: value.functions.into_iter().map(Into::into).collect(),
            name: value.name,
            description: value.description,
            rested_buff_duration: value.rested_buff_duration,
            light_radius: value.light_radius,
            model_asset_name: value.model_asset_name,
            icon_asset_name: value.icon_asset_name,
            unenterable: value.unenterable,
            wilderness: value.wilderness,
            footprint: value.footprint.into_iter().map(Into::into).collect(),
            max_health: value.max_health,
            ignore_damage: value.ignore_damage,
            defense_level: value.defense_level,
            decay: value.decay,
            maintenance: value.maintenance,
            build_permission: value.build_permission.into(),
            interact_permission: value.interact_permission.into(),
            has_action: value.has_action,
            show_in_compendium: value.show_in_compendium,
            is_ruins: value.is_ruins,
            not_deconstructible: value.not_deconstructible,
        }
    }
}

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Eq, Hash, Deserialize, Serialize)]
pub enum BuildingInteractionLevel {
    None,
    Claim,
    Empire,
    All,
}

impl From<game_module::module_bindings::BuildingInteractionLevel> for BuildingInteractionLevel {
    fn from(value: game_module::module_bindings::BuildingInteractionLevel) -> Self {
        match value {
            game_module::module_bindings::BuildingInteractionLevel::None => {
                BuildingInteractionLevel::None
            }
            game_module::module_bindings::BuildingInteractionLevel::Claim => {
                BuildingInteractionLevel::Claim
            }
            game_module::module_bindings::BuildingInteractionLevel::Empire => {
                BuildingInteractionLevel::Empire
            }
            game_module::module_bindings::BuildingInteractionLevel::All => {
                BuildingInteractionLevel::All
            }
        }
    }
}
