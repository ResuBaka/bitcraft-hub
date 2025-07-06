use sea_orm::entity::prelude::*;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
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
    pub invalidates_type: InvalidatesType,
    pub auto_collect: bool,
    pub collectible_rarity: CollectibleRarity,
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, TS)]
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
    Pet = 17,
    ClothesCape = 18,
}

impl<'de> Deserialize<'de> for InvalidatesType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct InvalidatesTypeVisitor;

        impl<'de> Visitor<'de> for InvalidatesTypeVisitor {
            type Value = InvalidatesType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a map with a single key representing the enum variant or an array")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                if let Some((key, _)) = map.next_entry::<i32, serde_json::Value>()? {
                    match key {
                        0 => Ok(InvalidatesType::Default),
                        1 => Ok(InvalidatesType::Hair),
                        2 => Ok(InvalidatesType::Mask),
                        3 => Ok(InvalidatesType::MaskPattern),
                        4 => Ok(InvalidatesType::HairColor),
                        5 => Ok(InvalidatesType::Nameplate),
                        6 => Ok(InvalidatesType::BodyColor),
                        7 => Ok(InvalidatesType::Emblem),
                        8 => Ok(InvalidatesType::ClothesHead),
                        9 => Ok(InvalidatesType::ClothesBelt),
                        10 => Ok(InvalidatesType::ClothesTorso),
                        11 => Ok(InvalidatesType::ClothesArms),
                        12 => Ok(InvalidatesType::ClothesLegs),
                        13 => Ok(InvalidatesType::ClothesFeet),
                        14 => Ok(InvalidatesType::Deployable),
                        15 => Ok(InvalidatesType::Title),
                        16 => Ok(InvalidatesType::Crown),
                        17 => Ok(InvalidatesType::Pet),
                        18 => Ok(InvalidatesType::ClothesCape),
                        _ => Err(de::Error::custom("invalid enum variant invalidates_type")),
                    }
                } else {
                    Err(de::Error::custom("expected a map with a single key"))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if let Some(key) = seq.next_element::<i32>()? {
                    let _ = seq.next_element::<serde_json::Value>()?;
                    match key {
                        0 => Ok(InvalidatesType::Default),
                        1 => Ok(InvalidatesType::Hair),
                        2 => Ok(InvalidatesType::Mask),
                        3 => Ok(InvalidatesType::MaskPattern),
                        4 => Ok(InvalidatesType::HairColor),
                        5 => Ok(InvalidatesType::Nameplate),
                        6 => Ok(InvalidatesType::BodyColor),
                        7 => Ok(InvalidatesType::Emblem),
                        8 => Ok(InvalidatesType::ClothesHead),
                        9 => Ok(InvalidatesType::ClothesBelt),
                        10 => Ok(InvalidatesType::ClothesTorso),
                        11 => Ok(InvalidatesType::ClothesArms),
                        12 => Ok(InvalidatesType::ClothesLegs),
                        13 => Ok(InvalidatesType::ClothesFeet),
                        14 => Ok(InvalidatesType::Deployable),
                        15 => Ok(InvalidatesType::Title),
                        16 => Ok(InvalidatesType::Crown),
                        17 => Ok(InvalidatesType::Pet),
                        18 => Ok(InvalidatesType::ClothesCape),
                        _ => Err(de::Error::custom("invalid enum variant invalidates_type")),
                    }
                } else {
                    Err(de::Error::custom(
                        "expected a sequence with a single element",
                    ))
                }
            }
        }

        deserializer.deserialize_any(InvalidatesTypeVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, TS)]
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

impl<'de> Deserialize<'de> for CollectibleType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CollectibleTypeVisitor;

        impl<'de> Visitor<'de> for CollectibleTypeVisitor {
            type Value = CollectibleType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a map with a single key representing the enum variant or an array")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                if let Some((key, _)) = map.next_entry::<i32, serde_json::Value>()? {
                    match key {
                        0 => Ok(CollectibleType::Default),
                        1 => Ok(CollectibleType::Hair),
                        2 => Ok(CollectibleType::Mask),
                        3 => Ok(CollectibleType::MaskPattern),
                        4 => Ok(CollectibleType::HairColor),
                        5 => Ok(CollectibleType::Nameplate),
                        6 => Ok(CollectibleType::BodyColor),
                        7 => Ok(CollectibleType::Emblem),
                        8 => Ok(CollectibleType::ClothesHead),
                        9 => Ok(CollectibleType::ClothesBelt),
                        10 => Ok(CollectibleType::ClothesTorso),
                        11 => Ok(CollectibleType::ClothesArms),
                        12 => Ok(CollectibleType::ClothesLegs),
                        13 => Ok(CollectibleType::ClothesFeet),
                        14 => Ok(CollectibleType::Deployable),
                        15 => Ok(CollectibleType::Title),
                        16 => Ok(CollectibleType::Crown),
                        17 => Ok(CollectibleType::Pet),
                        18 => Ok(CollectibleType::ClothesCape),
                        _ => Err(de::Error::custom("invalid enum variant collectible_type")),
                    }
                } else {
                    Err(de::Error::custom("expected a map with a single key"))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if let Some(key) = seq.next_element::<i32>()? {
                    let _ = seq.next_element::<serde_json::Value>()?;

                    match key {
                        0 => Ok(CollectibleType::Default),
                        1 => Ok(CollectibleType::Hair),
                        2 => Ok(CollectibleType::Mask),
                        3 => Ok(CollectibleType::MaskPattern),
                        4 => Ok(CollectibleType::HairColor),
                        5 => Ok(CollectibleType::Nameplate),
                        6 => Ok(CollectibleType::BodyColor),
                        7 => Ok(CollectibleType::Emblem),
                        8 => Ok(CollectibleType::ClothesHead),
                        9 => Ok(CollectibleType::ClothesBelt),
                        10 => Ok(CollectibleType::ClothesTorso),
                        11 => Ok(CollectibleType::ClothesArms),
                        12 => Ok(CollectibleType::ClothesLegs),
                        13 => Ok(CollectibleType::ClothesFeet),
                        14 => Ok(CollectibleType::Deployable),
                        15 => Ok(CollectibleType::Title),
                        16 => Ok(CollectibleType::Crown),
                        17 => Ok(CollectibleType::Pet),
                        18 => Ok(CollectibleType::ClothesCape),
                        _ => Err(de::Error::custom("invalid enum variant collectible_type")),
                    }
                } else {
                    Err(de::Error::custom(
                        "expected a sequence with a single element",
                    ))
                }
            }
        }

        deserializer.deserialize_any(CollectibleTypeVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, TS)]
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

impl<'de> Deserialize<'de> for CollectibleRarity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CollectibleRarityVisitor;

        impl<'de> Visitor<'de> for CollectibleRarityVisitor {
            type Value = CollectibleRarity;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a map with a single key representing the enum variant or an array")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                if let Some((key, _)) = map.next_entry::<i32, serde_json::Value>()? {
                    match key {
                        0 => Ok(CollectibleRarity::Default),
                        1 => Ok(CollectibleRarity::Common),
                        2 => Ok(CollectibleRarity::Uncommon),
                        3 => Ok(CollectibleRarity::Rare),
                        4 => Ok(CollectibleRarity::Epic),
                        5 => Ok(CollectibleRarity::Legendary),
                        6 => Ok(CollectibleRarity::Mythic),
                        _ => Err(de::Error::custom("invalid enum variant collectible_rarity")),
                    }
                } else {
                    Err(de::Error::custom("expected a map with a single key"))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if let Some(key) = seq.next_element::<i32>()? {
                    let _ = seq.next_element::<serde_json::Value>()?;
                    match key {
                        0 => Ok(CollectibleRarity::Default),
                        1 => Ok(CollectibleRarity::Common),
                        2 => Ok(CollectibleRarity::Uncommon),
                        3 => Ok(CollectibleRarity::Rare),
                        4 => Ok(CollectibleRarity::Epic),
                        5 => Ok(CollectibleRarity::Legendary),
                        6 => Ok(CollectibleRarity::Mythic),
                        _ => Err(de::Error::custom("invalid enum variant collectible_rarity")),
                    }
                } else {
                    Err(de::Error::custom(
                        "expected a sequence with a single element",
                    ))
                }
            }
        }

        deserializer.deserialize_any(CollectibleRarityVisitor)
    }
}
