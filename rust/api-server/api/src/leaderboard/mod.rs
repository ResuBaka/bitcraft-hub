pub(crate) mod bitcraft;

use crate::{AppRouter, AppState, leaderboard};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_skiplist::{SkipMap, SkipSet};
use dashmap::DashMap;
use log::error;
use serde::{Deserialize, Serialize};
use service::Query;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
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
    LazyLock::new(|| vec![360287970201941063, 504403158285774600]);
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
            "/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/leaderboard/claims/{claim_id}",
            axum_codec::routing::get(get_claim_leaderboard).into(),
        )
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

// The message sent to the background worker
enum LeaderboardOp {
    Update {
        user_id: i64,
        old_xp: i64,
        new_xp: i64,
    },
    Remove {
        user_id: i64,
        xp: i64,
    },
}

pub(super) struct LeaderboardInner {
    // ID -> XP
    // pub(crate) scores: DashMap<i64, i64>,
    // (XP, ID) for sorted iteration
    sorted_ranks: BTreeMap<(i64, i64), ()>,
    // Bucket ID -> Atomic Count (Ordered for range scans)
    bucket_counts: BTreeMap<i64, usize>,
    // XP -> Atomic Count (Ordered for range scans)
    xp_counts: BTreeMap<i64, usize>,
}

pub(super) struct Leaderboard {
    pub(crate) scores: DashMap<i64, i64>,
    // Protected by RwLock, but only ONE writer (the worker thread)
    // and many readers. This eliminates write-contention.
    data: Arc<RwLock<LeaderboardInner>>,
    tx: Sender<LeaderboardOp>,
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Leaderboard {
    const BUCKET_RANGES: [(i64, i64); 5] = [
        (50_000, 1_000),
        (250_000, 5_000),
        (1_000_000, 25_000),
        (5_000_000, 100_000),
        (i64::MAX, 500_000),
    ];

    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let inner = Arc::new(RwLock::new(LeaderboardInner {
            sorted_ranks: BTreeMap::new(),
            bucket_counts: BTreeMap::new(),
            xp_counts: BTreeMap::new(),
        }));

        let inner_clone = Arc::clone(&inner);

        std::thread::spawn(move || {
            Self::worker_loop(inner_clone, rx);
        });

        Self {
            scores: DashMap::new(),
            data: inner,
            tx,
        }
    }

    fn worker_loop(inner: Arc<RwLock<LeaderboardInner>>, rx: Receiver<LeaderboardOp>) {
        let mut batch = Vec::with_capacity(1024);

        while let Ok(first_op) = rx.recv() {
            // 1. Start a new batch with the first message (blocking)
            batch.push(first_op);

            // 2. "Drain" the channel of all currently pending messages (non-blocking)
            // This prevents us from locking/unlocking 1000 times for 1000 messages.
            while let Ok(op) = rx.try_recv() {
                batch.push(op);
                if batch.len() > 5000 {
                    break;
                } // Safety cap to prevent starvation
            }

            // 3. Apply the entire batch in ONE write lock
            if let Ok(mut guard) = inner.write() {
                for op in batch.drain(..) {
                    match op {
                        LeaderboardOp::Update {
                            user_id,
                            old_xp,
                            new_xp,
                        } => {
                            if old_xp != 0 {
                                guard.sorted_ranks.remove(&(old_xp, user_id));
                                Self::decrement_btreemap(
                                    &mut guard.bucket_counts,
                                    Self::bucket_for(old_xp),
                                );
                                Self::decrement_btreemap(&mut guard.xp_counts, old_xp);
                            }
                            guard.sorted_ranks.insert((new_xp, user_id), ());
                            *guard
                                .bucket_counts
                                .entry(Self::bucket_for(new_xp))
                                .or_insert(0) += 1;
                            *guard.xp_counts.entry(new_xp).or_insert(0) += 1;
                        }
                        LeaderboardOp::Remove { user_id, xp } => {
                            guard.sorted_ranks.remove(&(xp, user_id));
                            Self::decrement_btreemap(
                                &mut guard.bucket_counts,
                                Self::bucket_for(xp),
                            );
                            Self::decrement_btreemap(&mut guard.xp_counts, xp);
                        }
                    }
                }
            }
            // Write lock is dropped here, allowing readers back in.
        }
    }

    fn bucket_for(xp: i64) -> i64 {
        let normalized = xp.max(0);
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

    fn bucket_bounds(bucket: i64) -> (i64, i64) {
        let mut range_start = 0i64;
        let mut offset = 0i64;

        for (range_max, bucket_size) in Self::BUCKET_RANGES.iter() {
            if *range_max == i64::MAX {
                let idx_in_range = bucket - offset;
                let min = range_start + (idx_in_range * *bucket_size);
                let max = min + *bucket_size - 1;
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

    pub fn bucket_distribution(&self) -> Vec<BucketDistribution> {
        let guard = self.data.read().unwrap();
        guard
            .bucket_counts
            .iter()
            .map(|(&bucket, &count)| {
                let (min_xp, max_xp) = Self::bucket_bounds(bucket);
                BucketDistribution {
                    bucket,
                    min_xp,
                    max_xp,
                    count,
                }
            })
            .collect()
    }

    // fn increment_bucket(&self, bucket: i64) {
    //     let mut bucket_counts = self
    //         .bucket_counts
    //         .write()
    //         .unwrap_or_else(|poisoned| poisoned.into_inner());
    //     *bucket_counts.entry(bucket).or_insert(0) += 1;
    // }
    //
    // fn decrement_bucket(&self, bucket: i64) {
    //     let mut bucket_counts = self
    //         .bucket_counts
    //         .write()
    //         .unwrap_or_else(|poisoned| poisoned.into_inner());
    //
    //     if let Some(entry) = bucket_counts.get_mut(&bucket) {
    //         if *entry > 0 {
    //             *entry -= 1;
    //         }
    //
    //         if *entry == 0 {
    //             bucket_counts.remove(&bucket);
    //         }
    //     }
    // }
    //
    // fn increment_xp_count(&self, xp: i64) {
    //     let mut xp_counts = self
    //         .xp_counts
    //         .write()
    //         .unwrap_or_else(|poisoned| poisoned.into_inner());
    //     *xp_counts.entry(xp).or_insert(0) += 1;
    // }
    //
    // fn decrement_xp_count(&self, xp: i64) {
    //     let mut xp_counts = self
    //         .xp_counts
    //         .write()
    //         .unwrap_or_else(|poisoned| poisoned.into_inner());
    //
    //     if let Some(entry) = xp_counts.get_mut(&xp) {
    //         if *entry > 0 {
    //             *entry -= 1;
    //         }
    //
    //         if *entry == 0 {
    //             xp_counts.remove(&xp);
    //         }
    //     }
    // }

    fn increment_count(map: &SkipMap<i64, AtomicUsize>, key: i64) {
        if let Some(entry) = map.get(&key) {
            entry.value().fetch_add(1, Ordering::Relaxed);
        } else {
            // Entry doesn't exist, try to insert it
            map.get_or_insert(key, AtomicUsize::new(1));
        }
    }

    fn decrement_count(map: &SkipMap<i64, AtomicUsize>, key: i64) {
        if let Some(entry) = map.get(&key) {
            // Note: In highly concurrent systems, a count might briefly
            // drop to 0. We leave the key in the map to avoid
            // the overhead of constant removals.
            entry.value().fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub(super) fn update(&self, user_id: i64, new_xp: i64) {
        let mut entry = self.scores.entry(user_id).or_insert(0);
        let old_xp = *entry.value();

        if old_xp == new_xp {
            return;
        }

        *entry = new_xp;
        // The "Write" is now just a channel send. Extremely fast.
        let _ = self.tx.send(LeaderboardOp::Update {
            user_id,
            old_xp,
            new_xp,
        });
    }

    pub(super) fn has(&self, user_id: &i64) -> bool {
        self.scores.contains_key(user_id)
    }

    pub(super) fn get_value<'a>(&self, user_id: &i64) -> Option<i64> {
        if let Some(xp) = self.scores.get(user_id) {
            Some(*xp)
        } else {
            None
        }
    }

    pub(super) fn get_rank(&self, user_id: i64) -> Option<usize> {
        let xp = *self.scores.get(&user_id)?;
        let guard = self.data.read().ok()?;

        let bucket = Self::bucket_for(xp);
        let (_, bucket_max) = Self::bucket_bounds(bucket);

        let higher_count: usize = guard
            .bucket_counts
            .range((bucket + 1)..)
            .map(|(_, &c)| c)
            .sum();
        let above_same_bucket_higher_xp: usize = if xp >= bucket_max {
            0
        } else {
            guard
                .xp_counts
                .range((xp + 1)..=bucket_max)
                .map(|(_, &c)| c)
                .sum()
        };

        // Tie-break scan in BTreeMap is very fast (cache friendly)
        let mut tie_rank = 0;
        for ((entry_xp, _), _) in guard.sorted_ranks.range((xp, user_id)..=(xp, i64::MAX)) {
            tie_rank += 1;
        }

        Some(higher_count + above_same_bucket_higher_xp + tie_rank)
    }

    pub(super) fn remove(&self, user_id: i64) {
        if let Some((_, xp)) = self.scores.remove(&user_id) {
            let _ = self.tx.send(LeaderboardOp::Remove { user_id, xp });
        }
    }

    pub(super) fn get_range(&self, offset: usize, limit: usize) -> Vec<LeaderboardEntry> {
        // Acquire the read lock.
        // This will only block if the background thread is currently applying a batch.
        let guard = self.data.read().unwrap_or_else(|p| p.into_inner());

        guard
            .sorted_ranks
            .iter()
            .rev() // Highest XP first
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(idx, (&(xp, user_id), _))| {
                LeaderboardEntry {
                    // rank is offset + current index + 1 (for 1-based ranking)
                    rank: offset + idx + 1,
                    user_id,
                    xp,
                }
            })
            .collect()
    }

    fn decrement_btreemap(map: &mut BTreeMap<i64, usize>, key: i64) {
        if let Some(count) = map.get_mut(&key) {
            if *count <= 1 {
                map.remove(&key);
            } else {
                *count -= 1;
            }
        }
    }
}

#[cfg(feature = "bench")]
pub struct LeaderboardBenchHarness {
    leaderboard: Leaderboard,
    user_ids: Vec<i64>,
}

#[cfg(feature = "bench")]
impl LeaderboardBenchHarness {
    pub fn new(player_count: usize) -> Self {
        let leaderboard = Leaderboard::default();
        let mut user_ids = Vec::with_capacity(player_count);

        for idx in 0..player_count {
            let user_id = (idx as i64) + 1;
            let xp = (idx as i64) * 137 + ((idx as i64) % 53) * 17;
            leaderboard.update(user_id, xp);
            user_ids.push(user_id);
        }

        Self {
            leaderboard,
            user_ids,
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
}

pub struct RankingSystem {
    pub skill_leaderboards: DashMap<i64, Leaderboard>,
    pub global_leaderboard: Leaderboard,
    pub xp_per_hour: Leaderboard,
    pub level_leaderboard: Leaderboard,
    pub time_played: Leaderboard,
    pub time_signed_in: Leaderboard,
}

impl Default for RankingSystem {
    fn default() -> Self {
        Self {
            skill_leaderboards: DashMap::new(),
            global_leaderboard: Leaderboard::default(),
            level_leaderboard: Leaderboard::default(),
            xp_per_hour: Leaderboard::default(),
            time_played: Leaderboard::default(),
            time_signed_in: Leaderboard::default(),
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
            player_id: entry.user_id as i64,
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
            player_id: entry.user_id as i64,
            player_name: None,
            time_played: entry.xp as u64,
            rank: rank as u64,
        }));
    }

    results.push(("Time Online".to_string(), leaderboard));

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
            experience_per_hour: state
                .ranking_system
                .xp_per_hour
                .scores
                .get(&entry.user_id)
                .unwrap()
                .value()
                .clone() as i32,
            rank: rank as u64,
        }));
    }

    results.push(("Experience".to_string(), leaderboard));

    for skill in skills {
        if skill.skill_category == 0 {
            continue;
        }

        // let db = state.conn.clone();
        let entries = state
            .ranking_system
            .skill_leaderboards
            .get(&skill.id)
            .unwrap()
            .get_range(0, 100);

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

    for (_, top) in leaderboard_result.iter_mut() {
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

    let players = Query::find_player_by_ids(&state.conn, player_ids.clone())
        .await
        .unwrap_or_else(|error| {
            error!("Error loading players: {error}");

            vec![]
        })
        .iter()
        .map(|player| (player.entity_id, player.clone()))
        .collect::<HashMap<i64, entity::player_state::Model>>();

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct GetTop100Response {
    pub player_map: HashMap<i64, entity::player_state::Model>,
    pub leaderboard: BTreeMap<String, Vec<RankType>>,
}

pub(crate) fn experience_to_level(experience: i64) -> i32 {
    if experience == 0 {
        return 1;
    }

    for (level, xp) in EXPERIENCE_PER_LEVEL.iter().rev() {
        if experience.gt(xp) || experience.eq(xp) {
            return *level;
        }
    }

    100i32
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct PlayerLeaderboardResponse(BTreeMap<String, RankType>);

pub(crate) async fn player_leaderboard(
    state: State<AppState>,
    Path(player_id): Path<i64>,
) -> Result<axum_codec::Codec<PlayerLeaderboardResponse>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

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

        let (skill_exp, rank) = if let Some(a) = state
            .ranking_system
            .skill_leaderboards
            .get(&skill.id)
            .unwrap()
            .scores
            .get(&player_id)
        {
            let rank = state
                .ranking_system
                .skill_leaderboards
                .get(&skill.id)
                .unwrap()
                .get_rank(player_id);

            (a.clone(), rank.unwrap())
        } else {
            let db = state.conn.clone();
            let (entrie, _rank) = Query::get_experience_state_player_by_skill_id(
                &db,
                skill.id,
                player_id,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD.clone()),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            if let Some(result) = entrie {
                state
                    .ranking_system
                    .skill_leaderboards
                    .get(&skill.id)
                    .unwrap()
                    .update(player_id.clone(), result.experience as i64);

                let rank = state
                    .ranking_system
                    .skill_leaderboards
                    .get(&skill.id)
                    .unwrap()
                    .get_rank(player_id);

                (result.experience as i64, rank.unwrap())
            } else {
                tracing::warn!(
                    player_id,
                    skill_id = skill.id,
                    "Could not find player skill experience"
                );

                (0, 0)
            }
        };

        results.push((
            skill.name.clone(),
            RankType::Skill(LeaderboardSkill {
                player_id: player_id.clone(),
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
        .scores
        .get(&player_id)
    {
        total_experience.value().clone()
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

    let rank = state.ranking_system.level_leaderboard.get_rank(player_id);
    let level = state
        .ranking_system
        .level_leaderboard
        .scores
        .get(&player_id);

    results.push((
        "Level".to_string(),
        RankType::Level(LeaderboardLevel {
            player_id,
            player_name: None,
            level: level.unwrap().value().clone() as u32,
            rank: rank.unwrap() as u64,
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

    for (_, top) in leaderboard_result.iter_mut() {
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
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let claim_member = Query::find_claim_member_by_claim_id(&state.conn, claim_id)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    if claim_member.is_empty() {
        return Err((StatusCode::NOT_FOUND, ""));
    }

    let player_ids = claim_member
        .iter()
        .map(|member| member.player_entity_id)
        .collect::<Vec<i64>>();

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

    for (_, top) in leaderboard_result.iter_mut() {
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

    let players = Query::find_player_by_ids(&state.conn, player_ids.clone())
        .await
        .unwrap_or_else(|error| {
            error!("Error loading players: {error}");

            vec![]
        })
        .iter()
        .map(|player| (player.entity_id, player.clone()))
        .collect::<HashMap<i64, entity::player_state::Model>>();

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}
