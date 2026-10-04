use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ClaimEvent::Table)
                    .add_column(ColumnDef::new(ClaimEvent::MemberEntityId).big_integer())
                    .add_column(ColumnDef::new(ClaimEvent::PermissionsBefore).json_binary())
                    .add_column(ColumnDef::new(ClaimEvent::PermissionsAfter).json_binary())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ClaimEvent::Table)
                    .drop_column(ClaimEvent::MemberEntityId)
                    .drop_column(ClaimEvent::PermissionsBefore)
                    .drop_column(ClaimEvent::PermissionsAfter)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ClaimEvent {
    Table,
    MemberEntityId,
    PermissionsBefore,
    PermissionsAfter,
}
