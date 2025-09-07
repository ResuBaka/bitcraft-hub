use crate::shared;
use game_module::module_bindings::{CollectibleDesc, Rarity};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "CollectibleDesc")]
#[sea_orm(table_name = "collectible_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub description: String,
    pub collectible_type: CollectibleType,
    pub invalidates_type: CollectibleType,
    pub auto_collect: bool,
    pub collectible_rarity: shared::Rarity,
    pub starting_loadout: bool,
    pub locked: bool,
    pub variant: i32,
    pub color: String,
    pub emission: String,
    pub max_equip_count: i32,
    pub model_asset_name: String,
    pub variant_material: String,
    pub icon_asset_name: String,
    pub tag: String,
    pub display_string: String,
    pub item_deed_id: i32,
    pub required_knowledges_to_use: Vec<i32>,
    pub required_knowledges_to_convert: Vec<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl From<game_module::module_bindings::CollectibleDesc> for Model {
    fn from(value: game_module::module_bindings::CollectibleDesc) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            collectible_type: value.collectible_type.into(),
            invalidates_type: value.invalidates_type.into(),
            auto_collect: value.auto_collect,
            collectible_rarity: value.collectible_rarity.into(),
            starting_loadout: value.starting_loadout,
            locked: value.locked,
            variant: value.variant,
            color: value.color,
            emission: value.emission,
            max_equip_count: value.max_equip_count,
            model_asset_name: value.model_asset_name,
            variant_material: value.variant_material,
            icon_asset_name: value.icon_asset_name,
            tag: value.tag,
            display_string: value.display_string,
            item_deed_id: value.item_deed_id,
            required_knowledges_to_use: value.required_knowledges_to_use,
            required_knowledges_to_convert: value.required_knowledges_to_convert,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum CollectibleType {
    Default = 0,
    Hair = 1,
    Mask = 2,
    MaskPattern = 3,
    HairColor = 4,
    Nameplate = 5,
    BodyColor = 6,
    Emblem = 7,
    ClothesHead = 8,
    ClothesBelt = 9,
    ClothesTorso = 10,
    ClothesArms = 11,
    ClothesLegs = 12,
    ClothesFeet = 13,
    Deployable = 14,
    Title = 15,
    Crown = 16,
    Pet = 17,
    ClothesCape = 18,
}

impl From<game_module::module_bindings::CollectibleType> for CollectibleType {
    fn from(value: game_module::module_bindings::CollectibleType) -> Self {
        match value {
            game_module::module_bindings::CollectibleType::Default => CollectibleType::Default,
            game_module::module_bindings::CollectibleType::Hair => CollectibleType::Hair,
            game_module::module_bindings::CollectibleType::Mask => CollectibleType::Mask,
            game_module::module_bindings::CollectibleType::MaskPattern => {
                CollectibleType::MaskPattern
            }
            game_module::module_bindings::CollectibleType::HairColor => CollectibleType::HairColor,
            game_module::module_bindings::CollectibleType::Nameplate => CollectibleType::Nameplate,
            game_module::module_bindings::CollectibleType::BodyColor => CollectibleType::BodyColor,
            game_module::module_bindings::CollectibleType::Emblem => CollectibleType::Emblem,
            game_module::module_bindings::CollectibleType::ClothesHead => {
                CollectibleType::ClothesHead
            }
            game_module::module_bindings::CollectibleType::ClothesBelt => {
                CollectibleType::ClothesBelt
            }
            game_module::module_bindings::CollectibleType::ClothesTorso => {
                CollectibleType::ClothesTorso
            }
            game_module::module_bindings::CollectibleType::ClothesArms => {
                CollectibleType::ClothesArms
            }
            game_module::module_bindings::CollectibleType::ClothesLegs => {
                CollectibleType::ClothesLegs
            }
            game_module::module_bindings::CollectibleType::ClothesFeet => {
                CollectibleType::ClothesFeet
            }
            game_module::module_bindings::CollectibleType::Deployable => {
                CollectibleType::Deployable
            }
            game_module::module_bindings::CollectibleType::Title => CollectibleType::Title,
            game_module::module_bindings::CollectibleType::Crown => CollectibleType::Crown,
            game_module::module_bindings::CollectibleType::Pet => CollectibleType::Pet,
            game_module::module_bindings::CollectibleType::ClothesCape => {
                CollectibleType::ClothesCape
            }
        }
    }
}

pub struct ModelBuilder {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub collectible_type: game_module::module_bindings::CollectibleType,
    pub invalidates_type: game_module::module_bindings::CollectibleType,
    pub auto_collect: bool,
    pub collectible_rarity: Rarity,
    pub starting_loadout: bool,
    pub locked: bool,
    pub variant: i32,
    pub color: String,
    pub emission: String,
    pub max_equip_count: i32,
    pub model_asset_name: String,
    pub variant_material: String,
    pub icon_asset_name: String,
    pub tag: String,
    pub display_string: String,
    pub item_deed_id: i32,
    pub required_knowledges_to_use: Vec<i32>,
    pub required_knowledges_to_convert: Vec<i32>,
}

impl ModelBuilder {
    pub fn new(value: CollectibleDesc) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            collectible_type: value.collectible_type,
            invalidates_type: value.invalidates_type,
            auto_collect: value.auto_collect,
            collectible_rarity: value.collectible_rarity,
            starting_loadout: value.starting_loadout,
            locked: value.locked,
            variant: value.variant,
            color: value.color,
            emission: value.emission,
            max_equip_count: value.max_equip_count,
            model_asset_name: value.model_asset_name,
            variant_material: value.variant_material,
            icon_asset_name: value.icon_asset_name,
            tag: value.tag,
            display_string: value.display_string,
            item_deed_id: value.item_deed_id,
            required_knowledges_to_use: value.required_knowledges_to_use,
            required_knowledges_to_convert: value.required_knowledges_to_convert,
        }
    }

    pub fn build(self) -> Model {
        Model {
            id: self.id,
            name: self.name,
            description: self.description,
            collectible_type: self.collectible_type.into(),
            invalidates_type: self.invalidates_type.into(),
            auto_collect: self.auto_collect,
            collectible_rarity: self.collectible_rarity.into(),
            starting_loadout: self.starting_loadout,
            locked: self.locked,
            variant: self.variant,
            color: self.color,
            emission: self.emission,
            max_equip_count: self.max_equip_count,
            model_asset_name: self.model_asset_name,
            variant_material: self.variant_material,
            icon_asset_name: self.icon_asset_name,
            tag: self.tag,
            display_string: self.display_string,
            item_deed_id: self.item_deed_id,
            required_knowledges_to_use: self.required_knowledges_to_use,
            required_knowledges_to_convert: self.required_knowledges_to_convert,
        }
    }
}
