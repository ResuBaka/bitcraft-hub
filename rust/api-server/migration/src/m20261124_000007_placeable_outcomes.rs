use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("
            ALTER TABLE placeable_interaction_desc ADD COLUMN next_ids integer[] NOT NULL DEFAULT '{}';
            ALTER TABLE placeable_interaction_desc ADD COLUMN reward_keys bigint[] NOT NULL DEFAULT '{}';
            CREATE TABLE placeable_growth_desc (id integer PRIMARY KEY, placeable_id integer NOT NULL, next_ids integer[] NOT NULL);
        ").await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "
            DROP TABLE placeable_growth_desc;
            ALTER TABLE placeable_interaction_desc DROP COLUMN next_ids, DROP COLUMN reward_keys;
        ",
            )
            .await?;
        Ok(())
    }
}
