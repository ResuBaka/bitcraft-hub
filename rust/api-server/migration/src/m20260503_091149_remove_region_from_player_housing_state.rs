use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerHousingState::Table)
                    .drop_column(PlayerHousingState::Region)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerHousingState::Table)
                    .add_column(
                        ColumnDef::new(PlayerHousingState::Region)
                            .small_integer()
                            .null()
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerHousingState {
    Table,
    Region,
}
