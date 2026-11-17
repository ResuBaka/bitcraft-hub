use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PublicProgressiveActionState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PublicProgressiveActionState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PublicProgressiveActionState::BuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PublicProgressiveActionState::OwnerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PublicProgressiveActionState::Region)
                            .small_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PublicProgressiveActionState::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PublicProgressiveActionState {
    Table,
    EntityId,
    BuildingEntityId,
    OwnerEntityId,
    Region,
}
