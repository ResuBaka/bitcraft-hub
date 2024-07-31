use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .create_table(
                Table::create()
                    .table(VehicleState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VehicleState::EntityId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(VehicleState::OwnerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(VehicleState::Direction).integer().not_null())
                    .col(
                        ColumnDef::new(VehicleState::VehicleDescriptionId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(VehicleState::Nickname).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("vehicle_state_owner_id_idx")
                    .table(VehicleState::Table)
                    .col(VehicleState::OwnerId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(Table::drop().table(VehicleState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum VehicleState {
    Table,
    EntityId,
    OwnerId,
    Direction,
    VehicleDescriptionId,
    Nickname,
}
