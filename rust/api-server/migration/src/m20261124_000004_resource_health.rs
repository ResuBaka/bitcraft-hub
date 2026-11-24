use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE resource_desc ADD COLUMN max_health integer;
             CREATE TABLE resource_health_state (
                 entity_id bigint PRIMARY KEY, region smallint NOT NULL, health integer NOT NULL
             );
             CREATE INDEX resource_health_region_idx ON resource_health_state(region);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE resource_health_state;
             ALTER TABLE resource_desc DROP COLUMN max_health;",
            )
            .await?;
        Ok(())
    }
}
