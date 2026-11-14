use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for sql in [
            "CREATE TABLE player_experience_claim_history (recorded_at timestamptz NOT NULL, player_id bigint NOT NULL, skill_id integer NOT NULL, experience_gained bigint NOT NULL, region smallint NOT NULL, location_status text NOT NULL CHECK (location_status IN ('claim', 'unclaimed', 'unknown')), claim_id bigint, CHECK ((location_status = 'claim') = (claim_id IS NOT NULL)))",
            "SELECT create_hypertable('player_experience_claim_history', by_range('recorded_at', INTERVAL '1 day'))",
            "CREATE INDEX player_experience_claim_history_player_idx ON player_experience_claim_history (player_id, recorded_at DESC, skill_id)",
            "CREATE MATERIALIZED VIEW player_claim_xp_30m WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '30 minutes', recorded_at) AS bucket, player_id, skill_id, region, location_status, claim_id, SUM(experience_gained)::bigint AS experience_gained FROM player_experience_claim_history GROUP BY 1, 2, 3, 4, 5, 6 WITH NO DATA",
            "CREATE MATERIALIZED VIEW player_claim_xp_weekly WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '1 week', recorded_at) AS bucket, player_id, skill_id, region, location_status, claim_id, SUM(experience_gained)::bigint AS experience_gained FROM player_experience_claim_history GROUP BY 1, 2, 3, 4, 5, 6 WITH NO DATA",
            "CREATE INDEX player_claim_xp_30m_player_idx ON player_claim_xp_30m (player_id, bucket DESC)",
            "CREATE INDEX player_claim_xp_weekly_player_idx ON player_claim_xp_weekly (player_id, bucket DESC)",
            "SELECT add_continuous_aggregate_policy('player_claim_xp_30m', start_offset => INTERVAL '28 days', end_offset => INTERVAL '30 minutes', schedule_interval => INTERVAL '30 minutes')",
            "SELECT add_continuous_aggregate_policy('player_claim_xp_weekly', start_offset => INTERVAL '28 days', end_offset => INTERVAL '1 hour', schedule_interval => INTERVAL '1 hour')",
            "SELECT add_retention_policy('player_experience_claim_history', INTERVAL '30 days')",
            "SELECT add_retention_policy('player_claim_xp_30m', INTERVAL '30 days')",
        ] { db.execute_unprepared(sql).await?; }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP MATERIALIZED VIEW IF EXISTS player_claim_xp_weekly").await?;
        db.execute_unprepared("DROP MATERIALIZED VIEW IF EXISTS player_claim_xp_30m").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS player_experience_claim_history").await?;
        Ok(())
    }
}
