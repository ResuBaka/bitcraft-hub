use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("CREATE TABLE leaderboard_snapshot_runs (snapshot_at timestamptz PRIMARY KEY, completed_at timestamptz NOT NULL)").await?;
        db.execute_unprepared("CREATE TABLE leaderboard_rank_snapshot (snapshot_at timestamptz NOT NULL, player_id bigint NOT NULL, board text NOT NULL, rank bigint NOT NULL, captured_at timestamptz NOT NULL, PRIMARY KEY (snapshot_at, player_id, board))").await?;
        db.execute_unprepared("SELECT create_hypertable('leaderboard_rank_snapshot', by_range('snapshot_at', INTERVAL '7 days'))").await?;
        db.execute_unprepared("CREATE INDEX leaderboard_rank_snapshot_player_idx ON leaderboard_rank_snapshot (player_id, snapshot_at DESC)").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS leaderboard_rank_snapshot").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS leaderboard_snapshot_runs").await?;
        Ok(())
    }
}
