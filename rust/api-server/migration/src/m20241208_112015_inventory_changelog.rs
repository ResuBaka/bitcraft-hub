use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table(InventoryChangelog::Table)
                    .col(
                        ColumnDef::new(InventoryChangelog::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::UserId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::ItemId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::TypeOfChange)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryChangelog::EventData)
                            .json_binary()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_changelog_entity_id")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::EntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_changelog_user_id")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_changelog_item_id")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::ItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_changelog_type_of_change")
                    .table(InventoryChangelog::Table)
                    .col(InventoryChangelog::TypeOfChange)
                    .to_owned(),
            )
            .await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "SELECT create_hypertable('{}', by_range('{}'));",
                match InventoryChangelog::Table.into_table_ref() {
                    TableRef::Table(table_ref) => table_ref.to_string(),
                    _ => panic!("Unexpected table ref type"),
                },
                InventoryChangelog::Timestamp.to_string(),
            )
            .as_str(),
        );

        db.execute(stmt).await?;

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
    EntityId,
    UserId,
    ItemId,
    Amount,
    TypeOfChange,
    EventData,
    Timestamp,
}
