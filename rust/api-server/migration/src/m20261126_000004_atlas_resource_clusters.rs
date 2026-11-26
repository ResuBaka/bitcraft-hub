use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE atlas_resource_cluster (
                region smallint NOT NULL,
                level integer NOT NULL,
                resource_id integer NOT NULL,
                bucket_x integer NOT NULL,
                bucket_z integer NOT NULL,
                node_count integer NOT NULL,
                PRIMARY KEY (region, level, resource_id, bucket_x, bucket_z)
            )",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX idx_atlas_resource_cluster_empty
             ON atlas_resource_cluster (region) WHERE node_count = 0",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE atlas_resource_region ADD COLUMN clusters_synced boolean NOT NULL DEFAULT false",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE atlas_resource_region DROP COLUMN clusters_synced")
            .await?;
        db.execute_unprepared("DROP TABLE atlas_resource_cluster")
            .await?;
        Ok(())
    }
}
