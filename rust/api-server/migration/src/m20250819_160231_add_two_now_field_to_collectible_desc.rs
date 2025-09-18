use crate::ColumnType::Integer;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column(
                CollectibleDesc::Table.to_string(),
                CollectibleDesc::RequiredKnowledgesToUse.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(CollectibleDesc::Table)
                        .drop_column(CollectibleDesc::RequiredKnowledgesToUse)
                        .to_owned(),
                )
                .await?;
        };
        if manager
            .has_column(
                CollectibleDesc::Table.to_string(),
                CollectibleDesc::RequiredKnowledgesToConvert.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(CollectibleDesc::Table)
                        .drop_column(CollectibleDesc::RequiredKnowledgesToConvert)
                        .to_owned(),
                )
                .await?;
        };

        manager
            .alter_table(
                Table::alter()
                    .table(CollectibleDesc::Table)
                    .add_column(array(CollectibleDesc::RequiredKnowledgesToUse, Integer))
                    .add_column(array(CollectibleDesc::RequiredKnowledgesToConvert, Integer))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CollectibleDesc::Table)
                    .drop_column(CollectibleDesc::RequiredKnowledgesToUse)
                    .drop_column(CollectibleDesc::RequiredKnowledgesToConvert)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CollectibleDesc {
    Table,
    RequiredKnowledgesToUse,
    RequiredKnowledgesToConvert,
}
