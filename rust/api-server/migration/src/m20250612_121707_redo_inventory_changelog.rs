use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InventoryChangelog::Table).to_owned())
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(InventoryChangelog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryChangelog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(InventoryChangelog::UserId).big_integer())
                    .col(
                        ColumnDef::new(InventoryChangelog::PocketNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(InventoryChangelog::OldItemType).integer())
                    .col(ColumnDef::new(InventoryChangelog::OldItemId).integer())
                    .col(ColumnDef::new(InventoryChangelog::OldItemQuantity).integer())
                    .col(ColumnDef::new(InventoryChangelog::NewItemType).integer())
                    .col(ColumnDef::new(InventoryChangelog::NewItemId).integer())
                    .col(ColumnDef::new(InventoryChangelog::NewItemQuantity).integer())
                    .col(
                        ColumnDef::new(InventoryChangelog::TypeOfChange)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk-inventory_changelog")
                            .col(InventoryChangelog::Id)
                            .col(InventoryChangelog::Timestamp),
                    )
                    .to_owned(),
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
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogOldItemId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::OldItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogOldItemType")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::OldItemType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogNewItemId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::NewItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogNewItemType")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::NewItemType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("InventoryChangelogUserId")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InventoryChangelog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum InventoryChangelog {
    Table,
    Id,
    EntityId,
    UserId,
    PocketNumber,
    OldItemType,
    OldItemId,
    OldItemQuantity,
    NewItemType,
    NewItemId,
    NewItemQuantity,
    TypeOfChange,
    Timestamp,
}
