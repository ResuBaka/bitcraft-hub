use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DimensionDescriptionState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DimensionDescriptionState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionNetworkEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::CollapseTimestamp)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::InteriorInstanceId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionPositionLargeX)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionPositionLargeZ)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionSizeLargeX)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionSizeLargeZ)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::DimensionType)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DimensionDescriptionState::Region)
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
                    .table(DimensionDescriptionState::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DimensionDescriptionState {
    Table,
    EntityId,
    DimensionNetworkEntityId,
    CollapseTimestamp,
    InteriorInstanceId,
    DimensionPositionLargeX,
    DimensionPositionLargeZ,
    DimensionSizeLargeX,
    DimensionSizeLargeZ,
    DimensionId,
    DimensionType,
    Region,
}
