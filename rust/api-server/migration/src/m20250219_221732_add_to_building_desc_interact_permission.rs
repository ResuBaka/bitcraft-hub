use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .add_column(
                        ColumnDef::new(BuildingDesc::InteractPermission)
                            .json()
                            .not_null()
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .drop_column(BuildingDesc::InteractPermission)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BuildingDesc {
    Table,
    InteractPermission,
}
