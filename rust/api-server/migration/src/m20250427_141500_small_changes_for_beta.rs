use sea_orm_migration::sea_orm::{EntityName, Schema};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let builder = manager.get_database_backend();
        let db = manager.get_connection();
        let schema = Schema::new(builder);

        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .add_column(
                        integer(PlayerState::TravelerTasksExpiration)
                            .not_null()
                            .default(0),
                    )
                    .drop_column(PlayerState::LastShardClaim)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingState::Table)
                    .drop_column(BuildingState::Nickname)
                    .to_owned(),
            )
            .await?;

        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::StartTimestamp.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::StartTimestamp)
                        .to_owned(),
                )
                .await?;
        }

        db.execute(builder.build(&schema.create_table_from_entity(entity::claim_state::Entity)))
            .await?;
        db.execute(
            builder.build(&schema.create_table_from_entity(entity::claim_member_state::Entity)),
        )
        .await?;
        db.execute(
            builder.build(&schema.create_table_from_entity(entity::claim_local_state::Entity)),
        )
        .await?;

        manager
            .truncate_table(Table::truncate().table(ClaimTechState::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(json_binary(ClaimTechState::StartTimestamp))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .modify_column(array(ClaimTechState::Learned, ColumnType::Integer))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .rename_column(ClaimTechDesc::Supply, ClaimTechDesc::Supplies)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .modify_column(integer(ClaimTechDesc::Id).default(0))
                    .to_owned(),
            )
            .await?;

        if !manager.has_table(ClaimState::Table.to_string()).await? {
            manager
                .create_table(
                    Table::create()
                        .table(ClaimState::Table)
                        .col(big_unsigned(ClaimState::EntityId))
                        .col(big_unsigned(ClaimState::OwnerPlayerEntityId))
                        .col(big_unsigned(ClaimState::OwnerBuildingEntityId))
                        .col(string(ClaimState::Name).default(""))
                        .col(boolean(ClaimState::Neutral).default(false))
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .add_column(
                        boolean(BuildingDesc::IgnoreDamage)
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PlayerState::Table)
                    .drop_column(PlayerState::TravelerTasksExpiration)
                    .add_column(integer(PlayerState::LastShardClaim).not_null().default(0))
                    .to_owned(),
            )
            .await?;

        let mut player_state_down = Table::alter().table(PlayerState::Table).to_owned();

        if manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::TravelerTasksExpiration.to_string(),
            )
            .await?
        {
            player_state_down = player_state_down
                .drop_column(PlayerState::TravelerTasksExpiration)
                .to_owned();
        }

        if !manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::LastShardClaim.to_string(),
            )
            .await?
        {
            player_state_down = player_state_down
                .add_column(integer(PlayerState::LastShardClaim).not_null().default(0))
                .to_owned();
        }

        let player_state_down = player_state_down.to_owned();

        if !manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::LastShardClaim.to_string(),
            )
            .await?
            || manager
                .has_column(
                    PlayerState::Table.to_string(),
                    PlayerState::TravelerTasksExpiration.to_string(),
                )
                .await?
        {
            manager.alter_table(player_state_down).await?;
        }

        if manager.has_table(ClaimState::Table.to_string()).await? {
            manager
                .drop_table(Table::drop().table(ClaimState::Table).to_owned())
                .await?;
        }

        if manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::StartTimestamp.to_string(),
            )
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ClaimTechState::Table)
                        .drop_column(ClaimTechState::StartTimestamp)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .add_column(big_integer(ClaimTechState::StartTimestamp).default(0))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .rename_column(ClaimTechDesc::Supplies, ClaimTechDesc::Supply)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechDesc::Table)
                    .modify_column(big_integer(ClaimTechDesc::Id).default(0))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BuildingState::Table)
                    .add_column(string(BuildingState::Nickname).not_null().default(""))
                    .to_owned(),
            )
            .await?;

        if manager
            .has_table(entity::claim_state::Entity.table_name())
            .await?
        {
            manager
                .drop_table(Table::drop().table(entity::claim_state::Entity).to_owned())
                .await?;
        }

        if manager
            .has_table(entity::claim_member_state::Entity.table_name())
            .await?
        {
            manager
                .drop_table(
                    Table::drop()
                        .table(entity::claim_member_state::Entity)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_table(entity::claim_local_state::Entity.table_name())
            .await?
        {
            manager
                .drop_table(
                    Table::drop()
                        .table(entity::claim_local_state::Entity)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ClaimTechState::Table)
                    .modify_column(array(ClaimTechState::Learned, ColumnType::BigInteger))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BuildingDesc::Table)
                    .drop_column(BuildingDesc::IgnoreDamage)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    TravelerTasksExpiration,
    LastShardClaim,
}

#[derive(DeriveIden)]
enum BuildingDesc {
    Table,
    IgnoreDamage,
}

#[derive(DeriveIden)]
enum BuildingState {
    Table,
    Nickname,
}

#[derive(DeriveIden)]
enum ClaimTechDesc {
    Table,
    Id,
    Supplies,
    Supply,
}

#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    StartTimestamp,
    Learned,
}

#[derive(DeriveIden)]
enum ClaimState {
    Table,
    EntityId,
    OwnerPlayerEntityId,
    OwnerBuildingEntityId,
    Name,
    Neutral,
}
