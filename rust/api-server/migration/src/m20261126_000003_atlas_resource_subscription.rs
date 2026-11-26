use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE atlas_resource_entry (
                region smallint NOT NULL,
                entity_id bigint NOT NULL,
                resource_id integer,
                x integer,
                z integer,
                PRIMARY KEY (region, entity_id)
            )",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_atlas_resource_entry_map
             ON atlas_resource_entry (region, resource_id, x, z)
             WHERE resource_id IS NOT NULL AND x IS NOT NULL AND z IS NOT NULL",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE atlas_resource_region
             ADD COLUMN subscription_synced boolean NOT NULL DEFAULT false",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // A downgraded server can keep serving the most recent complete entries.
        db.execute_unprepared(
            "INSERT INTO atlas_resource_node (region, resource_id, x, z)
             SELECT region, resource_id, x, z FROM atlas_resource_entry
             WHERE resource_id IS NOT NULL AND x IS NOT NULL AND z IS NOT NULL
             ON CONFLICT DO NOTHING",
        )
        .await?;
        db.execute_unprepared("ALTER TABLE atlas_resource_region DROP COLUMN subscription_synced")
            .await?;
        db.execute_unprepared("DROP TABLE atlas_resource_entry")
            .await?;
        Ok(())
    }
}
