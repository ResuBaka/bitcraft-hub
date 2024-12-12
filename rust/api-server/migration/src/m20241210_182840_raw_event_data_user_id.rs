use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RawEventData::Table)
                    .add_column(
                        ColumnDef::new(RawEventData::UserId)
                            .big_integer()
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
                    .table(RawEventData::Table)
                    .drop_column(RawEventData::UserId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RawEventData {
    Table,
    UserId,
}
