use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InventoryStatsSnapshots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryStatsSnapshots::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(InventoryStatsSnapshots::Ts)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("now()")),
                    )
                    .col(
                        ColumnDef::new(InventoryStatsSnapshots::Items)
                            .json()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add composite primary key (id, ts) so TimescaleDB can create a hypertable
        let db = manager.get_connection();
        let stmt_pk = Statement::from_string(
            manager.get_database_backend(),
            "ALTER TABLE inventory_stats_snapshots ADD PRIMARY KEY (id, ts);".to_string(),
        );
        // Ignore error if already added
        let _ = db.execute(stmt_pk).await;

        // Create sequence for id if it doesn't exist
        let stmt_seq = Statement::from_string(
            manager.get_database_backend(),
            "CREATE SEQUENCE IF NOT EXISTS inventory_stats_snapshots_id_seq AS BIGINT;".to_string(),
        );
        let _ = db.execute(stmt_seq).await;

        // Set default value for id column to use the sequence
        let stmt_default = Statement::from_string(
            manager.get_database_backend(),
            "ALTER TABLE inventory_stats_snapshots ALTER COLUMN id SET DEFAULT nextval('inventory_stats_snapshots_id_seq');".to_string(),
        );
        let _ = db.execute(stmt_default).await;

        // Convert to hypertable (timescaledb)
        let stmt = Statement::from_string(
            manager.get_database_backend(),
            "SELECT create_hypertable('inventory_stats_snapshots', 'ts', migrate_data => true);".to_string(),
        );

        // Ignore error if extension not installed or hypertable already exists
        let _ = db.execute(stmt).await;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InventoryStatsSnapshots::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum InventoryStatsSnapshots {
    Table,
    Id,
    Ts,
    Items,
}
