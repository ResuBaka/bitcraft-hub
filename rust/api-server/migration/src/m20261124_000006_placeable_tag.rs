use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Existing descriptor rows are populated by the next game snapshot.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE placeable_desc ADD COLUMN tag text NOT NULL DEFAULT ''",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE placeable_desc DROP COLUMN tag")
            .await?;
        Ok(())
    }
}
