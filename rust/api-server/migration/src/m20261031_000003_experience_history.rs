use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE player_experience_history (\
             recorded_at timestamptz NOT NULL, player_id bigint NOT NULL, skill_id integer NOT NULL, \
             previous_experience integer, experience integer NOT NULL, region smallint NOT NULL)",
        ).await?;
        db.execute_unprepared(
            "SELECT create_hypertable('player_experience_history', by_range('recorded_at', INTERVAL '1 day'))",
        ).await?;
        db.execute_unprepared(
            "CREATE INDEX player_experience_history_player_time_idx ON player_experience_history (player_id, recorded_at DESC, skill_id)",
        ).await?;
        db.execute_unprepared(
            "CREATE FUNCTION record_player_experience_history() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN \
               IF TG_OP = 'INSERT' THEN \
                 INSERT INTO player_experience_history VALUES (clock_timestamp(), NEW.entity_id, NEW.skill_id, NULL, NEW.experience, NEW.region); \
               ELSIF OLD.experience IS DISTINCT FROM NEW.experience THEN \
                 INSERT INTO player_experience_history VALUES (clock_timestamp(), NEW.entity_id, NEW.skill_id, OLD.experience, NEW.experience, NEW.region); \
               END IF; \
               RETURN NEW; \
             END $$",
        ).await?;
        db.execute_unprepared(
            "CREATE TRIGGER experience_history_record AFTER INSERT OR UPDATE ON experience_state FOR EACH ROW EXECUTE FUNCTION record_player_experience_history()",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TRIGGER IF EXISTS experience_history_record ON experience_state").await?;
        db.execute_unprepared("DROP FUNCTION IF EXISTS record_player_experience_history()").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS player_experience_history").await?;
        Ok(())
    }
}
