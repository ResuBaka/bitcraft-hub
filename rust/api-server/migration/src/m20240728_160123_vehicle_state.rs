use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .create_table(
                Table::create()
                    .table(ExperienceState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExperienceState::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExperienceState::SkillId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExperienceState::Experience)
                            .integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(ExperienceState::EntityId)
                            .col(ExperienceState::SkillId),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating ExperienceState table");

        manager
            .create_index(
                Index::create()
                    .name("experience_state_entity_id_skill_id")
                    .table(ExperienceState::Table)
                    .col(ExperienceState::EntityId)
                    .col(ExperienceState::SkillId)
                    .to_owned(),
            )
            .await
            .expect("Creating experience_state_entity_id_skill_id index");

        manager
            .create_table(
                Table::create()
                    .table(ClaimTechState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimTechState::EntityId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimTechState::Learned)
                            .array(ColumnType::BigInteger)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimTechState::Researching)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClaimTechState::StartTimestamp)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ClaimTechState::ScheduledId)
                            .json()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating ClaimTechState table");

        manager
            .create_table(
                Table::create()
                    .table(ClaimTechDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimTechDesc::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimTechDesc::Description)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimTechDesc::Tier).integer().not_null())
                    .col(
                        ColumnDef::new(ClaimTechDesc::SuppliesCost)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimTechDesc::ResearchTime)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimTechDesc::Requirements)
                            .json()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimTechDesc::Input).json().not_null())
                    .col(ColumnDef::new(ClaimTechDesc::Members).integer().not_null())
                    .col(ColumnDef::new(ClaimTechDesc::Area).integer().not_null())
                    .col(ColumnDef::new(ClaimTechDesc::Supply).integer().not_null())
                    .col(
                        ColumnDef::new(ClaimTechDesc::XpToMintHexCoin)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating ClaimTechDesc table");

        manager
            .create_table(
                Table::create()
                    .table(ClaimDescriptionState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClaimDescriptionState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::OwnerPlayerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::OwnerBuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Name)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Supplies)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::BuildingMaintenance)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Members)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::NumTiles)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::NumTileNeighbors)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Extensions)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Neutral)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Location)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::Treasury)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::XpGainedSinceLastCoinMinting)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::SuppliesPurchaseThreshold)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::SuppliesPurchasePrice)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimDescriptionState::BuildingDescriptionId)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating ClaimDescription table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(ExperienceState::Table).to_owned())
            .await
            .expect("Dropping ExperienceState table");

        manager
            .drop_table(Table::drop().table(ClaimTechDesc::Table).to_owned())
            .await
            .expect("Dropping ClaimTechDesc table");

        manager
            .drop_table(Table::drop().table(ClaimDescriptionState::Table).to_owned())
            .await
            .expect("Dropping ClaimDescriptionState table");

        manager
            .drop_table(Table::drop().table(ClaimTechState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ExperienceState {
    Table,
    EntityId,
    SkillId,
    Experience,
}

#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    EntityId,
    Learned,
    Researching,
    StartTimestamp,
    ScheduledId,
}

#[derive(DeriveIden)]
enum ClaimTechDesc {
    Table,
    Id,
    Description,
    Tier,
    SuppliesCost,
    ResearchTime,
    Requirements,
    Input,
    Members,
    Area,
    Supply,
    XpToMintHexCoin,
}

#[derive(DeriveIden)]
enum ClaimDescriptionState {
    Table,
    EntityId,
    OwnerPlayerEntityId,
    OwnerBuildingEntityId,
    Name,
    Supplies,
    BuildingMaintenance,
    Members,
    NumTiles,
    NumTileNeighbors,
    Extensions,
    Neutral,
    Location,
    Treasury,
    XpGainedSinceLastCoinMinting,
    SuppliesPurchaseThreshold,
    SuppliesPurchasePrice,
    BuildingDescriptionId,
}
