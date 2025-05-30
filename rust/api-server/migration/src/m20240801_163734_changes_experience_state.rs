use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ItemDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ItemDesc::Id)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ItemDesc::Name).string().not_null())
                    .col(ColumnDef::new(ItemDesc::Description).string().not_null())
                    .col(ColumnDef::new(ItemDesc::Volume).integer().not_null())
                    .col(ColumnDef::new(ItemDesc::Durability).integer().not_null())
                    .col(
                        ColumnDef::new(ItemDesc::SecondaryKnowledgeId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ItemDesc::ModelAssetName).string().not_null())
                    .col(ColumnDef::new(ItemDesc::IconAssetName).string().not_null())
                    .col(ColumnDef::new(ItemDesc::Tier).integer().not_null())
                    .col(ColumnDef::new(ItemDesc::Tag).string().not_null())
                    .col(ColumnDef::new(ItemDesc::Rarity).json().not_null())
                    .col(
                        ColumnDef::new(ItemDesc::CompendiumEntry)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ItemDesc::ItemListId).integer().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating ItemDesc table");

        manager
            .create_table(
                Table::create()
                    .table(CargoDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CargoDesc::Id)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CargoDesc::Name).string().not_null())
                    .col(ColumnDef::new(CargoDesc::Description).string().not_null())
                    .col(ColumnDef::new(CargoDesc::Volume).integer().not_null())
                    .col(
                        ColumnDef::new(CargoDesc::SecondaryKnowledgeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CargoDesc::ModelAssetName)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CargoDesc::IconAssetName).string().not_null())
                    .col(
                        ColumnDef::new(CargoDesc::CarriedModelAssetName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CargoDesc::PickUpAnimationStart)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CargoDesc::PickUpAnimationEnd)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CargoDesc::DropAnimationStart)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CargoDesc::DropAnimationEnd)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CargoDesc::PickUpTime).float().not_null())
                    .col(ColumnDef::new(CargoDesc::PlaceTime).float().not_null())
                    .col(ColumnDef::new(CargoDesc::AnimatorState).string().not_null())
                    .col(
                        ColumnDef::new(CargoDesc::MovementModifier)
                            .float()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CargoDesc::BlocksPath).boolean().not_null())
                    .col(
                        ColumnDef::new(CargoDesc::OnDestroyYieldCargos)
                            .json()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CargoDesc::DespawnTime).float().not_null())
                    .col(ColumnDef::new(CargoDesc::Tier).integer().not_null())
                    .col(ColumnDef::new(CargoDesc::Tag).string().not_null())
                    .col(ColumnDef::new(CargoDesc::Rarity).json().not_null())
                    .col(
                        ColumnDef::new(CargoDesc::NotPickupable)
                            .boolean()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating CargoDescription table");

        manager
            .create_table(
                Table::create()
                    .table(Inventory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Inventory::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Inventory::Pockets).json().not_null())
                    .col(
                        ColumnDef::new(Inventory::InventoryIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Inventory::CargoIndex).integer().not_null())
                    .col(
                        ColumnDef::new(Inventory::OwnerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Inventory::PlayerOwnerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating Inventory table");

        manager
            .create_index(
                Index::create()
                    .name("inventory_owner_entity_id")
                    .table(Inventory::Table)
                    .col(Inventory::OwnerEntityId)
                    .to_owned(),
            )
            .await
            .expect("Creating inventory_owner_entity_id index");

        manager
            .create_index(
                Index::create()
                    .name("inventory_player_owner_entity_id")
                    .table(Inventory::Table)
                    .col(Inventory::PlayerOwnerEntityId)
                    .to_owned(),
            )
            .await
            .expect("Creating inventory_owner_entity_id index");

        manager
            .create_table(
                Table::create()
                    .table(DeployableState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DeployableState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DeployableState::OwnerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeployableState::ClaimEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeployableState::Direction)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeployableState::DeployableDescriptionId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeployableState::Nickname)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DeployableState::Hidden).boolean().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating DeployableState table");

        manager
            .create_index(
                Index::create()
                    .name("deployable_state_owner_id")
                    .table(DeployableState::Table)
                    .col(DeployableState::OwnerId)
                    .to_owned(),
            )
            .await
            .expect("Creating deployable_state_owner_id index");

        manager
            .create_index(
                Index::create()
                    .name("deployable_state_claim_entity_id")
                    .table(DeployableState::Table)
                    .col(DeployableState::ClaimEntityId)
                    .to_owned(),
            )
            .await
            .expect("Creating deployable_state_claim_entity_id index");

        manager
            .create_table(
                Table::create()
                    .table(BuildingState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BuildingState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BuildingState::ClaimEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingState::DirectionIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingState::BuildingDescriptionId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingState::ConstructedByPlayerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BuildingState::Nickname).string().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating BuildingState table");

        manager
            .create_index(
                Index::create()
                    .name("building_state_claim_entity_id")
                    .table(BuildingState::Table)
                    .col(BuildingState::ClaimEntityId)
                    .to_owned(),
            )
            .await
            .expect("Creating building_state_claim_entity_id index");

        manager
            .create_table(
                Table::create()
                    .table(BuildingDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BuildingDesc::Id)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BuildingDesc::Functions).json().not_null())
                    .col(ColumnDef::new(BuildingDesc::Name).string().not_null())
                    .col(
                        ColumnDef::new(BuildingDesc::Description)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::RestedBuffDuration)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::LightRadius)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::ModelAssetName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::IconAssetName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::Unenterable)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::Wilderness)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BuildingDesc::Footprint).json().not_null())
                    .col(ColumnDef::new(BuildingDesc::MaxHealth).integer().not_null())
                    .col(
                        ColumnDef::new(BuildingDesc::DefenseLevel)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BuildingDesc::Decay).float().not_null())
                    .col(ColumnDef::new(BuildingDesc::Maintenance).float().not_null())
                    .col(
                        ColumnDef::new(BuildingDesc::BuildPermission)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingDesc::InteractionLevel)
                            .json()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BuildingDesc::HasAction).boolean().not_null())
                    .col(
                        ColumnDef::new(BuildingDesc::ShowInCompendium)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BuildingDesc::IsRuins).boolean().not_null())
                    .col(
                        ColumnDef::new(BuildingDesc::NotDeconstructible)
                            .boolean()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating BuildingDesc table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ItemDesc::Table).to_owned())
            .await
            .expect("Dropping ItemDesc table");
        manager
            .drop_table(Table::drop().table(CargoDesc::Table).to_owned())
            .await
            .expect("Dropping CargoDesc table");
        manager
            .drop_table(Table::drop().table(Inventory::Table).to_owned())
            .await
            .expect("Dropping Inventory table");
        manager
            .drop_table(Table::drop().table(DeployableState::Table).to_owned())
            .await
            .expect("Dropping DeployableState table");
        manager
            .drop_table(Table::drop().table(BuildingState::Table).to_owned())
            .await
            .expect("Dropping BuildingState table");
        manager
            .drop_table(Table::drop().table(BuildingDesc::Table).to_owned())
            .await
            .expect("Dropping BuildingDesc table");

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ItemDesc {
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
enum CargoDesc {
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
    NotPickupable,
}

#[derive(DeriveIden)]
enum Inventory {
    Table,
    EntityId,
    Pockets,
    #[allow(clippy::enum_variant_names)]
    InventoryIndex,
    CargoIndex,
    OwnerEntityId,
    PlayerOwnerEntityId,
}

#[derive(DeriveIden)]
enum DeployableState {
    Table,
    EntityId,
    OwnerId,
    ClaimEntityId,
    Direction,
    DeployableDescriptionId,
    Nickname,
    Hidden,
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
    MaxHealth,
    DefenseLevel,
    Decay,
    Maintenance,
    BuildPermission,
    InteractionLevel,
    HasAction,
    ShowInCompendium,
    IsRuins,
    NotDeconstructible,
}
