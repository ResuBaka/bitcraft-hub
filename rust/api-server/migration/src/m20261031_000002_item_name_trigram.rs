use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for sql in [
            "CREATE INDEX IF NOT EXISTS item_desc_name_trgm_idx ON item_desc USING GIN (name gin_trgm_ops)",
            "CREATE INDEX IF NOT EXISTS cargo_desc_name_trgm_idx ON cargo_desc USING GIN (name gin_trgm_ops)",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for sql in [
            "DROP INDEX IF EXISTS item_desc_name_trgm_idx",
            "DROP INDEX IF EXISTS cargo_desc_name_trgm_idx",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }
        Ok(())
    }
}
