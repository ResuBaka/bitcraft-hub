use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ResourceDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ResourceDesc::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ResourceDesc::Name).string().not_null())
                    .col(
                        ColumnDef::new(ResourceDesc::Description)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ResourceDesc::Tier).integer().not_null())
                    .col(ColumnDef::new(ResourceDesc::Tag).string().not_null())
                    .col(ColumnDef::new(ResourceDesc::Rarity).integer().not_null())
                    .col(
                        ColumnDef::new(ResourceDesc::OnDestroyYield)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResourceDesc::OnDestroyYieldResourceId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResourceDesc::IconAssetName)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ExtractionRecipeDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::ResourceId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::ExtractedItemStacks)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::ToolRequirements)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::AllowUseHands)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::TimeRequirement)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExtractionRecipeDesc::StaminaRequirement)
                            .float()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResourceDesc::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExtractionRecipeDesc::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ResourceDesc {
    Table,
    Id,
    Name,
    Description,
    Tier,
    Tag,
    Rarity,
    OnDestroyYield,
    OnDestroyYieldResourceId,
    IconAssetName,
}

#[derive(DeriveIden)]
enum ExtractionRecipeDesc {
    Table,
    Id,
    ResourceId,
    ExtractedItemStacks,
    ToolRequirements,
    AllowUseHands,
    TimeRequirement,
    StaminaRequirement,
}
