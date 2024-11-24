use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerState::TimePlayed).integer().not_null())
                    .col(
                        ColumnDef::new(PlayerState::SessionStartTimestamp)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerState::TimeSignedIn)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerState::SignInTimestamp)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerState::SignedIn).boolean().not_null())
                    .col(
                        ColumnDef::new(PlayerState::TeleportLocation)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerState::LastSharedClaim)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating PlayerState table");

        manager
            .create_table(
                Table::create()
                    .table(PlayerUsernameState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerUsernameState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerUsernameState::Username)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating PlayerUsernameState table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerState::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PlayerUsernameState::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    TeleportLocation,
    EntityId,
    TimePlayed,
    SessionStartTimestamp,
    TimeSignedIn,
    SignInTimestamp,
    LastSharedClaim,
    SignedIn,
}

#[derive(DeriveIden)]
enum PlayerUsernameState {
    Table,
    EntityId,
    Username,
}
