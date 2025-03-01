//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub auto_id: u64,
    pub entity_id: u64,
    pub action_type: ActionType,
    pub layer: serde_json::Value,
    pub last_action_result: serde_json::Value,
    pub start_time: u64,
    pub duration: u64,
    pub target: serde_json::Value,
    #[serde(deserialize_with = "deserialize_with_recipe_id")]
    pub recipe_id: Option<i32>,
    pub client_cancel: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

fn deserialize_with_recipe_id<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let json = serde_json::Value::deserialize(deserializer)?;

    if let serde_json::Value::Array(array) = json {
        if array.len() == 2 {
            if let serde_json::Value::Number(number) = array[0].clone() {
                if let Some(number) = number.as_i64() {
                    if number == 1 {
                        return Ok(None);
                    }
                }
            }

            if let serde_json::Value::Number(number) = array[1].clone() {
                if let Some(number) = number.as_i64() {
                    return Ok(Some(number as i32));
                }
            }
        }
    }

    Err(serde::de::Error::custom("Invalid value"))
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum ActionType {
    None,
    Attack,
    DestroyPaving,
    StationaryEmote,
    Extract,
    PaveTile,
    SpawnCargo,
    Build,
    Deconstruct,
    RepairBuilding,
    ResupplyClaim,
    CargoPickUp,
    Terraform,
    DeployDeployable,
    StoreDeployable,
    Sleep,
    Teleport,
    Death,
    Climb,
    UseItem,
    Craft,
    ConvertItems,
    PlayerMove,
    DeployableMove,
    ResupplyEmpireNode,
    SetHome,
    UseElevator,
    MobileEmote,
}

impl ActionType {
    pub fn get_action_name(&self) -> String {
        match self {
            ActionType::None => "Idle".to_string(),
            ActionType::Attack => "Attack".to_string(),
            ActionType::DestroyPaving => "DestroyPaving".to_string(),
            ActionType::StationaryEmote => "StationaryEmote".to_string(),
            ActionType::Extract => "Extract".to_string(),
            ActionType::PaveTile => "PaveTile".to_string(),
            ActionType::SpawnCargo => "SpawnCargo".to_string(),
            ActionType::Build => "Build".to_string(),
            ActionType::Deconstruct => "Deconstruct".to_string(),
            ActionType::RepairBuilding => "RepairBuilding".to_string(),
            ActionType::ResupplyClaim => "ResupplyClaim".to_string(),
            ActionType::CargoPickUp => "CargoPickUp".to_string(),
            ActionType::Terraform => "Terraform".to_string(),
            ActionType::DeployDeployable => "DeployDeployable".to_string(),
            ActionType::StoreDeployable => "StoreDeployable".to_string(),
            ActionType::Sleep => "Sleep".to_string(),
            ActionType::Teleport => "Teleport".to_string(),
            ActionType::Death => "Death".to_string(),
            ActionType::Climb => "Climb".to_string(),
            ActionType::UseItem => "UseItem".to_string(),
            ActionType::Craft => "Craft".to_string(),
            ActionType::ConvertItems => "ConvertItems".to_string(),
            ActionType::PlayerMove => "PlayerMove".to_string(),
            ActionType::DeployableMove => "DeployableMove".to_string(),
            ActionType::ResupplyEmpireNode => "ResupplyEmpireNode".to_string(),
            ActionType::SetHome => "SetHome".to_string(),
            ActionType::UseElevator => "UseElevator".to_string(),
            ActionType::MobileEmote => "MobileEmote".to_string(),
        }
    }
}

impl<'de> Deserialize<'de> for ActionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct InvalidatesTypeVisitor;

        impl<'de> Visitor<'de> for InvalidatesTypeVisitor {
            type Value = ActionType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a map with a single key representing the enum variant or an array")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                if let Some((key, _value)) = map.next_entry::<i32, serde_json::Value>()? {
                    match key {
                        0 => Ok(ActionType::None),
                        1 => Ok(ActionType::Attack),
                        2 => Ok(ActionType::DestroyPaving),
                        3 => Ok(ActionType::StationaryEmote),
                        4 => Ok(ActionType::Extract),
                        5 => Ok(ActionType::PaveTile),
                        6 => Ok(ActionType::SpawnCargo),
                        7 => Ok(ActionType::Build),
                        8 => Ok(ActionType::Deconstruct),
                        9 => Ok(ActionType::RepairBuilding),
                        10 => Ok(ActionType::ResupplyClaim),
                        11 => Ok(ActionType::CargoPickUp),
                        12 => Ok(ActionType::Terraform),
                        13 => Ok(ActionType::DeployDeployable),
                        14 => Ok(ActionType::StoreDeployable),
                        15 => Ok(ActionType::Sleep),
                        16 => Ok(ActionType::Teleport),
                        17 => Ok(ActionType::Death),
                        18 => Ok(ActionType::Climb),
                        19 => Ok(ActionType::UseItem),
                        20 => Ok(ActionType::Craft),
                        21 => Ok(ActionType::ConvertItems),
                        22 => Ok(ActionType::PlayerMove),
                        23 => Ok(ActionType::DeployableMove),
                        24 => Ok(ActionType::ResupplyEmpireNode),
                        25 => Ok(ActionType::SetHome),
                        26 => Ok(ActionType::UseElevator),
                        27 => Ok(ActionType::MobileEmote),
                        _ => Err(de::Error::custom("invalid enum variant")),
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
                        0 => Ok(ActionType::None),
                        1 => Ok(ActionType::Attack),
                        2 => Ok(ActionType::DestroyPaving),
                        3 => Ok(ActionType::StationaryEmote),
                        4 => Ok(ActionType::Extract),
                        5 => Ok(ActionType::PaveTile),
                        6 => Ok(ActionType::SpawnCargo),
                        7 => Ok(ActionType::Build),
                        8 => Ok(ActionType::Deconstruct),
                        9 => Ok(ActionType::RepairBuilding),
                        10 => Ok(ActionType::ResupplyClaim),
                        11 => Ok(ActionType::CargoPickUp),
                        12 => Ok(ActionType::Terraform),
                        13 => Ok(ActionType::DeployDeployable),
                        14 => Ok(ActionType::StoreDeployable),
                        15 => Ok(ActionType::Sleep),
                        16 => Ok(ActionType::Teleport),
                        17 => Ok(ActionType::Death),
                        18 => Ok(ActionType::Climb),
                        19 => Ok(ActionType::UseItem),
                        20 => Ok(ActionType::Craft),
                        21 => Ok(ActionType::ConvertItems),
                        22 => Ok(ActionType::PlayerMove),
                        23 => Ok(ActionType::DeployableMove),
                        24 => Ok(ActionType::ResupplyEmpireNode),
                        25 => Ok(ActionType::SetHome),
                        26 => Ok(ActionType::UseElevator),
                        27 => Ok(ActionType::MobileEmote),
                        _ => Err(de::Error::custom("invalid enum variant")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize() {
        let json =
            r#"[1,72057594038086099,[0,[]],[0,[]],[0,[]],1740412031898,0,[1,[]],[1,[]],false]"#;

        let parsed = serde_json::from_str::<Model>(json);

        assert_eq!(parsed.is_err(), false);

        let parsed = parsed.unwrap();

        assert_eq!(parsed.auto_id, 1);
        assert_eq!(parsed.entity_id, 72057594038086099);
        assert_eq!(parsed.start_time, 1740412031898);
        assert_eq!(parsed.duration, 0);
        assert_eq!(parsed.recipe_id, None);
        assert_eq!(parsed.client_cancel, false);
    }
}
