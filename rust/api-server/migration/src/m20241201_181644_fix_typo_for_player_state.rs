use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .rename_column(PlayerState::LastSharedClaim, PlayerState::LastShardClaim)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .rename_column(PlayerState::LastShardClaim, PlayerState::LastSharedClaim)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    LastShardClaim,
    LastSharedClaim,
}
