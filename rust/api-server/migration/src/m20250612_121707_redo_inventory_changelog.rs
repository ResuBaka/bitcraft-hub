use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Schema;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let builder = manager.get_database_backend();
        let db = manager.get_connection();
        let schema = Schema::new(builder);
        manager
            .drop_table(Table::drop().table(InventoryChangelog::Table).to_owned())
            .await?;
        db.execute(
            builder.build(&schema.create_table_from_entity(entity::inventory_changelog::Entity)),
        )
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogEntityId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::EntityId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");
        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogOldItemId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::OldItemId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogOldItemType")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::OldItemType)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogNewItemId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::NewItemId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogNewItemType")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::NewItemType)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogUserId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::UserId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let builder = manager.get_database_backend();
        let db = manager.get_connection();
        let schema = Schema::new(builder);
        manager
            .drop_table(Table::drop().table(InventoryChangelog::Table).to_owned())
            .await?;
        db.execute(
            builder.build(&schema.create_table_from_entity(entity::inventory_changelog::Entity)),
        )
        .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum InventoryChangelog {
    Table,
    EntityId,
    UserId,
    OldItemId,
    OldItemType,
    NewItemType,
    NewItemId,
}
