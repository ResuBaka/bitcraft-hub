use game_module::module_bindings;
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Deserialize, Serialize, TS)]
pub enum CharacterStatType {
    MaxHealth,
    MaxStamina,
    PassiveHealthRegenRate,
    PassiveStaminaRegenRate,
    MovementMultiplier,
    SprintMultiplier,
    SprintStaminaDrain,
    Armor,
    CooldownMultiplier,
    HuntingWeaponPower,
    Strength,
    ColdProtection,
    HeatProtection,
    Evasion,
    ToolbeltSlots,
    CraftingSpeed,
    GatheringSpeed,
    BuildingSpeed,
    SatiationRegenRate,
    MaxSatiation,
    DefenseLevel,
    ForestrySpeed,
    CarpentrySpeed,
    MasonrySpeed,
    MiningSpeed,
    SmithingSpeed,
    ScholarSpeed,
    LeatherworkingSpeed,
    HuntingSpeed,
    TailoringSpeed,
    FarmingSpeed,
    FishingSpeed,
    CookingSpeed,
    ForagingSpeed,
    ForestryPower,
    CarpentryPower,
    MasonryPower,
    MiningPower,
    SmithingPower,
    ScholarPower,
    LeatherworkingPower,
    HuntingPower,
    TailoringPower,
    FarmingPower,
    FishingPower,
    CookingPower,
    ForagingPower,
    ActiveHealthRegenRate,
    ActiveStaminaRegenRate,
    ClimbProficiency,
    ExperienceRate,
    Accuracy,
    MaxTeleportationEnergy,
    TeleportationEnergyRegenRate,
}

impl From<module_bindings::CharacterStatType> for CharacterStatType {
    fn from(value: module_bindings::CharacterStatType) -> Self {
        match value {
            module_bindings::CharacterStatType::MaxHealth => CharacterStatType::MaxHealth,
            module_bindings::CharacterStatType::MaxStamina => CharacterStatType::MaxStamina,
            module_bindings::CharacterStatType::PassiveHealthRegenRate => {
                CharacterStatType::PassiveHealthRegenRate
            }
            module_bindings::CharacterStatType::PassiveStaminaRegenRate => {
                CharacterStatType::PassiveStaminaRegenRate
            }
            module_bindings::CharacterStatType::MovementMultiplier => {
                CharacterStatType::MovementMultiplier
            }
            module_bindings::CharacterStatType::SprintMultiplier => {
                CharacterStatType::SprintMultiplier
            }
            module_bindings::CharacterStatType::SprintStaminaDrain => {
                CharacterStatType::SprintStaminaDrain
            }
            module_bindings::CharacterStatType::Armor => CharacterStatType::Armor,
            module_bindings::CharacterStatType::CooldownMultiplier => {
                CharacterStatType::CooldownMultiplier
            }
            module_bindings::CharacterStatType::HuntingWeaponPower => {
                CharacterStatType::HuntingWeaponPower
            }
            module_bindings::CharacterStatType::Strength => CharacterStatType::Strength,
            module_bindings::CharacterStatType::ColdProtection => CharacterStatType::ColdProtection,
            module_bindings::CharacterStatType::HeatProtection => CharacterStatType::HeatProtection,
            module_bindings::CharacterStatType::Evasion => CharacterStatType::Evasion,
            module_bindings::CharacterStatType::ToolbeltSlots => CharacterStatType::ToolbeltSlots,
            module_bindings::CharacterStatType::CraftingSpeed => CharacterStatType::CraftingSpeed,
            module_bindings::CharacterStatType::GatheringSpeed => CharacterStatType::GatheringSpeed,
            module_bindings::CharacterStatType::BuildingSpeed => CharacterStatType::BuildingSpeed,
            module_bindings::CharacterStatType::SatiationRegenRate => {
                CharacterStatType::SatiationRegenRate
            }
            module_bindings::CharacterStatType::MaxSatiation => CharacterStatType::MaxSatiation,
            module_bindings::CharacterStatType::DefenseLevel => CharacterStatType::DefenseLevel,
            module_bindings::CharacterStatType::ForestrySpeed => CharacterStatType::ForestrySpeed,
            module_bindings::CharacterStatType::CarpentrySpeed => CharacterStatType::CarpentrySpeed,
            module_bindings::CharacterStatType::MasonrySpeed => CharacterStatType::MasonrySpeed,
            module_bindings::CharacterStatType::MiningSpeed => CharacterStatType::MiningSpeed,
            module_bindings::CharacterStatType::SmithingSpeed => CharacterStatType::SmithingSpeed,
            module_bindings::CharacterStatType::ScholarSpeed => CharacterStatType::ScholarSpeed,
            module_bindings::CharacterStatType::LeatherworkingSpeed => {
                CharacterStatType::LeatherworkingSpeed
            }
            module_bindings::CharacterStatType::HuntingSpeed => CharacterStatType::HuntingSpeed,
            module_bindings::CharacterStatType::TailoringSpeed => CharacterStatType::TailoringSpeed,
            module_bindings::CharacterStatType::FarmingSpeed => CharacterStatType::FarmingSpeed,
            module_bindings::CharacterStatType::FishingSpeed => CharacterStatType::FishingSpeed,
            module_bindings::CharacterStatType::CookingSpeed => CharacterStatType::CookingSpeed,
            module_bindings::CharacterStatType::ForagingSpeed => CharacterStatType::ForagingSpeed,
            module_bindings::CharacterStatType::ForestryPower => CharacterStatType::ForestryPower,
            module_bindings::CharacterStatType::CarpentryPower => CharacterStatType::CarpentryPower,
            module_bindings::CharacterStatType::MasonryPower => CharacterStatType::MasonryPower,
            module_bindings::CharacterStatType::MiningPower => CharacterStatType::MiningPower,
            module_bindings::CharacterStatType::SmithingPower => CharacterStatType::SmithingPower,
            module_bindings::CharacterStatType::ScholarPower => CharacterStatType::ScholarPower,
            module_bindings::CharacterStatType::LeatherworkingPower => {
                CharacterStatType::LeatherworkingPower
            }
            module_bindings::CharacterStatType::HuntingPower => CharacterStatType::HuntingPower,
            module_bindings::CharacterStatType::TailoringPower => CharacterStatType::TailoringPower,
            module_bindings::CharacterStatType::FarmingPower => CharacterStatType::FarmingPower,
            module_bindings::CharacterStatType::FishingPower => CharacterStatType::FishingPower,
            module_bindings::CharacterStatType::CookingPower => CharacterStatType::CookingPower,
            module_bindings::CharacterStatType::ForagingPower => CharacterStatType::ForagingPower,
            module_bindings::CharacterStatType::ActiveHealthRegenRate => {
                CharacterStatType::ActiveHealthRegenRate
            }
            module_bindings::CharacterStatType::ActiveStaminaRegenRate => {
                CharacterStatType::ActiveStaminaRegenRate
            }
            module_bindings::CharacterStatType::ClimbProficiency => {
                CharacterStatType::ClimbProficiency
            }
            module_bindings::CharacterStatType::ExperienceRate => CharacterStatType::ExperienceRate,
            module_bindings::CharacterStatType::Accuracy => CharacterStatType::Accuracy,
            module_bindings::CharacterStatType::MaxTeleportationEnergy => {
                CharacterStatType::MaxTeleportationEnergy
            }
            module_bindings::CharacterStatType::TeleportationEnergyRegenRate => {
                CharacterStatType::TeleportationEnergyRegenRate
            }
        }
    }
}
