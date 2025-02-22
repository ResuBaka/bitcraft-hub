use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerToClaim::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PlayerToClaim::PlayerId).integer().not_null())
                    .col(ColumnDef::new(PlayerToClaim::ClaimId).integer().not_null())
                    .col(
                        ColumnDef::new(PlayerToClaim::InventoryPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerToClaim::BuildPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerToClaim::OfficerPermission)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerToClaim::CoOwnerPermissioN)
                            .boolean()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(PlayerToClaim::PlayerId)
                            .col(PlayerToClaim::ClaimId),
                    )
                    .to_owned(),
            )
            .await
            .expect("Creating EntityToClaim Table");

        manager
            .create_index(
                Index::create()
                    .name("player_to_claim_player_id")
                    .table(PlayerToClaim::Table)
                    .col(PlayerToClaim::PlayerId)
                    .to_owned(),
            )
            .await
            .expect("Creating player_to_claim_player_id index");

        manager
            .create_index(
                Index::create()
                    .name("player_to_claim_claim_id")
                    .table(PlayerToClaim::Table)
                    .col(PlayerToClaim::ClaimId)
                    .to_owned(),
            )
            .await
            .expect("Creating player_to_claim_claim_id index");

        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlayerToClaim::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerToClaim {
    Table,
    PlayerId,
    ClaimId,
    InventoryPermission,
    BuildPermission,
    OfficerPermission,
    CoOwnerPermissioN,
}
