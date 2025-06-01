use crate::ColumnType::{BigInteger, Integer, Json};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::ScheduledId.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::ScheduledId)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(big_integer_null(ClaimTechState::ScheduledId))
                    .to_owned(),
            )
            .await?;

        if manager
            .has_column(
                ClaimTechDesc::Table.to_string(),
                ClaimTechDesc::Requirements.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechDesc::Table)
                        .drop_column(ClaimTechDesc::Requirements)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .add_column(array_null(ClaimTechDesc::Requirements, Integer))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::ScheduledId.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::ScheduledId)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(json_null(ClaimTechState::ScheduledId))
                    .to_owned(),
            )
            .await?;

        if manager
            .has_column(
                ClaimTechDesc::Table.to_string(),
                ClaimTechDesc::Requirements.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechDesc::Table)
                        .drop_column(ClaimTechDesc::Requirements)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .add_column(ColumnDef::new(ClaimTechDesc::Requirements).json().null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    ScheduledId,
}

#[derive(DeriveIden)]
enum ClaimTechDesc {
    Table,
    Requirements,
}
