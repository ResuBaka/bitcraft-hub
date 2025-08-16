use sea_orm_migration::{prelude::*, schema::*};
use crate::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "ALTER TABLE inventory_changelog SET(timescaledb.enable_columnstore, timescaledb.orderby = 'timestamp DESC', timescaledb.segmentby = 'entity_id');"
            ),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "ALTER TABLE inventory_changelog SET (timescaledb.compress_chunk_time_interval = '24 hours');"
            ),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
