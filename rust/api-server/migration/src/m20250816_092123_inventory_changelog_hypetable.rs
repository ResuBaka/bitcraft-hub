use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            "ALTER TABLE inventory_changelog DROP CONSTRAINT inventory_changelog_pkey;".to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            "ALTER TABLE inventory_changelog ADD PRIMARY KEY (id, timestamp);".to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            "SELECT create_hypertable('inventory_changelog', by_range('timestamp', INTERVAL '1 day'), migrate_data => true);".to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
