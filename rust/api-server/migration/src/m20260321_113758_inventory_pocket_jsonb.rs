use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // We use raw SQL because altering column types with casting
        // is very specific to the database engine (PostgreSQL).
        let db = manager.get_connection();

        db.execute_unprepared(
            "ALTER TABLE inventory
             ALTER COLUMN pockets TYPE JSONB
             USING pockets::jsonb",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // To go back, we cast it from binary back to plain text JSON
        db.execute_unprepared(
            "ALTER TABLE inventory
             ALTER COLUMN pockets TYPE JSON
             USING pockets::json",
        )
        .await?;

        Ok(())
    }
}
