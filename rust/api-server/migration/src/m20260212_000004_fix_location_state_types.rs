use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Alter X, Z, and Dimension columns to big_integer()
        manager
            .alter_table(
                Table::alter()
                    .table(LocationState::Table)
                    .modify_column(ColumnDef::new(LocationState::X).big_integer().not_null())
                    .modify_column(ColumnDef::new(LocationState::Z).big_integer().not_null())
                    .modify_column(
                        ColumnDef::new(LocationState::Dimension)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Revert to integer() if needed (though not strictly necessary for this fix)
        manager
            .alter_table(
                Table::alter()
                    .table(LocationState::Table)
                    .modify_column(ColumnDef::new(LocationState::X).integer().not_null())
                    .modify_column(ColumnDef::new(LocationState::Z).integer().not_null())
                    .modify_column(
                        ColumnDef::new(LocationState::Dimension)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum LocationState {
    Table,
    X,
    Z,
    Dimension,
}
