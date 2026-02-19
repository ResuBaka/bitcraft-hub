use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LocationState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LocationState::EntityId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(LocationState::ChunkIndex)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(LocationState::X).big_integer().not_null())
                    .col(ColumnDef::new(LocationState::Z).big_integer().not_null())
                    .col(
                        ColumnDef::new(LocationState::Dimension)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LocationState::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum LocationState {
    Table,
    EntityId,
    ChunkIndex,
    X,
    Z,
    Dimension,
}
