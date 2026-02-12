use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // LocationState Dimension index
        manager
            .create_index(
                Index::create()
                    .name("idx-location-state-dimension")
                    .table(LocationState::Table)
                    .col(LocationState::Dimension)
                    .to_owned(),
            )
            .await?;

        // BuildingState ClaimEntityId index
        manager
            .create_index(
                Index::create()
                    .name("idx-building-state-claim-entity-id")
                    .table(BuildingState::Table)
                    .col(BuildingState::ClaimEntityId)
                    .to_owned(),
            )
            .await?;

        // DeployableState ClaimEntityId index
        manager
            .create_index(
                Index::create()
                    .name("idx-deployable-state-claim-entity-id")
                    .table(DeployableState::Table)
                    .col(DeployableState::ClaimEntityId)
                    .to_owned(),
            )
            .await?;

        // Inventory OwnerEntityId index
        manager
            .create_index(
                Index::create()
                    .name("idx-inventory-owner-entity-id")
                    .table(Inventory::Table)
                    .col(Inventory::OwnerEntityId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx-location-state-dimension").table(LocationState::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-building-state-claim-entity-id").table(BuildingState::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-deployable-state-claim-entity-id").table(DeployableState::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx-inventory-owner-entity-id").table(Inventory::Table).to_owned())
            .await?;
            
        Ok(())
    }
}

#[derive(DeriveIden)]
enum LocationState {
    Table,
    Dimension,
}

#[derive(DeriveIden)]
enum BuildingState {
    Table,
    ClaimEntityId,
}

#[derive(DeriveIden)]
enum DeployableState {
    Table,
    ClaimEntityId,
}

#[derive(DeriveIden)]
enum Inventory {
    Table,
    OwnerEntityId,
}
