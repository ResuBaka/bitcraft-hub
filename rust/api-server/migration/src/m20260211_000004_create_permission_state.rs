use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PermissionState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PermissionState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PermissionState::OrdainedEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionState::AllowedEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PermissionState::Group).integer().not_null())
                    .col(ColumnDef::new(PermissionState::Rank).integer().not_null())
                    .col(ColumnDef::new(PermissionState::Region).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PermissionState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PermissionState {
    Table,
    EntityId,
    OrdainedEntityId,
    AllowedEntityId,
    Group,
    Rank,
    Region,
}
