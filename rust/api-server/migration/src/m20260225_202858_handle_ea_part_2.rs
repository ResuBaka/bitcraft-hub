use sea_orm_migration::{prelude::*, schema::*, };

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
                        ColumnDef::new(BuildingDesc::DestroyOnUnclaim)
                            .boolean()
                            .not_null(),
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
                    .drop_column(BuildingDesc::DestroyOnUnclaim)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BuildingDesc {
    Table,
    DestroyOnUnclaim,
}
