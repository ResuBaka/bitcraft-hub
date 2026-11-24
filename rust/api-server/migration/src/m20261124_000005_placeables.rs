use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared("
            CREATE TABLE placeable_state (entity_id bigint PRIMARY KEY, region smallint NOT NULL,
                owner_entity_id bigint NOT NULL, placeable_id integer NOT NULL);
            CREATE INDEX placeable_owner_idx ON placeable_state(owner_entity_id);
            CREATE INDEX placeable_region_idx ON placeable_state(region);
            CREATE TABLE placeable_growth_timer (entity_id bigint PRIMARY KEY, region smallint NOT NULL, ends_at_ms bigint);
            CREATE INDEX placeable_timer_region_idx ON placeable_growth_timer(region);
            CREATE TABLE placeable_desc (id integer PRIMARY KEY, name text NOT NULL, tier integer NOT NULL);
            CREATE TABLE placeable_interaction_desc (id integer PRIMARY KEY, placeable_id integer NOT NULL, verb_phrase text NOT NULL);
            CREATE INDEX placeable_interaction_type_idx ON placeable_interaction_desc(placeable_id);
        ").await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "DROP TABLE placeable_interaction_desc, placeable_desc, placeable_growth_timer, placeable_state"
        ).await?;
        Ok(())
    }
}
