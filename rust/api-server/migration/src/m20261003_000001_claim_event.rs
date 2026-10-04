use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClaimEvent::Table)
                    .col(
                        ColumnDef::new(ClaimEvent::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ClaimEvent::ClaimEntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimEvent::Region)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ClaimEvent::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ClaimEvent::EventType).integer().not_null())
                    .col(ColumnDef::new(ClaimEvent::ActorEntityId).big_integer())
                    .col(ColumnDef::new(ClaimEvent::Amount).big_integer())
                    .col(ColumnDef::new(ClaimEvent::TreasuryAfter).big_integer())
                    .col(ColumnDef::new(ClaimEvent::ResearchId).integer())
                    .col(ColumnDef::new(ClaimEvent::BuildingEntityId).big_integer())
                    .col(ColumnDef::new(ClaimEvent::BuildingDescriptionId).integer())
                    .col(ColumnDef::new(ClaimEvent::SubjectName).string())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("claim_event_claim_timestamp_idx")
                    .table(ClaimEvent::Table)
                    .col(ClaimEvent::ClaimEntityId)
                    .col(ClaimEvent::Region)
                    .col(ClaimEvent::Timestamp)
                    .col(ClaimEvent::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClaimEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ClaimEvent {
    Table,
    Id,
    ClaimEntityId,
    Region,
    Timestamp,
    EventType,
    ActorEntityId,
    Amount,
    TreasuryAfter,
    ResearchId,
    BuildingEntityId,
    BuildingDescriptionId,
    SubjectName,
}
