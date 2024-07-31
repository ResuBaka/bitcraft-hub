use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(ClaimTechState::Learned).json().not_null())
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
                        ColumnDef::new(ClaimTechState::CancelToken)
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
                    .col(ColumnDef::new(ClaimTechDesc::Members).json().not_null())
                    .col(ColumnDef::new(ClaimTechDesc::Area).integer().not_null())
                    .col(ColumnDef::new(ClaimTechDesc::Supply).integer().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating ClaimTechDesc table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClaimTechState::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ClaimTechDesc::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    EntityId,
    Learned,
    Researching,
    StartTimestamp,
    ResearchTime,
    CancelToken,
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
}
