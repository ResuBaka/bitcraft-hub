use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(Table::truncate().table(TravelerTaskState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                TravelerTaskState::Table.to_string(),
                TravelerTaskState::Region.to_string(),
            )
            .await?
        {
            let building_nickname_state_alter = Table::alter()
                .table(TravelerTaskState::Table)
                .add_column(string(TravelerTaskState::Region))
                .to_owned();
            manager.alter_table(building_nickname_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("TravelerTaskStateRegionIndex")
                    .table(TravelerTaskState::Table)
                    .col(TravelerTaskState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let vault_state_alter = Table::alter()
            .table(TravelerTaskState::Table)
            .drop_column(TravelerTaskState::Region)
            .to_owned();
        manager.alter_table(vault_state_alter).await
    }
}

#[derive(DeriveIden)]
enum TravelerTaskState {
    Table,
    Region,
}
