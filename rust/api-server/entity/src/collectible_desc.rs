use sea_orm::entity::prelude::*;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use std::fmt;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "collectible_desc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub description: String,
    pub collectible_type: Json,
    pub invalidates_type: Json,
    pub auto_collect: bool,
    pub collectible_rarity: Json,
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
}


// #[derive(Eq,Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
// #[serde(transparent)]
// pub struct EnumType<T>(pub (T,Json))

// impl<T> sea_orm::TryGetableFromJson for EnumType<T> where for<'de> T: Deserialize<'de> {}

// impl<T> std::convert::From<EnumType<T>> for sea_orm::Value
// where
// EnumType<T>: Serialize,
// {
//     fn from(source: EnumType<T>) -> Self {
//         sea_orm::Value::Json(
//             serde_json::to_value(&source)
//                 .ok()
//                 .map(|s| std::boxed::Box::new(s)),
//         )
//     }
// }

// impl<T> sea_orm::sea_query::ValueType for EnumType<T>
// where
// EnumType<T>: DeserializeOwned,
// {
//     fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
//         match v {
//             sea_orm::Value::Json(Some(json)) => {
//                 Ok(serde_json::from_value(*json).map_err(|_| sea_orm::sea_query::ValueTypeErr)?)
//             }
//             _ => Err(sea_orm::sea_query::ValueTypeErr),
//         }
//     }

//     fn type_name() -> String {
//         stringify!(#ident).to_owned()
//     }

//     fn array_type() -> sea_orm::sea_query::ArrayType {
//         sea_orm::sea_query::ArrayType::Json
//     }

//     fn column_type() -> sea_orm::sea_query::ColumnType {
//         sea_orm::sea_query::ColumnType::Json
//     }
// }

// impl<T> sea_orm::sea_query::Nullable for EnumType<T> {
//     fn null() -> sea_orm::Value {
//         sea_orm::Value::Json(None)
//     }
// }

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum InvalidatesType {
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
}


#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
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
}


#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum CollectibleRarity {
    Default = 0,
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
    Mythic = 6,
}
