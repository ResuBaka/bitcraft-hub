use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for sql in [
            "CREATE MATERIALIZED VIEW player_xp_30m WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '30 minutes', recorded_at) AS bucket, player_id, skill_id, SUM(experience::bigint - previous_experience::bigint)::bigint AS experience_gained FROM player_experience_history WHERE previous_experience IS NOT NULL GROUP BY 1, 2, 3 WITH NO DATA",
            "CREATE MATERIALIZED VIEW player_xp_weekly WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '1 week', recorded_at) AS bucket, player_id, skill_id, SUM(experience::bigint - previous_experience::bigint)::bigint AS experience_gained FROM player_experience_history WHERE previous_experience IS NOT NULL GROUP BY 1, 2, 3 WITH NO DATA",
            "CREATE INDEX player_xp_30m_lookup_idx ON player_xp_30m (player_id, bucket DESC, skill_id)",
            "CREATE INDEX player_xp_weekly_lookup_idx ON player_xp_weekly (player_id, bucket DESC, skill_id)",
            "SELECT add_continuous_aggregate_policy('player_xp_30m', start_offset => INTERVAL '28 days', end_offset => INTERVAL '30 minutes', schedule_interval => INTERVAL '30 minutes')",
            "SELECT add_continuous_aggregate_policy('player_xp_weekly', start_offset => INTERVAL '28 days', end_offset => INTERVAL '1 hour', schedule_interval => INTERVAL '1 hour')",
            "SELECT add_retention_policy('player_experience_history', INTERVAL '30 days')",
            "SELECT add_retention_policy('player_xp_30m', INTERVAL '30 days')",
        ] {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("SELECT remove_retention_policy('player_experience_history', if_exists => true)").await?;
        db.execute_unprepared("DROP MATERIALIZED VIEW IF EXISTS player_xp_weekly").await?;
        db.execute_unprepared("DROP MATERIALIZED VIEW IF EXISTS player_xp_30m").await?;
        Ok(())
    }
}
