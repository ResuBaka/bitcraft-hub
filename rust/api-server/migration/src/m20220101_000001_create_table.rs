use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Location::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Location::EntityId)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Location::ChunkIndex).big_unsigned().not_null())
                    .col(ColumnDef::new(Location::X).integer().not_null())
                    .col(ColumnDef::new(Location::Z).integer().not_null())
                    .col(ColumnDef::new(Location::Dimension).integer().not_null())
                    .to_owned(),
            )
            .await.expect("Creating Location table");

        manager
            .create_table(
                Table::create()
                    .table(PlayerState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerState::EntityId)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerState::SerialId).integer().not_null())
                    .col(ColumnDef::new(PlayerState::Username).string().not_null())
                    .col(ColumnDef::new(PlayerState::TimePlayed).integer().not_null())
                    .col(ColumnDef::new(PlayerState::SessionStartTimestamp).integer().not_null())
                    .col(ColumnDef::new(PlayerState::TimeSignedIn).integer().not_null())
                    .col(ColumnDef::new(PlayerState::SignInTimestamp).integer().not_null())
                    .col(ColumnDef::new(PlayerState::SignedIn).boolean().not_null())
                    .col(ColumnDef::new(PlayerState::UnmannedVehicleCoords).json().not_null())
                    .col(ColumnDef::new(PlayerState::DestinationMarker).json().not_null())
                    .col(ColumnDef::new(PlayerState::FavoriteCraftingRecipes).json().not_null())
                    .col(ColumnDef::new(PlayerState::TeleportLocation).json().not_null())
                    .col(ColumnDef::new(PlayerState::LightRadius).float().not_null())
                    .col(ColumnDef::new(PlayerState::AccessLevel).integer().not_null())
                    .col(ColumnDef::new(PlayerState::LastSharedClaim).integer().not_null())
                    .to_owned(),
            )
            .await.expect("Creating PlayerState table");

        manager
            .create_table(
                Table::create()
                    .table(CraftingRecipe::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CraftingRecipe::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CraftingRecipe::Name).string().not_null())
                    .col(ColumnDef::new(CraftingRecipe::TimeRequirement).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::StaminaRequirement).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::BuildingRequirement).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::LevelRequirements).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ToolRequirements).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ConsumedItemStacks).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::DiscoveryTriggers).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::RequiredKnowledges).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::RequiredClaimTechId).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::FullDiscoveryScore).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::CompletionExperience).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::AllowUseHands).boolean().not_null())
                    .col(ColumnDef::new(CraftingRecipe::CraftedItemStacks).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::IsPassive).boolean().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ActionsRequired).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ToolMeshIndex).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::AnimationStart).string().not_null())
                    .col(ColumnDef::new(CraftingRecipe::AnimationEnd).string().not_null())
                    .to_owned(),
            )
            .await.expect("Creating CraftingRecipe table");

        manager
            .create_table(
                Table::create()
                    .table(SkillDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillDesc::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillDesc::Name).string().not_null())
                    .col(ColumnDef::new(SkillDesc::Description).string().not_null())
                    .col(ColumnDef::new(SkillDesc::IconAssetName).string().not_null())
                    .col(ColumnDef::new(SkillDesc::Title).string().not_null())
                    .to_owned(),
            ).await.expect("Creating SkillDesc table");

        manager
            .create_table(
                Table::create()
                    .table(TradeOrder::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TradeOrder::EntityId)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TradeOrder::BuildingEntityId).big_unsigned().not_null())
                    .col(ColumnDef::new(TradeOrder::RemainingStock).json().not_null())
                    .col(ColumnDef::new(TradeOrder::OfferItems).json().not_null())
                    .col(ColumnDef::new(TradeOrder::OfferCargoId).json().not_null())
                    .col(ColumnDef::new(TradeOrder::RequiredItems).json().not_null())
                    .col(ColumnDef::new(TradeOrder::RequiredCargoId).json().not_null())
                    .to_owned(),
            ).await.expect("Creating TradeOrder table");

        manager.create_table(
            Table::create()
                .table(UserState::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(UserState::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(UserState::Identity).string().not_null())
                .to_owned(),
        ).await.expect("Creating UserState table");

        manager.create_table(
            Table::create()
                .table(Item::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Item::Id)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Item::Name).string().not_null())
                .col(ColumnDef::new(Item::Description).string().not_null())
                .col(ColumnDef::new(Item::Volume).integer().not_null())
                .col(ColumnDef::new(Item::Durability).integer().not_null())
                .col(ColumnDef::new(Item::SecondaryKnowledgeId).integer().not_null())
                .col(ColumnDef::new(Item::ModelAssetName).string().not_null())
                .col(ColumnDef::new(Item::IconAssetName).string().not_null())
                .col(ColumnDef::new(Item::Tier).integer().not_null())
                .col(ColumnDef::new(Item::Tag).string().not_null())
                .col(ColumnDef::new(Item::Rarity).json().not_null())
                .col(ColumnDef::new(Item::CompendiumEntry).boolean().not_null())
                .col(ColumnDef::new(Item::ItemListId).integer().not_null())
                .to_owned(),
        ).await.expect("Creating Item table");

        manager.create_table(
            Table::create()
                .table(Inventory::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Inventory::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Inventory::Pockets).json().not_null())
                .col(ColumnDef::new(Inventory::InventoryIndex).integer().not_null())
                .col(ColumnDef::new(Inventory::CargoIndex).integer().not_null())
                .col(ColumnDef::new(Inventory::OwnerEntityId).big_unsigned().not_null())
                .to_owned(),
        ).await.expect("Creating Inventory table");

        manager.create_table(
            Table::create()
                .table(ExperienceState::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ExperienceState::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(ExperienceState::SkillId).integer().not_null())
                .col(ColumnDef::new(ExperienceState::Experience).integer().not_null())
                .to_owned(),
        ).await.expect("Creating ExperienceState table");

        manager.create_table(
            Table::create()
                .table(Equipment::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Equipment::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Equipment::EquipmentSlots).json().not_null())
                .to_owned(),
        ).await.expect("Creating Equipment table");

        manager.create_table(
            Table::create()
                .table(ClaimDescription::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ClaimDescription::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(ClaimDescription::OwnerPlayerEntityId).big_unsigned().not_null())
                .col(ColumnDef::new(ClaimDescription::OwnerBuildingEntityId).big_unsigned().not_null())
                .col(ColumnDef::new(ClaimDescription::Name).string().not_null())
                .col(ColumnDef::new(ClaimDescription::Supplies).integer().not_null())
                .col(ColumnDef::new(ClaimDescription::BuildingMaintenance).integer().not_null())
                .col(ColumnDef::new(ClaimDescription::Members).json().not_null())
                .col(ColumnDef::new(ClaimDescription::Tiles).integer().not_null())
                .col(ColumnDef::new(ClaimDescription::Extensions).integer().not_null())
                .col(ColumnDef::new(ClaimDescription::Neutral).boolean().not_null())
                .col(ColumnDef::new(ClaimDescription::Location).json().not_null())
                .col(ColumnDef::new(ClaimDescription::Treasury).integer().not_null())
                .to_owned(),
            )
            .await.expect("Creating ClaimDescription table");

        manager
            .create_table(
                Table::create()
                    .table(CargoState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CargoState::EntityId)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CargoState::SpawnTimestamp).integer().not_null())
                    .col(ColumnDef::new(CargoState::DescriptionId).integer().not_null())
                    .col(ColumnDef::new(CargoState::Direction).integer().not_null())
                    .to_owned(),
            )
            .await.expect("Creating CargoState table");

        manager.create_table(
            Table::create()
                .table(CargoDescription::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(CargoDescription::Id)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(CargoDescription::Name).string().not_null())
                .col(ColumnDef::new(CargoDescription::Description).string().not_null())
                .col(ColumnDef::new(CargoDescription::Volume).integer().not_null())
                .col(ColumnDef::new(CargoDescription::SecondaryKnowledgeId).integer().not_null())
                .col(ColumnDef::new(CargoDescription::ModelAssetName).string().not_null())
                .col(ColumnDef::new(CargoDescription::IconAssetName).string().not_null())
                .col(ColumnDef::new(CargoDescription::CarriedModelAssetName).string().not_null())
                .col(ColumnDef::new(CargoDescription::PickUpAnimationStart).string().not_null())
                .col(ColumnDef::new(CargoDescription::PickUpAnimationEnd).string().not_null())
                .col(ColumnDef::new(CargoDescription::DropAnimationStart).string().not_null())
                .col(ColumnDef::new(CargoDescription::DropAnimationEnd).string().not_null())
                .col(ColumnDef::new(CargoDescription::PickUpTime).integer().not_null())
                .col(ColumnDef::new(CargoDescription::PlaceTime).integer().not_null())
                .col(ColumnDef::new(CargoDescription::AnimatorState).string().not_null())
                .col(ColumnDef::new(CargoDescription::MovementModifier).integer().not_null())
                .col(ColumnDef::new(CargoDescription::BlocksPath).boolean().not_null())
                .col(ColumnDef::new(CargoDescription::OnDestroyYieldCargos).json().not_null())
                .col(ColumnDef::new(CargoDescription::DespawnTime).integer().not_null())
                .col(ColumnDef::new(CargoDescription::Tier).integer().not_null())
                .col(ColumnDef::new(CargoDescription::Tag).string().not_null())
                .col(ColumnDef::new(CargoDescription::Rarity).json().not_null())
                .to_owned(),
            ).await.expect("Creating CargoDescription table");

        manager.create_table(
            Table::create()
                .table(BuildingState::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(BuildingState::EntityId)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(BuildingState::ClaimEntityId).big_unsigned().not_null())
                .col(ColumnDef::new(BuildingState::DirectionIndex).integer().not_null())
                .col(ColumnDef::new(BuildingState::BuildingDescriptionId).integer().not_null())
                .col(ColumnDef::new(BuildingState::ConstructedByPlayerEntityId).big_unsigned().not_null())
                .col(ColumnDef::new(BuildingState::Nickname).string().not_null())
                .to_owned(),
        ).await.expect("Creating BuildingState table");

        manager.create_table(
            Table::create()
                .table(BuildingDesc::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(BuildingDesc::Id)
                        .big_unsigned()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(BuildingDesc::Functions).json().not_null())
                .col(ColumnDef::new(BuildingDesc::Name).string().not_null())
                .col(ColumnDef::new(BuildingDesc::Description).string().not_null())
                .col(ColumnDef::new(BuildingDesc::RestedBuffDuration).integer().not_null())
                .col(ColumnDef::new(BuildingDesc::LightRadius).integer().not_null())
                .col(ColumnDef::new(BuildingDesc::ModelAssetName).string().not_null())
                .col(ColumnDef::new(BuildingDesc::IconAssetName).string().not_null())
                .col(ColumnDef::new(BuildingDesc::Unenterable).boolean().not_null())
                .col(ColumnDef::new(BuildingDesc::Wilderness).boolean().not_null())
                .col(ColumnDef::new(BuildingDesc::Footprint).json().not_null())
                .to_owned(),
            ).await.expect("Creating BuildingDesc table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Location::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PlayerState::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CraftingRecipe::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SkillDesc::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TradeOrder::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Item::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Inventory::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ExperienceState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Equipment::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ClaimDescription::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(CargoState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(CargoDescription::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(BuildingState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(BuildingDesc::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Location {
    Table,
    EntityId,
    ChunkIndex,
    X,
    Z,
    Dimension,
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    EntityId,
    SerialId,
    Username,
    TimePlayed,
    SessionStartTimestamp,
    TimeSignedIn,
    SignInTimestamp,
    SignedIn,
    UnmannedVehicleCoords,
    DestinationMarker,
    FavoriteCraftingRecipes,
    TeleportLocation,
    LightRadius,
    AccessLevel,
    LastSharedClaim,
}

#[derive(DeriveIden)]
enum CraftingRecipe {
    Table,
    Id,
    Name,
    TimeRequirement,
    StaminaRequirement,
    BuildingRequirement,
    LevelRequirements,
    ToolRequirements,
    ConsumedItemStacks,
    DiscoveryTriggers,
    RequiredKnowledges,
    RequiredClaimTechId,
    FullDiscoveryScore,
    CompletionExperience,
    AllowUseHands,
    CraftedItemStacks,
    IsPassive,
    ActionsRequired,
    ToolMeshIndex,
    AnimationStart,
    AnimationEnd,
}


#[derive(DeriveIden)]
enum SkillDesc {
    Table,
    Id,
    Name,
    Description,
    IconAssetName,
    Title,
}

#[derive(DeriveIden)]
enum TradeOrder {
    Table,
    EntityId,
    BuildingEntityId,
    RemainingStock,
    OfferItems,
    OfferCargoId,
    RequiredItems,
    RequiredCargoId,
}

#[derive(DeriveIden)]
enum UserState {
    Table,
    EntityId,
    Identity
}

#[derive(DeriveIden)]
enum Item {
    Table,
    Id,
    Name,
    Description,
    Volume,
    Durability,
    SecondaryKnowledgeId,
    ModelAssetName,
    IconAssetName,
    Tier,
    Tag,
    Rarity,
    CompendiumEntry,
    ItemListId,
}

#[derive(DeriveIden)]
enum Inventory {
    Table,
    EntityId,
    Pockets,
    InventoryIndex,
    CargoIndex,
    OwnerEntityId,
}

#[derive(DeriveIden)]
enum ExperienceState {
    Table,
    EntityId,
    SkillId,
    Experience,
}

#[derive(DeriveIden)]
enum Equipment {
    Table,
    EntityId,
    EquipmentSlots,
}

#[derive(DeriveIden)]
enum ClaimDescription {
    Table,
    EntityId,
    OwnerPlayerEntityId,
    OwnerBuildingEntityId,
    Name,
    Supplies,
    BuildingMaintenance,
    Members,
    Tiles,
    Extensions,
    Neutral,
    Location,
    Treasury,
}

#[derive(DeriveIden)]
enum CargoState {
    Table,
    EntityId,
    SpawnTimestamp,
    DescriptionId,
    Direction,
}

#[derive(DeriveIden)]
enum CargoDescription {
    Table,
    Id,
    Name,
    Description,
    Volume,
    SecondaryKnowledgeId,
    ModelAssetName,
    IconAssetName,
    CarriedModelAssetName,
    PickUpAnimationStart,
    PickUpAnimationEnd,
    DropAnimationStart,
    DropAnimationEnd,
    PickUpTime,
    PlaceTime,
    AnimatorState,
    MovementModifier,
    BlocksPath,
    OnDestroyYieldCargos,
    DespawnTime,
    Tier,
    Tag,
    Rarity,
}

#[derive(DeriveIden)]
enum BuildingState {
    Table,
    EntityId,
    ClaimEntityId,
    DirectionIndex,
    BuildingDescriptionId,
    ConstructedByPlayerEntityId,
    Nickname,
}

#[derive(DeriveIden)]
enum BuildingDesc {
    Table,
    Id,
    Functions,
    Name,
    Description,
    RestedBuffDuration,
    LightRadius,
    ModelAssetName,
    IconAssetName,
    Unenterable,
    Wilderness,
    Footprint,
}
