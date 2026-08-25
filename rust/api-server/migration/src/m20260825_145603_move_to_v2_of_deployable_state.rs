use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260825_145603_move_to_v2_of_deployable_state"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(Table::truncate().table(DeployableState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                DeployableState::Table.to_string(),
                DeployableState::AppearanceOverrideId.to_string(),
            )
            .await?
        {
            let building_nickname_state_alter = Table::alter()
                .table(DeployableState::Table)
                .add_column(integer(DeployableState::AppearanceOverrideId))
                .to_owned();
            manager.alter_table(building_nickname_state_alter).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let building_nickname_state_alter = Table::alter()
            .table(DeployableState::Table)
            .drop_column(DeployableState::AppearanceOverrideId)
            .to_owned();
        manager.alter_table(building_nickname_state_alter).await
    }
}

#[derive(DeriveIden)]
enum DeployableState {
    Table,
    AppearanceOverrideId,
}
