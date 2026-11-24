use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("m20261124_000002_watchtower_history.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "
            DROP TRIGGER watchtower_observation ON empire_node_state;
            DROP TRIGGER watchtower_observation ON empire_node_siege_state;
            DROP FUNCTION record_watchtower_observation();
            DROP TABLE watchtower_history;
            DROP INDEX empire_watchtower_reports;
        ",
            )
            .await?;
        Ok(())
    }
}
