use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .alter_table(
                Table::alter()
                    .table(CraftingRecipe::Table)
                    .add_column(integer(CraftingRecipe::ToolDurabilityLost))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
       manager
            .alter_table(
                Table::alter()
                    .table(CraftingRecipe::Table)
                    .drop_column(CraftingRecipe::ToolDurabilityLost)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CraftingRecipe {
    Table,
    ToolDurabilityLost
}
