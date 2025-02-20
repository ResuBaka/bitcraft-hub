use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ItemDesc::Table)
                    .add_column(
                        ColumnDef::new(ItemDesc::ConvertToOnDurabilityZero)
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
                    .table(ItemDesc::Table)
                    .drop_column(ItemDesc::ConvertToOnDurabilityZero)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ItemDesc {
    Table,
    ConvertToOnDurabilityZero,
}
