use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        manager
            .alter_table(
                Table::alter()
                    .table(CargoDesc::Table)
                    .modify_column(integer(CargoDesc::Id))
                    .to_owned(),
            )
        .await;

        manager
            .alter_table(
                Table::alter()
                    .table(CraftingRecipe::Table)
                    .modify_column(integer(CraftingRecipe::Id))
                    .to_owned(),
            )
        .await;


        manager
            .alter_table(
                Table::alter()
                    .table(ItemDesc::Table)
                    .modify_column(integer(ItemDesc::Id))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CargoDesc::Table)
                    .modify_column(big_integer(CargoDesc::Id))
                    .to_owned(),
            )
            .await;
        manager
            .alter_table(
                Table::alter()
                    .table(CraftingRecipe::Table)
                    .modify_column(big_integer(CraftingRecipe::Id))
                    .to_owned(),
            )
            .await;


        manager
            .alter_table(
                Table::alter()
                    .table(ItemDesc::Table)
                    .modify_column(big_integer(ItemDesc::Id))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CargoDesc {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ItemDesc {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CraftingRecipe {
    Table,
    Id,
}
