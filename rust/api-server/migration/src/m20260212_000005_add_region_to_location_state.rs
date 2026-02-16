use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(LocationState::Table)
                    .add_column(
                        ColumnDef::new(LocationState::Region)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(LocationState::Table)
                    .drop_column(LocationState::Region)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum LocationState {
    Table,
    Region,
}
