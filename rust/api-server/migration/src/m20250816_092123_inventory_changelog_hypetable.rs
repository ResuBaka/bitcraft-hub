use crate::sea_orm::Statement;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!("ALTER TABLE inventory_changelog DROP CONSTRAINT inventory_changelog_pkey;"),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!("ALTER TABLE inventory_changelog ADD PRIMARY KEY (id, timestamp);"),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "SELECT create_hypertable('inventory_changelog', by_range('timestamp', INTERVAL '1 day'), migrate_data => true);"
            ),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
