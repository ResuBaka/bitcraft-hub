use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VaultState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VaultState::EntityId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(VaultState::Shards).integer().not_null())
                    .to_owned(),
            )
            .await
            .expect("Couldn't create VaultState table");

        manager
            .create_table(
                Table::create()
                    .table(VaultStateCollectibles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VaultStateCollectibles::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VaultStateCollectibles::Id)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VaultStateCollectibles::Activated)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VaultStateCollectibles::Count)
                            .integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(VaultStateCollectibles::EntityId)
                            .col(VaultStateCollectibles::Id),
                    )
                    .to_owned(),
            )
            .await
            .expect("Couldn't create VaultStateCollectibles table");

        manager
            .create_index(
                Index::create()
                    .name("VaultStateEntityId")
                    .table(VaultState::Table)
                    .col(VaultState::EntityId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("VaultStateCollectiblesEntityId")
                    .table(VaultStateCollectibles::Table)
                    .col(VaultStateCollectibles::EntityId)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .create_index(
                Index::create()
                    .name("VaultStateCollectiblesId")
                    .table(VaultStateCollectibles::Table)
                    .col(VaultStateCollectibles::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VaultState::Table).to_owned())
            .await
            .expect("Couldn't drop VaultState table");

        manager
            .drop_table(
                Table::drop()
                    .table(VaultStateCollectibles::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum VaultState {
    Table,
    EntityId,
    Shards,
}

#[derive(DeriveIden)]
enum VaultStateCollectibles {
    Table,
    EntityId,
    Id,
    Activated,
    Count,
}
