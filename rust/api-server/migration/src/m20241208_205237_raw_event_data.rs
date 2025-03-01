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
                    .table(RawEventData::Table)
                    .col(
                        ColumnDef::new(RawEventData::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RawEventData::RequestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RawEventData::ReducerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RawEventData::ReducerName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RawEventData::EventData)
                            .var_binary(1_000_000_000_u32)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "SELECT create_hypertable('{}', by_range('{}'));",
                match RawEventData::Table.into_table_ref() {
                    TableRef::Table(table_ref) => table_ref.to_string(),
                    _ => panic!("Unexpected table ref type"),
                },
                RawEventData::Timestamp.to_string(),
            )
            .as_str(),
        );

        db.execute(stmt).await?;

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                    ALTER TABLE {} SET (
                      timescaledb.compress,
                      timescaledb.compress_segmentby = '{} , {}'
                    );
                "#,
                match RawEventData::Table.into_table_ref() {
                    TableRef::Table(table_ref) => table_ref.to_string(),
                    _ => panic!("Unexpected table ref type"),
                },
                RawEventData::ReducerName.to_string(),
                RawEventData::ReducerId.to_string(),
            ),
        ))
        .await?;

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                    SELECT add_compression_policy('{}', INTERVAL '1 days');
                "#,
                match RawEventData::Table.into_table_ref() {
                    TableRef::Table(table_ref) => table_ref.to_string(),
                    _ => panic!("Unexpected table ref type"),
                },
            ),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RawEventData::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RawEventData {
    Table,
    Timestamp,
    RequestId,
    ReducerId,
    ReducerName,
    EventData,
}
