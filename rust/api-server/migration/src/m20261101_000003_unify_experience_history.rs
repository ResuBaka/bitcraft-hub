use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for sql in [
            "ALTER TABLE player_experience_history ADD COLUMN location_status text, ADD COLUMN claim_id bigint, ADD COLUMN attributed_experience bigint, ADD COLUMN legacy_claim_week_archived boolean NOT NULL DEFAULT false, ADD CONSTRAINT player_xp_location_check CHECK ((location_status IS NULL AND claim_id IS NULL) OR (location_status IN ('unknown', 'unclaimed') AND claim_id IS NULL) OR (location_status = 'claim' AND claim_id IS NOT NULL))",
            // The original event rows may already have expired. Preserve complete
            // historical weeks before dropping their continuous aggregate.
            "CREATE TABLE player_xp_legacy_claim_weeks AS SELECT bucket, player_id, skill_id, region, location_status, claim_id, experience_gained FROM player_claim_xp_weekly WHERE bucket < time_bucket(INTERVAL '1 week', now())",
            "CREATE INDEX player_xp_legacy_claim_weeks_player_idx ON player_xp_legacy_claim_weeks (player_id, bucket DESC)",
            // Legacy attributed events duplicate old trigger totals; retain them
            // for location filtering, but leave previous_experience NULL so the
            // existing total aggregates do not count them a second time.
            "INSERT INTO player_experience_history (recorded_at, player_id, skill_id, previous_experience, experience, region, location_status, claim_id, attributed_experience, legacy_claim_week_archived) SELECT recorded_at, player_id, skill_id, NULL, 0, region, location_status, claim_id, experience_gained, recorded_at < time_bucket(INTERVAL '1 week', now()) FROM player_experience_claim_history",
            // Reconciliation is a baseline, not XP earned at reconnect time.
            // Live event deltas are inserted explicitly by the XP worker.
            "CREATE OR REPLACE FUNCTION record_player_experience_history() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP = 'INSERT' OR OLD.experience IS DISTINCT FROM NEW.experience THEN INSERT INTO player_experience_history (recorded_at, player_id, skill_id, previous_experience, experience, region) VALUES (clock_timestamp(), NEW.entity_id, NEW.skill_id, NULL, NEW.experience, NEW.region); END IF; RETURN NEW; END $$",
            "CREATE MATERIALIZED VIEW player_xp_location_30m WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '30 minutes', recorded_at) AS bucket, player_id, skill_id, region, location_status, claim_id, SUM(COALESCE(attributed_experience, experience::bigint - previous_experience::bigint))::bigint AS experience_gained FROM player_experience_history WHERE location_status IS NOT NULL AND (attributed_experience IS NOT NULL OR previous_experience IS NOT NULL) GROUP BY 1, 2, 3, 4, 5, 6 WITH NO DATA",
            "CREATE MATERIALIZED VIEW player_xp_location_weekly WITH (timescaledb.continuous, timescaledb.materialized_only = false) AS SELECT time_bucket(INTERVAL '1 week', recorded_at) AS bucket, player_id, skill_id, region, location_status, claim_id, SUM(COALESCE(attributed_experience, experience::bigint - previous_experience::bigint))::bigint AS experience_gained FROM player_experience_history WHERE location_status IS NOT NULL AND NOT legacy_claim_week_archived AND (attributed_experience IS NOT NULL OR previous_experience IS NOT NULL) GROUP BY 1, 2, 3, 4, 5, 6 WITH NO DATA",
            "CREATE INDEX player_xp_location_30m_player_idx ON player_xp_location_30m (player_id, bucket DESC)",
            "CREATE INDEX player_xp_location_weekly_player_idx ON player_xp_location_weekly (player_id, bucket DESC)",
            "SELECT add_continuous_aggregate_policy('player_xp_location_30m', start_offset => INTERVAL '30 days', end_offset => INTERVAL '30 minutes', schedule_interval => INTERVAL '30 minutes')",
            "SELECT add_continuous_aggregate_policy('player_xp_location_weekly', start_offset => INTERVAL '28 days', end_offset => INTERVAL '1 hour', schedule_interval => INTERVAL '1 hour')",
            "SELECT add_retention_policy('player_xp_location_30m', INTERVAL '30 days')",
            "DROP MATERIALIZED VIEW player_claim_xp_weekly",
            "DROP MATERIALIZED VIEW player_claim_xp_30m",
            "DROP TABLE player_experience_claim_history",
        ] { db.execute_unprepared(sql).await?; }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom("Unifying XP history cannot be reversed automatically without losing event data".into()))
    }
}
