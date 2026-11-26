use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE atlas_resource_cluster
             ADD COLUMN sum_x2 bigint NOT NULL DEFAULT 0,
             ADD COLUMN sum_z bigint NOT NULL DEFAULT 0",
        )
        .await?;
        // Existing counts lack positions; the next regional snapshot rebuilds
        // them, even when the individual resource rows haven't changed.
        db.execute_unprepared("UPDATE atlas_resource_region SET clusters_synced = false")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE atlas_resource_cluster DROP COLUMN sum_x2, DROP COLUMN sum_z",
            )
            .await?;
        Ok(())
    }
}
