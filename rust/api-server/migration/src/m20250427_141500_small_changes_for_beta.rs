use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .add_column(
                        integer(PlayerState::TravelerTasksExpiration)
                            .not_null()
                            .default(0),
                    )
                    .drop_column(PlayerState::LastShardClaim)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingState::Table)
                    .drop_column(BuildingState::Nickname)
                    .to_owned(),
            )
            .await?;

        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::StartTimestamp.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::StartTimestamp)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(ClaimState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimState::OwnerPlayerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimState::OwnerBuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimState::Name).string().not_null())
                    .col(ColumnDef::new(ClaimState::Neutral).boolean().not_null())
                    .col(ColumnDef::new(ClaimState::Region).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ClaimMemberState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimMemberState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimMemberState::ClaimEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimMemberState::PlayerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimMemberState::UserName).string().not_null())
                    .col(
                        ColumnDef::new(ClaimMemberState::InventoryPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimMemberState::BuildPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimMemberState::OfficerPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimMemberState::CoOwnerPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimMemberState::Region).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ClaimLocalState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimLocalState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ClaimLocalState::Supplies).integer().not_null())
                    .col(
                        ColumnDef::new(ClaimLocalState::BuildingMaintenance)
                            .float()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimLocalState::NumTiles).integer().not_null())
                    .col(
                        ColumnDef::new(ClaimLocalState::NumTileNeighbors)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimLocalState::Location).json())
                    .col(ColumnDef::new(ClaimLocalState::Treasury).integer().not_null())
                    .col(
                        ColumnDef::new(ClaimLocalState::XpGainedSinceLastCoinMinting)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimLocalState::SuppliesPurchaseThreshold)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimLocalState::SuppliesPurchasePrice)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimLocalState::BuildingDescriptionId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimLocalState::Region).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .truncate_table(Table::truncate().table(ClaimTechState::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(json_binary(ClaimTechState::StartTimestamp))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .modify_column(array(ClaimTechState::Learned, ColumnType::Integer))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .rename_column(ClaimTechDesc::Supply, ClaimTechDesc::Supplies)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .modify_column(integer(ClaimTechDesc::Id).default(0))
                    .to_owned(),
            )
            .await?;

        if !manager.has_table(ClaimState::Table.to_string()).await? {
            manager
                .create_table(
                    Table::create()
                        .table(ClaimState::Table)
                        .col(big_unsigned(ClaimState::EntityId))
                        .col(big_unsigned(ClaimState::OwnerPlayerEntityId))
                        .col(big_unsigned(ClaimState::OwnerBuildingEntityId))
                        .col(string(ClaimState::Name).default(""))
                        .col(boolean(ClaimState::Neutral).default(false))
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .add_column(
                        boolean(BuildingDesc::IgnoreDamage)
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .drop_column(PlayerState::TravelerTasksExpiration)
                    .add_column(integer(PlayerState::LastShardClaim).not_null().default(0))
                    .to_owned(),
            )
            .await?;

        let mut player_state_down = Table::alter().table(PlayerState::Table).to_owned();

        if manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::TravelerTasksExpiration.to_string(),
            )
            .await?
        {
            player_state_down = player_state_down
                .drop_column(PlayerState::TravelerTasksExpiration)
                .to_owned();
        }

        if !manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::LastShardClaim.to_string(),
            )
            .await?
        {
            player_state_down = player_state_down
                .add_column(integer(PlayerState::LastShardClaim).not_null().default(0))
                .to_owned();
        }

        let player_state_down = player_state_down.to_owned();

        if !manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::LastShardClaim.to_string(),
            )
            .await?
            || manager
                .has_column(
                    PlayerState::Table.to_string(),
                    PlayerState::TravelerTasksExpiration.to_string(),
                )
                .await?
        {
            manager.alter_table(player_state_down).await?;
        }

        if manager.has_table(ClaimState::Table.to_string()).await? {
            manager
                .drop_table(Table::drop().table(ClaimState::Table).to_owned())
                .await?;
        }

        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::StartTimestamp.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::StartTimestamp)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(big_integer(ClaimTechState::StartTimestamp).default(0))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .rename_column(ClaimTechDesc::Supplies, ClaimTechDesc::Supply)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .modify_column(big_integer(ClaimTechDesc::Id).default(0))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingState::Table)
                    .add_column(string(BuildingState::Nickname).not_null().default(""))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(ClaimState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ClaimMemberState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ClaimLocalState::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .modify_column(array(ClaimTechState::Learned, ColumnType::BigInteger))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .drop_column(BuildingDesc::IgnoreDamage)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    TravelerTasksExpiration,
    LastShardClaim,
}

#[derive(DeriveIden)]
enum BuildingDesc {
    Table,
    IgnoreDamage,
}

#[derive(DeriveIden)]
enum BuildingState {
    Table,
    Nickname,
}

#[derive(DeriveIden)]
enum ClaimTechDesc {
    Table,
    Id,
    Supplies,
    Supply,
}

#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    StartTimestamp,
    Learned,
}

#[derive(DeriveIden)]
enum ClaimState {
    Table,
    EntityId,
    OwnerPlayerEntityId,
    OwnerBuildingEntityId,
    Name,
    Neutral,
    Region,
}

#[derive(DeriveIden)]
enum ClaimMemberState {
    Table,
    EntityId,
    ClaimEntityId,
    PlayerEntityId,
    UserName,
    InventoryPermission,
    BuildPermission,
    OfficerPermission,
    CoOwnerPermission,
    Region,
}

#[derive(DeriveIden)]
enum ClaimLocalState {
    Table,
    EntityId,
    Supplies,
    BuildingMaintenance,
    NumTiles,
    NumTileNeighbors,
    Location,
    Treasury,
    XpGainedSinceLastCoinMinting,
    SuppliesPurchaseThreshold,
    SuppliesPurchasePrice,
    BuildingDescriptionId,
    Region,
}
