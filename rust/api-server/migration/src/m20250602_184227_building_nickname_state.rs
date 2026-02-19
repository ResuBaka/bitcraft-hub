use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BuildingNicknameState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BuildingNicknameState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BuildingNicknameState::Nickname)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BuildingNicknameState::Region)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BuildingNicknameState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BuildingNicknameState {
    Table,
    EntityId,
    Nickname,
    Region,
}
