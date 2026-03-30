use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProgressiveActionState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProgressiveActionState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::BuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::FunctionType)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::Progress)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::RecipeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::CraftCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::LastCritOutcome)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::OwnerEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::LockExpiration)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::Preparation)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProgressiveActionState::Region)
                            .string()
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
                    .table(ProgressiveActionState::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ProgressiveActionState {
    Table,
    EntityId,
    BuildingEntityId,
    FunctionType,
    Progress,
    RecipeId,
    CraftCount,
    LastCritOutcome,
    OwnerEntityId,
    LockExpiration,
    Preparation,
    Region,
}
