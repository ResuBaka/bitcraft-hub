use sea_orm_migration::prelude::*;


#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CraftingRecipe::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CraftingRecipe::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CraftingRecipe::Name).string().not_null())
                    .col(ColumnDef::new(CraftingRecipe::TimeRequirement).float().not_null())
                    .col(ColumnDef::new(CraftingRecipe::StaminaRequirement).float().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ToolDurabilityLost).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::BuildingRequirement).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::LevelRequirements).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ToolRequirements).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ConsumedItemStacks).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::DiscoveryTriggers).json().not_null())  
                    .col(ColumnDef::new(CraftingRecipe::RequiredKnowledges).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::RequiredClaimTechId).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::FullDiscoveryScore).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ExperiencePerProgress).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::AllowUseHands).boolean().not_null())

                    .col(ColumnDef::new(CraftingRecipe::CraftedItemStacks).json().not_null())
                    .col(ColumnDef::new(CraftingRecipe::IsPassive).boolean().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ActionsRequired).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::ToolMeshIndex).integer().not_null())
                    .col(ColumnDef::new(CraftingRecipe::RecipePerformanceId).integer().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating CraftingRecipe table");
            Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CraftingRecipe::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CraftingRecipe {
    Table,
    Id,
    Name,
    TimeRequirement,
    StaminaRequirement,
    ToolDurabilityLost,
    BuildingRequirement,
    LevelRequirements,
    ToolRequirements,
    ConsumedItemStacks,
    DiscoveryTriggers,
    RequiredKnowledges,
    RequiredClaimTechId,
    FullDiscoveryScore,
    ExperiencePerProgress,
    AllowUseHands,
    CraftedItemStacks,
    IsPassive,
    ActionsRequired,
    ToolMeshIndex,
    RecipePerformanceId

}