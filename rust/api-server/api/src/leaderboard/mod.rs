pub(crate) mod bitcraft;
pub(crate) mod snapshots;

use crate::{AppRouter, AppState, leaderboard};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use dashmap::DashMap;
use log::error;
use serde::{Deserialize, Serialize};
use service::Query;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::LazyLock;
use ts_rs::TS;

#[macro_export]
macro_rules! generate_mysql_sum_level_sql_statement {
    ($experience_per_level:expr) => {{
        let mut sql = String::new();
        sql.push_str("SUM(CASE ");
        for (level, xp) in $experience_per_level.iter().rev() {
            sql.push_str(format!("WHEN experience >= {xp} THEN {level} ").as_str());
        }
        sql.push_str("ELSE 0 END)");
        sql
    }};
}

pub(crate) static EXCLUDED_USERS_FROM_LEADERBOARD: LazyLock<Vec<i64>> =
    LazyLock::new(|| vec![576460752390479914]);
pub(crate) static EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY: [i64; 2] = [0, 0];

pub(crate) const EXPERIENCE_PER_LEVEL: [(i32, i64); 110] = [
    (1, 0),
    (2, 520),
    (3, 1100),
    (4, 1740),
    (5, 2460),
    (6, 3270),
    (7, 4170),
    (8, 5170),
    (9, 6290),
    (10, 7540),
    (11, 8930),
    (12, 10490),
    (13, 12220),
    (14, 14160),
    (15, 16320),
    (16, 18730),
    (17, 21420),
    (18, 24410),
    (19, 27760),
    (20, 31490),
    (21, 35660),
    (22, 40310),
    (23, 45490),
    (24, 51280),
    (25, 57740),
    (26, 64940),
    (27, 72980),
    (28, 81940),
    (29, 91950),
    (30, 103110),
    (31, 115560),
    (32, 129460),
    (33, 144960),
    (34, 162260),
    (35, 181560),
    (36, 203100),
    (37, 227130),
    (38, 253930),
    (39, 283840),
    (40, 317220),
    (41, 354450),
    (42, 396000),
    (43, 442350),
    (44, 494070),
    (45, 551770),
    (46, 616150),
    (47, 687980),
    (48, 768130),
    (49, 857560),
    (50, 957330),
    (51, 1068650),
    (52, 1192860),
    (53, 1331440),
    (54, 1486060),
    (55, 1658570),
    (56, 1851060),
    (57, 2065820),
    (58, 2305430),
    (59, 2572780),
    (60, 2871080),
    (61, 3203890),
    (62, 3575230),
    (63, 3989550),
    (64, 4451810),
    (65, 4967590),
    (66, 5543050),
    (67, 6185120),
    (68, 6901500),
    (69, 7700800),
    (70, 8592610),
    (71, 9587630),
    (72, 10697810),
    (73, 11936490),
    (74, 13318540),
    (75, 14860540),
    (76, 16581010),
    (77, 18500600),
    (78, 20642370),
    (79, 23032020),
    (80, 25698250),
    (81, 28673070),
    (82, 31992200),
    (83, 35695470),
    (84, 39827360),
    (85, 44437480),
    (86, 49581160),
    (87, 55320170),
    (88, 61723410),
    (89, 68867770),
    (90, 76839000),
    (91, 85732810),
    (92, 95656000),
    (93, 106727680),
    (94, 119080790),
    (95, 132863630),
    (96, 148241700),
    (97, 165399620),
    (98, 184543380),
    (99, 205902840),
    (100, 229734400),
    (101, 256324240),
    (102, 285991580),
    (103, 319092580),
    (104, 356024680),
    (105, 397231240),
    (106, 443207040),
    (107, 494504080),
    (108, 551738200),
    (109, 615596560),
    (110, 686845760),
];

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/leaderboard",
            axum_codec::routing::get(leaderboard::get_top_100).into(),
        )
        .route(
            "/leaderboard/rank-snapshots",
            axum_codec::routing::get(leaderboard_rank_snapshots).into(),
        )
        .route(
            "/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/experience/{player_id}/history",
            axum_codec::routing::get(experience_history).into(),
        )
        .route(
            "/api/bitcraft/claims/{claim_id}/experience/history",
            axum_codec::routing::get(claim_experience_history).into(),
        )
        .route(
            "/api/bitcraft/experience/{player_id}/rank-snapshot",
            axum_codec::routing::get(player_rank_snapshot).into(),
        )
        .route(
            "/api/bitcraft/experience/history/skills",
            axum_codec::routing::get(experience_history_skills).into(),
        )
        .route(
            "/api/bitcraft/leaderboard/claims/{claim_id}",
            axum_codec::routing::get(get_claim_leaderboard).into(),
        )
}

#[derive(sea_orm::FromQueryResult)]
struct StoredRankSnapshot {
    board: String,
    rank: i64,
    captured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct PlayerRankSnapshot {
    board: String,
    label: String,
    previous_rank: i64,
    current_rank: Option<u64>,
    captured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct LeaderboardSnapshotParams {
    board: String,
    players: String,
}

#[derive(Serialize, sea_orm::FromQueryResult)]
struct LeaderboardSnapshotRow {
    player_id: i64,
    rank: i64,
    captured_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct LeaderboardSnapshots {
    board: String,
    entries: Vec<LeaderboardSnapshotRow>,
}

async fn leaderboard_rank_snapshots(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<LeaderboardSnapshotParams>,
) -> Result<axum_codec::Codec<LeaderboardSnapshots>, (StatusCode, &'static str)> {
    use sea_orm::FromQueryResult;
    let board = match params.board.as_str() {
        "Experience" => "global_experience".to_string(),
        "Professions" => "profession_experience".to_string(),
        "Skills" => "adventure_experience".to_string(),
        "Level" => "level".to_string(),
        "Experience Per Hour" => "xp_per_hour".to_string(),
        "Time Played" => "time_played".to_string(),
        "Time Online" => "time_signed_in".to_string(),
        name => match state.skill_desc.iter().find(|skill| skill.name == name) {
            Some(skill) => format!("skill_{}", skill.id),
            None => return Ok(axum_codec::Codec(LeaderboardSnapshots { board: params.board, entries: Vec::new() })),
        },
    };
    if params.players.is_empty() {
        return Ok(axum_codec::Codec(LeaderboardSnapshots { board: params.board, entries: Vec::new() }));
    }
    let ids: Vec<i64> = params.players.split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid player IDs"))?;
    if ids.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Too many player IDs"));
    }
    let ids = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let rows = LeaderboardSnapshotRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Postgres,
        "SELECT player_id, rank, captured_at FROM leaderboard_rank_snapshot WHERE snapshot_at = date_trunc('day', now(), 'UTC') AND board = $1 AND player_id = ANY(string_to_array($2, ',')::bigint[])",
        [board.into(), ids.into()],
    )).all(&state.conn).await.map_err(|error| {
        tracing::error!(%error, "Failed to read leaderboard snapshots");
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
    })?;
    Ok(axum_codec::Codec(LeaderboardSnapshots { board: params.board, entries: rows }))
}

async fn player_rank_snapshot(
    State(state): State<AppState>,
    Path(player_id): Path<i64>,
) -> Result<axum_codec::Codec<Vec<PlayerRankSnapshot>>, (StatusCode, &'static str)> {
    use sea_orm::FromQueryResult;
    let rows = StoredRankSnapshot::find_by_statement(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Postgres,
        "SELECT board, rank, captured_at FROM leaderboard_rank_snapshot WHERE player_id = $1 AND snapshot_at = date_trunc('day', now(), 'UTC')",
        [player_id.into()],
    )).all(&state.conn).await.map_err(|error| {
        tracing::error!(%error, "Failed to read leaderboard snapshot");
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
    })?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let (label, current_rank) = if let Some(id) = row.board.strip_prefix("skill_").and_then(|id| id.parse::<i64>().ok()) {
            let name = state.skill_desc.get(&id).map(|skill| skill.name.clone()).unwrap_or_else(|| format!("Skill {id}"));
            (name, state.ranking_system.skill_leaderboard(id).and_then(|board| board.get_rank(player_id)))
        } else {
            let label = match row.board.as_str() {
                "global_experience" => "Experience",
                "profession_experience" => "Professions",
                "adventure_experience" => "Skills",
                "level" => "Level",
                "xp_per_hour" => "Experience Per Hour",
                "time_played" => "Time Played",
                "time_signed_in" => "Time Online",
                _ => continue,
            };
            (label.to_string(), snapshots::aggregate_board(&state.ranking_system, &row.board).and_then(|board| board.get_rank(player_id)))
        };
        result.push(PlayerRankSnapshot {
            board: row.board, label, previous_rank: row.rank,
            current_rank: current_rank.map(|rank| rank as u64), captured_at: row.captured_at,
        });
    }
    Ok(axum_codec::Codec(result))
}

#[derive(Serialize)]
struct ExperienceHistorySkill {
    id: i64,
    name: String,
}

async fn experience_history_skills(
    State(state): State<AppState>,
) -> axum_codec::Codec<Vec<ExperienceHistorySkill>> {
    let mut skills: Vec<_> = state.skill_desc.iter().filter(|skill| skill.skill_category != 0)
        .map(|skill| ExperienceHistorySkill { id: skill.id, name: skill.name.clone() }).collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    axum_codec::Codec(skills)
}

#[derive(Deserialize)]
struct ExperienceHistoryParams {
    skill_id: Option<i32>,
    group_by_skill: Option<bool>,
    days: Option<u32>,
    scale: Option<ExperienceHistoryScale>,
    location: Option<ExperienceLocationFilter>,
    claim_id: Option<i64>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ExperienceLocationFilter { Claim, Unclaimed, Unknown }

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum ExperienceHistoryScale {
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Serialize, sea_orm::FromQueryResult)]
struct ExperienceHistoryPoint {
    day: chrono::DateTime<chrono::Utc>,
    experience_gained: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_id: Option<i32>,
}

async fn experience_history(
    State(state): State<AppState>,
    Path(player_id): Path<i64>,
    axum::extract::Query(params): axum::extract::Query<ExperienceHistoryParams>,
) -> Result<axum_codec::Codec<Vec<ExperienceHistoryPoint>>, (StatusCode, &'static str)> {
    query_experience_history(state, player_id, params, false).await
}

async fn claim_experience_history(
    State(state): State<AppState>,
    Path(claim_id): Path<i64>,
    axum::extract::Query(params): axum::extract::Query<ExperienceHistoryParams>,
) -> Result<axum_codec::Codec<Vec<ExperienceHistoryPoint>>, (StatusCode, &'static str)> {
    query_experience_history(state, claim_id, params, true).await
}

async fn query_experience_history(
    state: AppState, id: i64, params: ExperienceHistoryParams, claim_scope: bool,
) -> Result<axum_codec::Codec<Vec<ExperienceHistoryPoint>>, (StatusCode, &'static str)> {
    use sea_orm::FromQueryResult;
    let scale = params.scale.unwrap_or(ExperienceHistoryScale::Day);
    let location = match (params.location, params.claim_id) {
        (Some(ExperienceLocationFilter::Unclaimed | ExperienceLocationFilter::Unknown), Some(_)) => {
            return Err((StatusCode::BAD_REQUEST, "claim_id requires claim location"));
        }
        (Some(value), _) => Some(match value { ExperienceLocationFilter::Claim => "claim", ExperienceLocationFilter::Unclaimed => "unclaimed", ExperienceLocationFilter::Unknown => "unknown" }),
        (None, Some(_)) => Some("claim"),
        (None, None) => None,
    };
    let filtered = claim_scope || location.is_some();
    let (interval, table, time_column, xp_expression, lookback, default_days, max_days) = match (scale, filtered) {
        (ExperienceHistoryScale::Hour, false) => ("5 minutes", "player_experience_history", "recorded_at", "experience::bigint - previous_experience::bigint", "hours", 1, 1),
        (ExperienceHistoryScale::Day, false) => ("30 minutes", "player_xp_30m", "bucket", "experience_gained", "days", 1, 30),
        (ExperienceHistoryScale::Week, false) => ("3 hours", "player_xp_30m", "bucket", "experience_gained", "days", 7, 30),
        (ExperienceHistoryScale::Month, false) => ("1 week", "player_xp_weekly", "bucket", "experience_gained", "days", 180, 3650),
        (ExperienceHistoryScale::Hour, true) => ("5 minutes", "player_experience_history", "recorded_at", "COALESCE(attributed_experience, experience::bigint - previous_experience::bigint)", "hours", 1, 1),
        (ExperienceHistoryScale::Day, true) => ("30 minutes", "player_xp_location_30m", "bucket", "experience_gained", "days", 1, 30),
        (ExperienceHistoryScale::Week, true) => ("3 hours", "player_xp_location_30m", "bucket", "experience_gained", "days", 7, 30),
        (ExperienceHistoryScale::Month, true) => ("1 week", "(SELECT bucket, player_id, skill_id, location_status, claim_id, experience_gained FROM player_xp_location_weekly UNION ALL SELECT bucket, player_id, skill_id, location_status, claim_id, experience_gained FROM player_xp_legacy_claim_weeks) AS location_history", "bucket", "experience_gained", "days", 180, 3650),
    };
    let days = params.days.unwrap_or(default_days).clamp(1, max_days);
    let (skill_select, grouping) = if params.group_by_skill.unwrap_or(false) {
        ("skill_id", "day, skill_id")
    } else {
        ("NULL::integer AS skill_id", "day")
    };
    let location_filter = if claim_scope { "AND location_status = 'claim'" } else if filtered { "AND location_status = $4 AND ($5::bigint IS NULL OR claim_id = $5)" } else { "" };
    let id_column = if claim_scope { "claim_id" } else { "player_id" };
    let sql = format!(
        "SELECT time_bucket(INTERVAL '{interval}', {time_column}) AS day, \
         SUM({xp_expression})::bigint AS experience_gained, {skill_select} FROM {table} \
         WHERE {id_column} = $1 AND {time_column} >= time_bucket(INTERVAL '{interval}', now() - make_interval({lookback} => $2)) \
         AND ({xp_expression}) IS NOT NULL AND ($3::integer IS NULL OR skill_id = $3) {location_filter} GROUP BY {grouping} ORDER BY day"
    );
    let mut values = vec![id.into(), (days as i32).into(), params.skill_id.into()];
    if !claim_scope && let Some(location) = location {
        values.extend([location.into(), params.claim_id.into()]);
    }
    let rows = ExperienceHistoryPoint::find_by_statement(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Postgres,
        sql,
        values,
    ))
    .all(&state.conn)
    .await
    .map_err(|error| {
        tracing::error!(%error, "Failed to query experience history");
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
    })?;
    Ok(axum_codec::Codec(rows))
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(untagged)]
pub(crate) enum RankType {
    Experience(LeaderboardExperience),
    ExperiencePerHour(LeaderboardExperiencePerHour),
    Level(LeaderboardLevel),
    Skill(LeaderboardSkill),
    Time(LeaderboardTime),
}

pub struct LeaderboardEntry {
    #[allow(dead_code)]
    pub rank: usize,
    pub user_id: i64,
    pub xp: i64,
}

pub(crate) struct BucketDistribution {
    pub(crate) bucket: i64,
    pub(crate) min_xp: i64,
    pub(crate) max_xp: i64,
    pub(crate) count: usize,
}

mod compact;
pub(super) use compact::Leaderboard;
use compact::{PlayerRegistry, RankingWorker, ScoreWidth};

#[derive(Clone, Copy, Default)]
pub(super) enum BucketLayout {
    #[default]
    Tiered,
    Fixed(std::num::NonZeroU32),
}

impl BucketLayout {
    const BUCKET_RANGES: [(i64, i64); 5] = [
        (50_000, 1_000),
        (250_000, 5_000),
        (1_000_000, 25_000),
        (5_000_000, 100_000),
        (i64::MAX, 500_000),
    ];

    fn bucket_for(self, xp: i64) -> i64 {
        let normalized = xp.max(0);
        if let Self::Fixed(width) = self {
            return normalized / i64::from(width.get());
        }
        let mut range_start = 0i64;
        let mut offset = 0i64;

        for (range_max, bucket_size) in Self::BUCKET_RANGES.iter() {
            if *range_max == i64::MAX {
                let idx_in_range = (normalized - range_start) / *bucket_size;
                return offset + idx_in_range;
            }

            if normalized <= *range_max {
                let idx_in_range = (normalized - range_start) / *bucket_size;
                return offset + idx_in_range;
            }

            let range_len = range_max - range_start + 1;
            let buckets_in_range = (range_len + bucket_size - 1) / bucket_size;
            offset += buckets_in_range;
            range_start = range_max + 1;
        }

        offset
    }

    fn bucket_bounds(self, bucket: i64) -> (i64, i64) {
        if let Self::Fixed(width) = self {
            let width = i64::from(width.get());
            let min = bucket.saturating_mul(width);
            return (min, min.saturating_add(width - 1));
        }
        let mut range_start = 0i64;
        let mut offset = 0i64;

        for (range_max, bucket_size) in Self::BUCKET_RANGES.iter() {
            if *range_max == i64::MAX {
                let idx_in_range = bucket - offset;
                let min = range_start + (idx_in_range * *bucket_size);
                let max = min.saturating_add(*bucket_size - 1);
                return (min, max);
            }

            let range_len = range_max - range_start + 1;
            let buckets_in_range = (range_len + bucket_size - 1) / bucket_size;

            if bucket < offset + buckets_in_range {
                let idx_in_range = bucket - offset;
                let min = range_start + (idx_in_range * *bucket_size);
                let max = min + *bucket_size - 1;
                return (min, max);
            }

            offset += buckets_in_range;
            range_start = range_max + 1;
        }

        (0, 0)
    }
}

#[cfg(test)]
mod bookkeeping_tests {
    use super::compact::LeaderboardOp;
    use super::*;
    use crossbeam_channel::Receiver;

    #[test]
    fn bucket_layout_defaults_overrides_and_extreme_scores() {
        let config = crate::config::LeaderboardConfig::default();
        let level = RankingSystem::bucket_layout(&config, "level");
        for (score, bounds) in [
            (0, (0, 49)),
            (49, (0, 49)),
            (50, (50, 99)),
            (1_294, (1_250, 1_299)),
            (1_800, (1_800, 1_849)),
            (2_470, (2_450, 2_499)),
        ] {
            assert_eq!(level.bucket_bounds(level.bucket_for(score)), bounds);
        }
        let tiered = RankingSystem::bucket_layout(&config, "global_experience");
        assert_eq!(tiered.bucket_bounds(tiered.bucket_for(150)), (0, 999));
        let config: crate::config::LeaderboardConfig =
            serde_json::from_str(r#"{"bucket_widths":{"level":5,"skill_1":100}}"#).unwrap();
        let level = RankingSystem::bucket_layout(&config, "level");
        assert_eq!(level.bucket_bounds(level.bucket_for(149)), (145, 149));
        let skill = RankingSystem::bucket_layout(&config, "skill_1");
        assert_eq!(skill.bucket_bounds(skill.bucket_for(150)), (100, 199));
        for score in [0, i64::MAX] {
            let (min, max) = skill.bucket_bounds(skill.bucket_for(score));
            assert!(min <= score && score <= max);
        }
        assert_eq!(skill.bucket_for(-1), 0);
    }

    // Drain changes synchronously to make coalescing deterministic in tests.
    fn board() -> (Leaderboard, Receiver<()>) {
        Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide)
    }

    fn apply_pending(board: &Leaderboard, rx: &Receiver<()>) {
        while rx.try_recv().is_ok() {}
        let batch = std::mem::take(&mut board.state.lock().pending);
        Leaderboard::apply_changes(&board.data, &board.players, batch);
    }

    fn pending_op(board: &Leaderboard, user_id: i64) -> Option<LeaderboardOp> {
        let index = board.players.get(user_id)?;
        board.state.lock().pending.get(&index).copied()
    }

    fn assert_consistent(board: &Leaderboard) {
        let expected = board
            .score_entries()
            .into_iter()
            .map(|(id, xp)| ((xp, id), ()))
            .collect::<BTreeMap<_, _>>();
        let mut bucket_counts = BTreeMap::new();
        for &(xp, _) in expected.keys() {
            *bucket_counts
                .entry(BucketLayout::default().bucket_for(xp))
                .or_insert(0) += 1;
        }
        {
            let data = board.data.read();
            assert_eq!(data.bucket_counts, bucket_counts);
        }
        let entries = board.get_range(0, expected.len() + 1);
        assert_eq!(entries.len(), expected.len());
        assert_eq!(
            entries
                .iter()
                .map(|entry| ((entry.xp, entry.user_id), ()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.rank, index + 1);
            assert_eq!(board.get_rank(entry.user_id), Some(index + 1));
            assert_eq!(board.get_value(&entry.user_id), Some(entry.xp));
        }
    }

    #[test]
    fn first_zero_is_indexed_but_repeated_zero_updates_are_not_queued() {
        let (board, rx) = board();
        for _ in 0..100 {
            board.update(1, 0);
        }
        assert_eq!(rx.len(), 1);
        assert!(matches!(
            pending_op(&board, 1),
            Some(LeaderboardOp::Insert { new_xp: 0 })
        ));
        apply_pending(&board, &rx);
        board.update(1, 0);
        assert!(rx.is_empty());
        assert_eq!(board.get_rank(1), Some(1));
        assert_consistent(&board);
    }

    #[test]
    fn zero_to_positive_removes_the_zero_index_and_counts() {
        let (board, rx) = board();
        board.update(1, 0);
        apply_pending(&board, &rx);
        board.update(1, 100_000);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        let entries = board.get_range(0, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].xp, 100_000);
    }

    #[test]
    fn positive_zero_and_positive_transitions_keep_exact_counts_and_tie_order() {
        let (board, rx) = board();
        board.update(1, 200_000);
        board.update(2, 0);
        board.update(3, 0);
        board.update(1, 0);
        board.update(1, 0);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(
            board
                .get_range(0, 10)
                .iter()
                .filter(|entry| entry.xp == 0)
                .count(),
            3
        );
        assert_eq!(board.get_rank(3), Some(1));
        assert_eq!(board.get_rank(2), Some(2));
        assert_eq!(board.get_rank(1), Some(3));

        board.update(1, 500_000);
        board.update(1, 500_000);
        assert_eq!(rx.len(), 1);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(
            board
                .get_range(0, 10)
                .iter()
                .filter(|entry| entry.xp == 0)
                .count(),
            2
        );
        assert_eq!(board.get_rank(1), Some(1));
    }

    #[test]
    fn zero_removal_and_reinsertion_leave_no_ghost_entries() {
        let (board, rx) = board();
        board.update(1, 0);
        board.remove(1);
        board.remove(1);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert!(board.get_range(0, 10).is_empty());
        assert_eq!(board.get_rank(1), None);

        board.update(1, 0);
        board.update(1, 100);
        board.remove(1);
        board.update(1, 0);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(board.get_rank(1), Some(1));
        board.remove(1);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert!(board.data.read().bucket_counts.is_empty());
    }

    #[test]
    fn concurrent_updates_and_removals_preserve_index_operation_order() {
        let (board, rx) = board();
        std::thread::scope(|scope| {
            for thread in 0..8 {
                let board = &board;
                scope.spawn(move || {
                    for iteration in 0..200 {
                        let xp = if iteration % 2 == 0 {
                            0
                        } else {
                            thread * 1_000 + iteration
                        };
                        board.update(1, xp);
                        if iteration % 5 == 0 {
                            board.remove(1);
                        }
                    }
                });
            }
        });
        board.update(1, 0);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(board.get_value(&1), Some(0));
        assert_eq!(board.get_range(0, 10).len(), 1);
    }

    #[test]
    fn repeated_player_updates_keep_one_pending_change_and_one_wakeup() {
        let (board, rx) = board();
        for xp in 0..100_000 {
            board.update(1, xp);
        }
        assert_eq!(board.state.lock().pending.len(), 1);
        assert_eq!(rx.len(), 1);
        assert_eq!(
            board.get_value(&1),
            Some(99_999),
            "score lookup remains immediate"
        );
        assert!(
            board.get_range(0, 10).is_empty(),
            "index worker is paused in this test"
        );
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert!(board.state.lock().pending.is_empty());
    }

    #[test]
    fn pending_changes_grow_with_players_not_update_count() {
        let (board, rx) = board();
        for xp in 0..20 {
            for user_id in 0..1_000 {
                board.update(user_id, xp);
            }
        }
        assert_eq!(board.state.lock().pending.len(), 1_000);
        assert_eq!(rx.len(), 1);
        apply_pending(&board, &rx);
        assert_consistent(&board);
    }

    #[test]
    fn coalescing_preserves_original_score_and_cancels_round_trips() {
        let (board, rx) = board();
        board.update(1, 0);
        apply_pending(&board, &rx);
        board.update(1, 100);
        board.update(1, 200);
        assert!(matches!(
            pending_op(&board, 1),
            Some(LeaderboardOp::Update {
                old_xp: 0,
                new_xp: 200,
                ..
            })
        ));
        board.remove(1);
        assert!(matches!(
            pending_op(&board, 1),
            Some(LeaderboardOp::Remove { xp: 0, .. })
        ));
        board.update(1, 300);
        assert!(matches!(
            pending_op(&board, 1),
            Some(LeaderboardOp::Update {
                old_xp: 0,
                new_xp: 300,
                ..
            })
        ));
        board.update(1, 0);
        assert!(board.state.lock().pending.is_empty());
        apply_pending(&board, &rx);
        assert_consistent(&board);
    }

    #[test]
    fn insert_followed_by_remove_cancels_pending_index_work() {
        let (board, rx) = board();
        board.update(1, 0);
        board.update(1, 100);
        board.remove(1);
        assert!(board.state.lock().pending.is_empty());
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(board.get_rank(1), None);
    }

    #[test]
    fn updates_after_worker_drain_use_the_in_flight_batches_final_score() {
        let (board, rx) = board();
        board.update(1, 0);
        rx.try_recv().unwrap();
        let in_flight = std::mem::take(&mut board.state.lock().pending);
        // These mutations arrive after the worker takes its batch but before it
        // applies it, so their baseline must be the in-flight score of zero.
        board.update(1, 100);
        board.remove(1);
        board.update(1, 200);
        assert!(matches!(
            pending_op(&board, 1),
            Some(LeaderboardOp::Update {
                old_xp: 0,
                new_xp: 200,
                ..
            })
        ));
        assert_eq!(
            rx.len(),
            1,
            "mutations arriving after drain must wake the worker again"
        );
        Leaderboard::apply_changes(&board.data, &board.players, in_flight);
        apply_pending(&board, &rx);
        assert_consistent(&board);
        assert_eq!(board.get_value(&1), Some(200));
    }

    #[test]
    fn background_worker_handles_concurrent_writes_and_drains_on_shutdown() {
        let (board, rx) = board();
        let inner = Arc::clone(&board.data);
        let state = Arc::clone(&board.state);
        let players = Arc::clone(&board.players);
        let worker_players = Arc::clone(&players);
        let worker_inner = Arc::clone(&inner);
        let worker = std::thread::spawn(move || {
            Leaderboard::worker_loop(worker_inner, state, worker_players, rx)
        });
        std::thread::scope(|scope| {
            for thread in 0..8 {
                let board = &board;
                scope.spawn(move || {
                    for xp in 0..1_000 {
                        board.update(1, xp);
                        board.update(thread + 2, xp);
                        if xp % 7 == 0 {
                            board.remove(1);
                        }
                    }
                });
            }
        });
        board.update(1, 0);
        let expected = board
            .score_entries()
            .into_iter()
            .map(|(id, xp)| ((xp, id), ()))
            .collect::<BTreeMap<_, _>>();
        drop(board);
        worker.join().unwrap();
        let data = inner.read();
        assert_eq!(data.sorted_players.len(), expected.len());
        for (rank, &(_, id)) in expected.keys().rev().enumerate() {
            let index = players.get(id).unwrap();
            assert_eq!(data.sorted_players[rank], index);
            assert_eq!(data.ranks[index as usize], (rank + 1) as u32);
        }
        assert_eq!(data.bucket_counts.values().sum::<usize>(), 9);
    }
}

#[cfg(feature = "bench")]
pub struct LeaderboardBenchHarness {
    leaderboard: Leaderboard,
    user_ids: Vec<i64>,
    wakeups: crossbeam_channel::Receiver<()>,
}

#[cfg(feature = "bench")]
impl LeaderboardBenchHarness {
    pub fn new(player_count: usize) -> Self {
        // Fully publish the initial snapshot before timing rank lookups.
        let (leaderboard, wakeups) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let mut user_ids = Vec::with_capacity(player_count);

        for idx in 0..player_count {
            let user_id = (idx as i64) + 1;
            let xp = (idx as i64) * 137 + ((idx as i64) % 53) * 17;
            leaderboard.update(user_id, xp);
            user_ids.push(user_id);
        }
        let changes = std::mem::take(&mut leaderboard.state.lock().pending);
        Leaderboard::apply_changes(&leaderboard.data, &leaderboard.players, changes);
        while wakeups.try_recv().is_ok() {}

        Self {
            leaderboard,
            user_ids,
            wakeups,
        }
    }

    pub fn user_at_percentile(&self, percentile: usize) -> i64 {
        let max_idx = self.user_ids.len().saturating_sub(1);
        let idx = ((max_idx as u128) * (percentile.min(100) as u128) / 100u128) as usize;
        self.user_ids[idx]
    }

    pub fn get_rank(&self, user_id: i64) -> Option<usize> {
        self.leaderboard.get_rank(user_id)
    }

    pub fn get_value(&self, user_id: i64) -> Option<i64> {
        self.leaderboard.get_value(&user_id)
    }

    pub fn get_range(&self, offset: usize, limit: usize) -> Vec<LeaderboardEntry> {
        self.leaderboard.get_range(offset, limit)
    }

    /// Time ingestion plus publication, not just queueing writes for later.
    /// The live worker's 100 ms refresh pacing is intentionally not included.
    pub fn update_and_publish(&mut self, users: &[i64], offset: i64) {
        for (position, &user_id) in users.iter().enumerate() {
            let index = user_id - 1;
            let xp = index * 137 + (index % 53) * 17 + offset + position as i64;
            self.leaderboard.update(user_id, xp);
        }
        while self.wakeups.try_recv().is_ok() {}
        let changes = std::mem::take(&mut self.leaderboard.state.lock().pending);
        Leaderboard::apply_changes(&self.leaderboard.data, &self.leaderboard.players, changes);
    }
}

pub(crate) struct RankingSystem {
    bucket_config: crate::config::LeaderboardConfig,
    players: Arc<PlayerRegistry>,
    worker: RankingWorker,
    skill_leaderboards: DashMap<i64, Arc<Leaderboard>>,
    pub global_leaderboard: Leaderboard,
    pub profession_leaderboard: Leaderboard,
    pub adventure_leaderboard: Leaderboard,
    pub xp_per_hour: Leaderboard,
    pub level_leaderboard: Leaderboard,
    pub time_played: Leaderboard,
    pub time_signed_in: Leaderboard,
}

impl Default for RankingSystem {
    fn default() -> Self {
        Self::with_bucket_config(crate::config::LeaderboardConfig::default())
    }
}

impl RankingSystem {
    pub(crate) fn with_bucket_config(bucket_config: crate::config::LeaderboardConfig) -> Self {
        let players = Arc::new(PlayerRegistry::default());
        let worker = RankingWorker::new();
        let board = |name: &str| {
            Leaderboard::with_worker_and_buckets(
                Arc::clone(&players),
                ScoreWidth::Wide,
                &worker,
                Self::bucket_layout(&bucket_config, name),
            )
        };
        Self {
            skill_leaderboards: DashMap::new(),
            global_leaderboard: board("global_experience"),
            profession_leaderboard: board("profession_experience"),
            adventure_leaderboard: board("adventure_experience"),
            level_leaderboard: board("level"),
            xp_per_hour: board("xp_per_hour"),
            time_played: board("time_played"),
            time_signed_in: board("time_signed_in"),
            bucket_config,
            players,
            worker,
        }
    }

    fn bucket_layout(config: &crate::config::LeaderboardConfig, name: &str) -> BucketLayout {
        config
            .bucket_widths
            .get(name)
            .copied()
            // 19 skills × 130 levels = 2,470 total levels. Width 50 gives
            // useful detail within the current top-100 range of 1,294–1,800.
            .or_else(|| (name == "level").then(|| std::num::NonZeroU32::new(50).unwrap()))
            .map_or(BucketLayout::Tiered, BucketLayout::Fixed)
    }
}

impl RankingSystem {
    pub(crate) fn subscribe_publications(&self) -> tokio::sync::watch::Receiver<u64> {
        self.worker.subscribe()
    }

    pub(crate) async fn shutdown(&self) {
        if let Some(thread) = self.worker.stop() {
            match tokio::task::spawn_blocking(move || thread.join()).await {
                Ok(Ok(())) => {}
                _ => tracing::error!("Ranking publisher failed during shutdown"),
            }
        }
    }

    /// Allocation hint only. Boards still grow for players absent from the
    /// database count, and boards created later inherit the registry hint.
    pub(crate) fn reserve_player_capacity(&self, count: usize) {
        self.players.reserve_players(count);
        for board in [
            &self.global_leaderboard,
            &self.profession_leaderboard,
            &self.adventure_leaderboard,
            &self.level_leaderboard,
            &self.xp_per_hour,
            &self.time_played,
            &self.time_signed_in,
        ] {
            board.reserve_players(count);
        }
        for (_, board) in self.skill_boards() {
            board.reserve_players(count);
        }
    }

    pub(crate) fn print_startup_memory_stats(&self) {
        let mut boards = 0usize;
        let mut buffer_bytes = 0usize;
        let mut pending_changes = 0usize;
        let mut score_slots = 0usize;
        let mut record = |board: &Leaderboard| {
            let stats = board.memory_stats();
            boards += 1;
            buffer_bytes += stats.buffer_bytes;
            pending_changes += stats.pending_changes;
            score_slots += stats.score_slots;
        };
        for board in [
            &self.global_leaderboard,
            &self.profession_leaderboard,
            &self.adventure_leaderboard,
            &self.level_leaderboard,
            &self.xp_per_hour,
            &self.time_played,
            &self.time_signed_in,
        ] {
            record(board);
        }
        for (_, board) in self.skill_boards() {
            record(&board);
        }
        eprintln!(
            "startup_rankings boards={boards} registered_players={} buffer_capacity_bytes={buffer_bytes} score_slots={score_slots} pending_changes={pending_changes}",
            self.players.len()
        );
    }

    pub(crate) fn record_memory_metrics(&self) {
        metrics::gauge!("leaderboard_registered_players").set(self.players.len() as f64);
        let record = |name: &str, board: &Leaderboard| {
            let stats = board.memory_stats();
            metrics::gauge!("leaderboard_active_players", "leaderboard" => name.to_string())
                .set(stats.active_players as f64);
            metrics::gauge!("leaderboard_score_slots", "leaderboard" => name.to_string())
                .set(stats.score_slots as f64);
            // Vec payload capacities only: this is not allocator accounting or RSS.
            metrics::gauge!("leaderboard_buffer_capacity_bytes", "leaderboard" => name.to_string())
                .set(stats.buffer_bytes as f64);
            metrics::gauge!("leaderboard_pending_changes", "leaderboard" => name.to_string())
                .set(stats.pending_changes as f64);
            metrics::gauge!("leaderboard_pending_change_capacity", "leaderboard" => name.to_string()).set(stats.pending_capacity as f64);
        };
        for (name, board) in [
            ("global_experience", &self.global_leaderboard),
            ("profession_experience", &self.profession_leaderboard),
            ("adventure_experience", &self.adventure_leaderboard),
            ("level", &self.level_leaderboard),
            ("xp_per_hour", &self.xp_per_hour),
            ("time_played", &self.time_played),
            ("time_signed_in", &self.time_signed_in),
        ] {
            record(name, board);
        }
        for (id, board) in self.skill_boards() {
            record(&format!("skill_{id}"), &board);
        }
    }

    /// Do not let DashMap guards escape into code that locks a board, creates
    /// another board, performs IO, or accesses another cache.
    pub(crate) fn skill_leaderboard(&self, skill_id: i64) -> Option<Arc<Leaderboard>> {
        self.skill_leaderboards
            .get(&skill_id)
            .map(|board| Arc::clone(board.value()))
    }

    pub(crate) fn skill_boards(&self) -> Vec<(i64, Arc<Leaderboard>)> {
        self.skill_leaderboards
            .iter()
            .map(|board| (*board.key(), Arc::clone(board.value())))
            .collect()
    }

    pub(crate) fn ensure_skill_leaderboard(&self, skill_id: i64) -> Arc<Leaderboard> {
        if let Some(board) = self.skill_leaderboard(skill_id) {
            return board;
        }
        // Construct/register outside the map write lock. Concurrent creators
        // atomically retain the first board; unused weak registrations expire.
        let candidate = Arc::new(Leaderboard::with_worker_and_buckets(
            Arc::clone(&self.players),
            ScoreWidth::Skill,
            &self.worker,
            Self::bucket_layout(&self.bucket_config, &format!("skill_{skill_id}")),
        ));
        use dashmap::mapref::entry::Entry;
        match self.skill_leaderboards.entry(skill_id) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(entry) => Arc::clone(entry.insert(candidate).value()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, TS)]
#[ts(export)]
pub(crate) struct LeaderboardSkill {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) level: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardLevel {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) level: u32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardExperiencePerHour {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardExperience {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i64,
    pub(crate) experience_per_hour: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardTime {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) time_played: u64,
    pub(crate) rank: u64,
}

type LeaderboardRankTypeTasks =
    Vec<tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>>;

pub(crate) async fn get_top_100(
    state: State<AppState>,
) -> Result<axum_codec::Codec<GetTop100Response>, (StatusCode, &'static str)> {
    let skills = state
        .skill_desc
        .iter()
        .map(|skill_desc| skill_desc.clone())
        .collect::<Vec<_>>();

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    // let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut results = vec![];

    let entries_time_played = state.ranking_system.time_played.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_time_played.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Time(LeaderboardTime {
            player_id: entry.user_id,
            player_name: None,
            time_played: entry.xp as u64,
            rank: rank as u64,
        }));
    }
    results.push(("Time Played".to_string(), leaderboard));

    let entries_time_signed_in = state.ranking_system.time_signed_in.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_time_signed_in.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Time(LeaderboardTime {
            player_id: entry.user_id,
            player_name: None,
            time_played: entry.xp as u64,
            rank: rank as u64,
        }));
    }
    results.push(("Time Online".to_string(), leaderboard));

    let entries_profession_leaderboard = state
        .ranking_system
        .profession_leaderboard
        .get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_profession_leaderboard.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Experience(LeaderboardExperience {
            player_id: entry.user_id,
            player_name: None,
            experience_per_hour: 0,
            experience: entry.xp,
            rank: rank as u64,
        }));
    }

    results.push(("Professions".to_string(), leaderboard));

    let entries_adventure_leaderboard =
        state.ranking_system.adventure_leaderboard.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_adventure_leaderboard.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Experience(LeaderboardExperience {
            player_id: entry.user_id,
            player_name: None,
            experience_per_hour: 0,
            experience: entry.xp,
            rank: rank as u64,
        }));
    }

    results.push(("Skills".to_string(), leaderboard));

    let entries_per_hour_xp = state.ranking_system.xp_per_hour.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_per_hour_xp.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::ExperiencePerHour(LeaderboardExperiencePerHour {
            player_id: entry.user_id,
            player_name: None,
            experience: entry.xp as i32,
            rank: rank as u64,
        }));
    }

    results.push(("Experience Per Hour".to_string(), leaderboard));

    let entries_total_level = state.ranking_system.level_leaderboard.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_total_level.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Level(LeaderboardLevel {
            player_id: entry.user_id,
            player_name: None,
            level: entry.xp as u32,
            rank: rank as u64,
        }));
    }

    results.push(("Level".to_string(), leaderboard));

    let entries_total_xp = state.ranking_system.global_leaderboard.get_range(0, 100);
    let mut leaderboard: Vec<RankType> = Vec::new();

    for (i, entry) in entries_total_xp.into_iter().enumerate() {
        let rank = i + 1;
        leaderboard.push(RankType::Experience(LeaderboardExperience {
            player_id: entry.user_id,
            player_name: None,
            experience: entry.xp,
            experience_per_hour: 0,
            rank: rank as u64,
        }));
    }

    results.push(("Experience".to_string(), leaderboard));

    for skill in skills {
        if skill.skill_category == 0 {
            continue;
        }

        let entries = state
            .ranking_system
            .skill_leaderboard(skill.id)
            .map(|board| board.get_range(0, 100))
            .unwrap_or_default();

        let mut leaderboard: Vec<RankType> = Vec::new();

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            let player_name = None;

            leaderboard.push(RankType::Skill(LeaderboardSkill {
                player_id: entry.user_id,
                player_name,
                experience: entry.xp as i32,
                level: experience_to_level(entry.xp),
                rank: rank as u64,
            }));
        }

        results.push((skill.name.clone(), leaderboard));
    }

    let mut player_ids: Vec<i64> = vec![];

    for (name, mut leaderboard) in results.into_iter() {
        player_ids.append(
            &mut leaderboard
                .iter()
                .map(|x| match x {
                    RankType::Skill(x) => x.player_id,
                    RankType::Level(x) => x.player_id,
                    RankType::Experience(x) => x.player_id,
                    RankType::Time(x) => x.player_id,
                    RankType::ExperiencePerHour(x) => x.player_id,
                })
                .collect::<Vec<i64>>(),
        );

        leaderboard_result
            .entry(name)
            .or_default()
            .append(&mut leaderboard);
    }

    player_ids.sort();
    player_ids.dedup();

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, player_ids.clone())
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for player_id in &player_ids {
        if !players_name_by_id.contains_key(player_id) {
            tracing::warn!("Player {} not found", player_id);
        }
    }

    for top in leaderboard_result.values_mut() {
        for x in top.iter_mut() {
            match x {
                RankType::Skill(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Level(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Experience(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Time(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::ExperiencePerHour(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
            };
        }
    }

    let players = player_ids
        .iter()
        .filter_map(|player_id| {
            state
                .player_state
                .get(player_id)
                .map(|player| (*player_id, player.value().clone()))
        })
        .collect::<HashMap<i64, entity::player_state::Model>>();

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}

/// Publish the same consistent, named rows as the HTTP endpoint after ranking
/// publication. Coalesce busy periods and do no database work without viewers.
pub(crate) fn start_live_updates(state: AppState) {
    let mut publications = state.ranking_system.subscribe_publications();
    let shutdown = state.shutdown.clone();
    shutdown.spawn_worker(async move {
        let topics = crate::websocket::BroadcastTopics::new([("leaderboard".into(), None)]);
        loop {
            tokio::select! {
                _ = state.shutdown.stop.cancelled() => break,
                result = publications.changed() => if result.is_err() { break },
            }
            tokio::select! {
                _ = state.shutdown.stop.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }
            publications.borrow_and_update();
            if !state.tx.is_interested(&topics) {
                continue;
            }
            match get_top_100(State(state.clone())).await {
                Ok(axum_codec::Codec(board)) => {
                    let _ = state.tx.send(crate::websocket::OutboundWebSocketMessages::LeaderboardUpdated(board));
                }
                Err(error) => tracing::warn!(?error, "Failed to publish leaderboard"),
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct GetTop100Response {
    pub player_map: HashMap<i64, entity::player_state::Model>,
    pub leaderboard: BTreeMap<String, Vec<RankType>>,
}

pub(crate) fn experience_to_level(experience: i64) -> i32 {
    let boundary = EXPERIENCE_PER_LEVEL.partition_point(|(_, xp)| *xp <= experience);
    // Preserve the existing fallback for negative XP as well as exact thresholds.
    boundary
        .checked_sub(1)
        .map_or(100, |index| EXPERIENCE_PER_LEVEL[index].0)
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct PlayerLeaderboardResponse(BTreeMap<String, RankType>);

pub(crate) async fn player_leaderboard(
    state: State<AppState>,
    Path(player_id): Path<i64>,
) -> Result<axum_codec::Codec<PlayerLeaderboardResponse>, (StatusCode, &'static str)> {
    let skills = state
        .skill_desc
        .iter()
        .map(|skill| skill.value().clone())
        .collect::<Vec<_>>();

    let has_player = state.player_state.contains_key(&player_id);

    if !has_player {
        tracing::warn!(player_id, "Player leaderboard with no player");

        return Err((StatusCode::NOT_FOUND, "Not found"));
    };

    let mut leaderboard_result: BTreeMap<String, RankType> = BTreeMap::new();

    let mut results = vec![];

    for skill in skills {
        if skill.skill_category == 0 {
            continue;
        }

        let player_name = None;

        let Some((skill_exp, rank)) = state
            .ranking_system
            .skill_leaderboard(skill.id)
            .and_then(|board| board.get_rank(player_id).zip(board.get_value(&player_id)))
            .map(|(rank, experience)| (experience, rank))
        else {
            tracing::warn!(
                player_id,
                skill_id = skill.id,
                "Could not find skill experience rank for player"
            );
            continue;
        };

        results.push((
            skill.name.clone(),
            RankType::Skill(LeaderboardSkill {
                player_id,
                player_name,
                experience: skill_exp as i32,
                level: experience_to_level(skill_exp),
                rank: rank as u64,
            }),
        ));
    }

    let rank = if let Some(rank) = state.ranking_system.global_leaderboard.get_rank(player_id) {
        rank
    } else {
        tracing::warn!(player_id, "Could not find total experience rank for player");

        0
    };

    let total_experience = if let Some(total_experience) = state
        .ranking_system
        .global_leaderboard
        .get_value(&player_id)
    {
        total_experience
    } else {
        tracing::warn!(player_id, "Could not find total experience xp for player");

        0
    };

    results.push((
        "Experience".to_string(),
        RankType::Experience(LeaderboardExperience {
            player_id,
            player_name: None,
            experience: total_experience,
            experience_per_hour: 0,
            rank: rank as u64,
        }),
    ));

    let rank = if let Some(rank) = state
        .ranking_system
        .profession_leaderboard
        .get_rank(player_id)
    {
        rank
    } else {
        tracing::warn!(player_id, "Could not find total experience rank for player");

        0
    };

    let total_profession_experience = if let Some(total_profession_experience) = state
        .ranking_system
        .profession_leaderboard
        .get_value(&player_id)
    {
        total_profession_experience
    } else {
        tracing::warn!(player_id, "Could not find total profession xp for player");

        0
    };

    results.push((
        "Professions".to_string(),
        RankType::Experience(LeaderboardExperience {
            player_id,
            player_name: None,
            experience: total_profession_experience,
            experience_per_hour: 0,
            rank: rank as u64,
        }),
    ));

    let rank = if let Some(rank) = state
        .ranking_system
        .adventure_leaderboard
        .get_rank(player_id)
    {
        rank
    } else {
        tracing::warn!(player_id, "Could not find total experience rank for player");

        0
    };

    let total_adventure_experience = if let Some(total_adventure_experience) = state
        .ranking_system
        .adventure_leaderboard
        .get_value(&player_id)
    {
        total_adventure_experience
    } else {
        tracing::warn!(player_id, "Could not find total adventure xp for player");

        0
    };

    results.push((
        "Skills".to_string(),
        RankType::Experience(LeaderboardExperience {
            player_id,
            player_name: None,
            experience: total_adventure_experience,
            experience_per_hour: 0,
            rank: rank as u64,
        }),
    ));

    let rank = state.ranking_system.level_leaderboard.get_rank(player_id);
    let level = state.ranking_system.level_leaderboard.get_value(&player_id);

    results.push((
        "Level".to_string(),
        RankType::Level(LeaderboardLevel {
            player_id,
            player_name: None,
            level: level.unwrap_or_else(|| {
                tracing::warn!(player_id, "Level leaderboard has no level player");

                0
            }) as u32,
            rank: rank.unwrap_or_else(|| {
                tracing::warn!(player_id, "Level leaderboard has no rank player");

                0
            }) as u64,
        }),
    ));

    for (name, leaderboard) in results.into_iter() {
        leaderboard_result.entry(name).or_insert(leaderboard);
    }

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, vec![player_id])
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for top in leaderboard_result.values_mut() {
        match top {
            RankType::Skill(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Level(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Experience(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Time(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::ExperiencePerHour(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
        };
    }

    Ok(axum_codec::Codec(PlayerLeaderboardResponse(
        leaderboard_result,
    )))
}

pub(crate) async fn get_claim_leaderboard(
    state: State<AppState>,
    Path(claim_id): Path<i64>,
) -> Result<axum_codec::Codec<GetTop100Response>, (StatusCode, &'static str)> {
    let skills = state
        .skill_desc
        .iter()
        .map(|skill| skill.value().clone())
        .collect::<Vec<_>>();

    let player_ids = state
        .claim_member_state
        .get(&(claim_id as u64))
        .map(|members| {
            members
                .values()
                .map(|member| member.player_entity_id)
                .collect::<Vec<i64>>()
        })
        .unwrap_or_default();

    if player_ids.is_empty() {
        return Err((StatusCode::NOT_FOUND, ""));
    }

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: LeaderboardRankTypeTasks = vec![];

    for skill in skills {
        if skill.skill_category == 0 {
            continue;
        }

        let db = state.conn.clone();
        let player_ids = player_ids.clone();
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<RankType> = Vec::new();
            let entries = Query::get_experience_state_player_ids_by_skill_id(
                &db,
                skill.id,
                player_ids,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            for (i, entry) in entries.into_iter().enumerate() {
                let rank = i + 1;
                let player_name = None;

                leaderboard.push(RankType::Skill(LeaderboardSkill {
                    player_id: entry.entity_id,
                    player_name,
                    experience: entry.experience,
                    level: experience_to_level(entry.experience as i64),
                    rank: rank as u64,
                }));
            }

            Ok((skill.name.clone(), leaderboard))
        }));
    }

    let db = state.conn.clone();
    let tmp_player_ids = player_ids.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_total_experience(
            &db,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Experience(LeaderboardExperience {
                player_id: entry.entity_id,
                player_name: None,
                experience: entry.experience as i64,
                experience_per_hour: 0,
                rank: rank as u64,
            }));
        }

        Ok(("Experience".to_string(), leaderboard))
    }));

    let db = state.conn.clone();
    let tmp_player_ids = player_ids.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_total_level(
            &db,
            generated_level_sql,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Level(LeaderboardLevel {
                player_id: entry.0 as i64,
                player_name: None,
                level: entry.1 as u32,
                rank: rank as u64,
            }));
        }

        Ok(("Level".to_string(), leaderboard))
    }));

    let tmp_player_ids = player_ids.clone();
    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_time_played(
            &db,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Time(LeaderboardTime {
                player_id: entry.0 as i64,
                player_name: None,
                time_played: entry.1 as u64,
                rank: rank as u64,
            }));
        }

        Ok(("Time Played".to_string(), leaderboard))
    }));

    let tmp_player_ids = player_ids.clone();
    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_time_online(
            &db,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Time(LeaderboardTime {
                player_id: entry.0 as i64,
                player_name: None,
                time_played: entry.1 as u64,
                rank: rank as u64,
            }));
        }

        Ok(("Time Online".to_string(), leaderboard))
    }));

    let results = futures::future::join_all(tasks).await;
    let mut player_ids: Vec<i64> = vec![];

    for result in results.into_iter().flatten() {
        if let Ok((name, mut leaderboard)) = result {
            player_ids.append(
                &mut leaderboard
                    .iter()
                    .map(|x| match x {
                        RankType::Skill(x) => x.player_id,
                        RankType::Level(x) => x.player_id,
                        RankType::Experience(x) => x.player_id,
                        RankType::Time(x) => x.player_id,
                        RankType::ExperiencePerHour(x) => x.player_id,
                    })
                    .collect::<Vec<i64>>(),
            );

            leaderboard_result
                .entry(name)
                .or_default()
                .append(&mut leaderboard);
        } else {
            error!("Error: {result:?}");
        }
    }

    player_ids.sort();
    player_ids.dedup();

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, player_ids.clone())
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for top in leaderboard_result.values_mut() {
        for x in top.iter_mut() {
            match x {
                RankType::Skill(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Level(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Experience(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Time(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::ExperiencePerHour(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
            };
        }
    }

    let players = player_ids
        .iter()
        .filter_map(|player_id| {
            state
                .player_state
                .get(player_id)
                .map(|player| (*player_id, player.value().clone()))
        })
        .collect::<HashMap<i64, entity::player_state::Model>>();

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}
