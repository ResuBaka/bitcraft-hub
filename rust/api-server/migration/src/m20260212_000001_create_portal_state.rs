use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PortalState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PortalState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PortalState::TargetBuildingEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortalState::DestinationX)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortalState::DestinationZ)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortalState::DestinationDimension)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PortalState::Enabled).boolean().not_null())
                    .col(
                        ColumnDef::new(PortalState::AllowDeployables)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PortalState::Region).text().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PortalState::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PortalState {
    Table,
    EntityId,
    TargetBuildingEntityId,
    DestinationX,
    DestinationZ,
    DestinationDimension,
    Enabled,
    AllowDeployables,
    Region,
}
