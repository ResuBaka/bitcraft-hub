use crate::ColumnType::Integer;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(ItemListDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ItemListDesc::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ItemListDesc::Name).string().not_null())
                    .col(
                        ColumnDef::new(ItemListDesc::Possibilities)
                            .json()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating PlayerState table");

        manager
            .alter_table(
                Table::alter()
                    .table(CraftingRecipe::Table)
                    .drop_column(CraftingRecipe::DiscoveryTriggers)
                    .add_column(array(CraftingRecipe::DiscoveryTriggers, Integer))
                    .drop_column(CraftingRecipe::RequiredKnowledges)
                    .add_column(array(CraftingRecipe::RequiredKnowledges, Integer))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CraftingRecipe {
    Table,
    DiscoveryTriggers,
    RequiredKnowledges,
}

#[derive(DeriveIden)]
enum ItemListDesc {
    Table,
    Id,
    Name,
    Possibilities,
}
