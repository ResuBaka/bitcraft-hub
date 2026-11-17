use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CraftActivity::Table)
                    .col(
                        ColumnDef::new(CraftActivity::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CraftActivity::Region)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CraftActivity::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CraftActivity::EventType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CraftActivity::ActorEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CraftActivity::ProgressiveActionEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CraftActivity::BuildingEntityId).big_integer())
                    .col(ColumnDef::new(CraftActivity::ClaimEntityId).big_integer())
                    .col(ColumnDef::new(CraftActivity::RecipeId).integer())
                    .col(ColumnDef::new(CraftActivity::CraftCount).integer())
                    .col(ColumnDef::new(CraftActivity::Progress).integer())
                    .col(ColumnDef::new(CraftActivity::IsCrit).boolean())
                    .to_owned(),
            )
            .await?;
        for (name, column) in [
            (
                "craft_activity_building_time_idx",
                CraftActivity::BuildingEntityId,
            ),
            (
                "craft_activity_claim_time_idx",
                CraftActivity::ClaimEntityId,
            ),
            (
                "craft_activity_project_idx",
                CraftActivity::ProgressiveActionEntityId,
            ),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(CraftActivity::Table)
                        .col(column)
                        .col(CraftActivity::Region)
                        .col(CraftActivity::Timestamp)
                        .col(CraftActivity::Id)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CraftActivity::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CraftActivity {
    Table,
    Id,
    Region,
    Timestamp,
    EventType,
    ActorEntityId,
    ProgressiveActionEntityId,
    BuildingEntityId,
    ClaimEntityId,
    RecipeId,
    CraftCount,
    Progress,
    IsCrit,
}
