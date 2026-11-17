use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, table) in [
            ("progressive_action_owner_idx", "progressive_action_state"),
            ("passive_craft_owner_idx", "passive_craft_state"),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(Alias::new(table))
                        .col(Alias::new("owner_entity_id"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, table) in [
            ("progressive_action_owner_idx", "progressive_action_state"),
            ("passive_craft_owner_idx", "passive_craft_state"),
        ] {
            manager
                .drop_index(Index::drop().name(name).table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}
