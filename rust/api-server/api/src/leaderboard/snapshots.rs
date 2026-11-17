use super::{Leaderboard, RankingSystem};
use crate::AppState;
use chrono::{DateTime, TimeDelta, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
use std::sync::Arc;

const CAPTURE_WINDOW_SECONDS: i64 = 600;
const LOCK_ID: i64 = 0x42695472616e6b;

fn capture_target(now: DateTime<Utc>) -> DateTime<Utc> {
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    if (now - midnight).num_seconds() < CAPTURE_WINDOW_SECONDS {
        midnight
    } else {
        midnight + TimeDelta::days(1)
    }
}

pub(crate) fn start(state: AppState) {
    let shutdown = state.shutdown.clone();
    shutdown.spawn_worker(async move {
        loop {
            let now = Utc::now();
            let target = capture_target(now);
            if target > now {
                let delay = (target - now).to_std().unwrap_or_default();
                tokio::select! {
                    _ = state.shutdown.stop.cancelled() => break,
                    _ = tokio::time::sleep(delay) => {}
                }
            }
            loop {
                if state.shutdown.stop.is_cancelled() || (Utc::now() - target).num_seconds() >= CAPTURE_WINDOW_SECONDS {
                    break;
                }
                match capture(&state, target).await {
                    Ok(true) => break,
                    Ok(false) => tracing::info!(%target, "Another server is capturing leaderboard ranks"),
                    Err(error) => tracing::error!(%target, %error, "Leaderboard snapshot failed; retrying"),
                }
                tokio::select! {
                    _ = state.shutdown.stop.cancelled() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
                }
            }
            // Avoid attempting the same completed midnight again.
            let next = target + TimeDelta::days(1);
            let delay = (next - Utc::now()).to_std().unwrap_or_default();
            tokio::select! {
                _ = state.shutdown.stop.cancelled() => break,
                _ = tokio::time::sleep(delay) => {}
            }
        }
    });
}

async fn capture(state: &AppState, snapshot_at: DateTime<Utc>) -> anyhow::Result<bool> {
    if state.ranking_system.global_leaderboard.get_range(0, 1).is_empty() {
        anyhow::bail!("leaderboards have not published any players");
    }
    let txn = state.conn.begin().await?;
    let lock = txn.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres, "SELECT pg_try_advisory_xact_lock($1) AS acquired", [LOCK_ID.into()],
    )).await?.ok_or_else(|| anyhow::anyhow!("missing advisory lock result"))?;
    if !lock.try_get::<bool>("", "acquired")? {
        return Ok(false);
    }
    let existing = txn.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres, "SELECT 1 FROM leaderboard_snapshot_runs WHERE snapshot_at = $1", [snapshot_at.into()],
    )).await?;
    if existing.is_some() {
        return Ok(true);
    }

    let ranking = Arc::clone(&state.ranking_system);
    let mut boards: Vec<(String, Option<Arc<Leaderboard>>)> = vec![
        ("global_experience".into(), None), ("profession_experience".into(), None),
        ("adventure_experience".into(), None), ("level".into(), None),
        ("xp_per_hour".into(), None), ("time_played".into(), None), ("time_signed_in".into(), None),
    ];
    boards.extend(ranking.skill_boards().into_iter().map(|(id, board)| (format!("skill_{id}"), Some(board))));
    for (name, skill_board) in boards {
        let ranking = Arc::clone(&ranking);
        let board_name = name.clone();
        let (captured_at, entries) = tokio::task::spawn_blocking(move || {
            let board: &Leaderboard = match skill_board.as_deref() {
                Some(board) => board,
                None => aggregate_board(&ranking, &board_name).expect("known aggregate board"),
            };
            (Utc::now(), board.get_range(0, usize::MAX))
        }).await?;
        for chunk in entries.chunks(1000) {
            let rows = chunk.iter().map(|entry| format!("($1,{},$2,{},$3)", entry.user_id, entry.rank)).collect::<Vec<_>>().join(",");
            let sql = format!("INSERT INTO leaderboard_rank_snapshot (snapshot_at, player_id, board, rank, captured_at) VALUES {rows}");
            txn.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres, sql,
                [snapshot_at.into(), name.clone().into(), captured_at.into()])).await?;
        }
    }
    txn.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO leaderboard_snapshot_runs (snapshot_at, completed_at) VALUES ($1, now())", [snapshot_at.into()])).await?;
    txn.commit().await?;
    tracing::info!(%snapshot_at, "Leaderboard snapshot completed");
    Ok(true)
}

pub(super) fn aggregate_board<'a>(ranking: &'a RankingSystem, name: &str) -> Option<&'a Leaderboard> {
    Some(match name {
        "global_experience" => &ranking.global_leaderboard,
        "profession_experience" => &ranking.profession_leaderboard,
        "adventure_experience" => &ranking.adventure_leaderboard,
        "level" => &ranking.level_leaderboard,
        "xp_per_hour" => &ranking.xp_per_hour,
        "time_played" => &ranking.time_played,
        "time_signed_in" => &ranking.time_signed_in,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn only_recovers_snapshot_within_ten_minutes_of_midnight_utc() {
        let midnight = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
        assert_eq!(capture_target(midnight), midnight);
        assert_eq!(capture_target(midnight + TimeDelta::seconds(599)), midnight);
        assert_eq!(capture_target(midnight + TimeDelta::seconds(600)), midnight + TimeDelta::days(1));
        assert_eq!(capture_target(midnight - TimeDelta::seconds(1)), midnight);
    }
}
