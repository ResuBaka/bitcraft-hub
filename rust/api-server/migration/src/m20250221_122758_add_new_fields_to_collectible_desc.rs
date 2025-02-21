use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CollectibleDesc::Table)
                    .add_column(
                        ColumnDef::new(CollectibleDesc::RequiredKnowledgesToUse)
                            .integer()
                            .null()
                            .to_owned(),
                    )
                    .add_column(
                        ColumnDef::new(CollectibleDesc::RequiredKnowledgesToConvert)
                            .integer()
                            .null()
                            .to_owned(),
                    )
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
    RequiredKnowledgesToConvert
}
