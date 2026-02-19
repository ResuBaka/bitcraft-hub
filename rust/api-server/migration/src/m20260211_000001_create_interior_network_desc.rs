use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InteriorNetworkDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InteriorNetworkDesc::BuildingId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InteriorNetworkDesc::DimensionType)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InteriorNetworkDesc::ChildInteriorInstances)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InteriorNetworkDesc::Region)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InteriorNetworkDesc::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum InteriorNetworkDesc {
    Table,
    BuildingId,
    DimensionType,
    ChildInteriorInstances,
    Region,
}
