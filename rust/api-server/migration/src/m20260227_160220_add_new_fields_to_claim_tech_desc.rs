use crate::ColumnType::Integer;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(Table::truncate().table(ClaimTechDesc::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .add_column(string(ClaimTechDesc::Name).not_null().default(""))
                    .add_column(integer(ClaimTechDesc::TechType).not_null().default(0))
                    .add_column(array(ClaimTechDesc::UnlocksTechs, Integer))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .drop_column(ClaimTechDesc::Name)
                    .drop_column(ClaimTechDesc::TechType)
                    .drop_column(ClaimTechDesc::UnlocksTechs)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ClaimTechDesc {
    Table,
    Name,
    TechType,
    UnlocksTechs,
}
