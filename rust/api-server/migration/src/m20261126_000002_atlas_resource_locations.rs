use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE atlas_resource_node (
                region smallint NOT NULL,
                resource_id integer NOT NULL,
                x integer NOT NULL,
                z integer NOT NULL,
                PRIMARY KEY (region, resource_id, x, z)
            )",
        )
        .await?;
        db.execute_unprepared(
            "CREATE TABLE atlas_resource_region (
                region smallint PRIMARY KEY,
                synced_at timestamptz NOT NULL
            )",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE atlas_resource_region")
            .await?;
        db.execute_unprepared("DROP TABLE atlas_resource_node")
            .await?;
        Ok(())
    }
}
