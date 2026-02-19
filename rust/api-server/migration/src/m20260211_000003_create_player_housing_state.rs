use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerHousingState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerHousingState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::EntranceBuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::NetworkEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::ExitPortalEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::Rank)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::LockedUntil)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::IsEmpty)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::RegionIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerHousingState::Region)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerHousingState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerHousingState {
    Table,
    EntityId,
    EntranceBuildingEntityId,
    NetworkEntityId,
    ExitPortalEntityId,
    Rank,
    LockedUntil,
    IsEmpty,
    RegionIndex,
    Region,
}
