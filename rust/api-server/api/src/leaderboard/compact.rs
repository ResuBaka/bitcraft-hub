//! Dense leaderboard arrays share a single stable player-ID registry. Removed
//! players leave absent slots in their boards; registry indices are never
//! renumbered or assigned to another player. Immediate scores and published
//! rankings are separate snapshots.
use super::{BucketDistribution, BucketLayout, LeaderboardEntry};
use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::{Mutex, RwLock};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

// Coalesce bursts so large batches can use one linear merge. Small batches use
// targeted rotations instead of scanning and merging the entire ranking.
const RANK_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const SPARSE_UPDATE_LIMIT: usize = 128;
const MAX_RETAINED_PENDING_CAPACITY: usize = 16_384;

/// Check the adjacent player first: most score changes do not change rank.
/// Fall back to the directional binary search for actual moves, preserving
/// logarithmic search cost even for jumps across the entire board.
fn sparse_update_position(
    sorted: &[u32],
    old_position: usize,
    increasing: bool,
    mut higher_than_new: impl FnMut(&u32) -> bool,
) -> usize {
    if increasing {
        if old_position == 0 || higher_than_new(&sorted[old_position - 1]) {
            old_position
        } else {
            sorted[..old_position].partition_point(higher_than_new)
        }
    } else if old_position + 1 == sorted.len() || !higher_than_new(&sorted[old_position + 1]) {
        old_position
    } else {
        old_position + sorted[old_position + 1..].partition_point(higher_than_new)
    }
}

/// Shared by every board in a RankingSystem. Indices are stable and never reused.
#[derive(Default)]
pub(super) struct PlayerRegistry {
    inner: RwLock<PlayerIds>,
    capacity_hint: AtomicUsize,
}

#[derive(Default)]
struct PlayerIds {
    by_id: HashMap<i64, u32>,
    ids: Vec<i64>,
}

/// A stable identity slot, not a cached score/rank. Borrowing the registry ties
/// its lifetime to the owner; slots are never renumbered or reused.
pub(super) struct ResolvedPlayer<'a> {
    registry: &'a PlayerRegistry,
    index: u32,
}

impl ResolvedPlayer<'_> {
    fn index_for(&self, board: &Leaderboard) -> u32 {
        assert!(
            std::ptr::eq(self.registry, board.players.as_ref()),
            "player handle belongs to another ranking system"
        );
        self.index
    }
}

impl PlayerRegistry {
    pub(super) fn reserve_players(&self, count: usize) {
        self.capacity_hint.fetch_max(count, AtomicOrdering::Relaxed);
        let mut inner = self.inner.write();
        reserve_vec(&mut inner.ids, count);
        let additional = count.saturating_sub(inner.by_id.len());
        inner.by_id.reserve(additional);
    }

    pub(super) fn len(&self) -> usize {
        self.inner.read().ids.len()
    }

    pub(super) fn get(&self, user_id: i64) -> Option<u32> {
        self.inner.read().by_id.get(&user_id).copied()
    }

    fn register(&self, user_id: i64) -> u32 {
        if let Some(index) = self.get(user_id) {
            return index;
        }
        let mut inner = self.inner.write();
        if let Some(&index) = inner.by_id.get(&user_id) {
            return index;
        }
        // Reserve zero as the absent rank; the number of players must fit u32.
        let index = u32::try_from(inner.ids.len() + 1)
            .expect("leaderboard player registry exhausted u32 indices")
            - 1;
        inner.ids.push(user_id);
        inner.by_id.insert(user_id, index);
        index
    }
}

// Reserve a total capacity, not `count` additional elements. Never create slots
// or shrink existing allocations: a hint may be stale or an underestimate.
fn reserve_vec<T>(values: &mut Vec<T>, count: usize) {
    if values.capacity() < count {
        values.reserve_exact(count.saturating_sub(values.len()));
    }
}

#[derive(Clone, Copy)]
pub(super) enum ScoreWidth {
    Skill,
    Wide,
}

enum ScoreValues {
    Skill(Vec<i32>),
    Wide(Vec<i64>),
}

struct Scores {
    values: ScoreValues,
    // Explicit presence: zero, negative scores and every i64 value remain valid.
    present: Vec<u64>,
    len: usize,
}

impl Scores {
    fn reserve_players(&mut self, count: usize) {
        match &mut self.values {
            ScoreValues::Skill(values) => reserve_vec(values, count),
            ScoreValues::Wide(values) => reserve_vec(values, count),
        }
        reserve_vec(&mut self.present, count.div_ceil(64));
    }

    fn buffer_bytes(&self) -> usize {
        let values = match &self.values {
            ScoreValues::Skill(values) => values.capacity() * std::mem::size_of::<i32>(),
            ScoreValues::Wide(values) => values.capacity() * std::mem::size_of::<i64>(),
        };
        values + self.present.capacity() * std::mem::size_of::<u64>()
    }

    fn new(width: ScoreWidth) -> Self {
        Self {
            values: match width {
                ScoreWidth::Skill => ScoreValues::Skill(Vec::new()),
                ScoreWidth::Wide => ScoreValues::Wide(Vec::new()),
            },
            present: Vec::new(),
            len: 0,
        }
    }

    fn slots(&self) -> usize {
        match &self.values {
            ScoreValues::Skill(values) => values.len(),
            ScoreValues::Wide(values) => values.len(),
        }
    }

    fn get(&self, index: u32) -> Option<i64> {
        let index = index as usize;
        if self.present.get(index / 64).copied().unwrap_or(0) & (1 << (index % 64)) == 0 {
            return None;
        }
        Some(match &self.values {
            ScoreValues::Skill(values) => i64::from(values[index]),
            ScoreValues::Wide(values) => values[index],
        })
    }

    fn set(&mut self, index: u32, value: Option<i64>) {
        let existed = self.get(index).is_some();
        let index = index as usize;
        if let Some(value) = value {
            // Skill data is i32 in SpacetimeDB, but never silently truncate an
            // unexpected wider value. Promote just this board's score storage.
            if let ScoreValues::Skill(values) = &self.values
                && i32::try_from(value).is_err()
            {
                self.values = ScoreValues::Wide(values.iter().map(|&v| i64::from(v)).collect());
            }
            match &mut self.values {
                ScoreValues::Skill(values) => {
                    if index >= values.len() {
                        values.resize(index + 1, 0);
                    }
                    values[index] = value as i32;
                }
                ScoreValues::Wide(values) => {
                    if index >= values.len() {
                        values.resize(index + 1, 0);
                    }
                    values[index] = value;
                }
            }
            if index / 64 >= self.present.len() {
                self.present.resize(index / 64 + 1, 0);
            }
            self.present[index / 64] |= 1 << (index % 64);
            if !existed {
                self.len += 1;
            }
        } else if existed {
            self.present[index / 64] &= !(1 << (index % 64));
            self.len -= 1;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LeaderboardOp {
    Insert { new_xp: i64 },
    Update { old_xp: i64, new_xp: i64 },
    Remove { xp: i64 },
}

impl LeaderboardOp {
    fn previous_score(self) -> Option<i64> {
        match self {
            Self::Insert { .. } => None,
            Self::Update { old_xp, .. } => Some(old_xp),
            Self::Remove { xp } => Some(xp),
        }
    }

    fn latest_score(self) -> Option<i64> {
        match self {
            Self::Insert { new_xp } | Self::Update { new_xp, .. } => Some(new_xp),
            Self::Remove { .. } => None,
        }
    }
}

pub(super) struct BoardState {
    scores: Scores,
    pub(super) pending: HashMap<u32, LeaderboardOp>,
}

struct WorkerTask {
    data: Weak<RwLock<LeaderboardInner>>,
    state: Weak<Mutex<BoardState>>,
    players: Weak<PlayerRegistry>,
}

/// One publisher per RankingSystem, not one allocating thread per board. Weak
/// task registrations must not keep dropped boards or the RankingSystem alive.
pub(super) struct RankingWorker {
    tasks: Arc<Mutex<Vec<WorkerTask>>>,
    wakeup: Sender<()>,
    stopping: Arc<AtomicBool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl RankingWorker {
    pub(super) fn new() -> Self {
        let tasks = Arc::new(Mutex::new(Vec::<WorkerTask>::new()));
        let worker_tasks = Arc::clone(&tasks);
        let (wakeup, rx) = bounded(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = std::thread::spawn(move || {
            let mut last_refresh: Option<Instant> = None;
            let mut changes = HashMap::new();
            let mut ready = Vec::new();
            while rx.recv().is_ok() {
                // A stop arriving mid-pass needs another complete pass: an
                // already-visited board may have received its final update.
                let final_pass = stop.load(AtomicOrdering::Acquire);
                if let Some(last) = last_refresh
                    && let Some(wait) = RANK_REFRESH_INTERVAL.checked_sub(last.elapsed())
                {
                    std::thread::sleep(wait);
                }
                worker_tasks.lock().retain(|task| {
                    match (
                        task.data.upgrade(),
                        task.state.upgrade(),
                        task.players.upgrade(),
                    ) {
                        (Some(data), Some(state), Some(players)) => {
                            ready.push((data, state, players));
                            true
                        }
                        _ => false,
                    }
                });
                let mut published = false;
                for (data, state, players) in ready.drain(..) {
                    std::mem::swap(&mut changes, &mut state.lock().pending);
                    if !changes.is_empty() {
                        Leaderboard::apply_reused(&data, &players, &mut changes);
                        if changes.capacity() > MAX_RETAINED_PENDING_CAPACITY {
                            changes.shrink_to(4_096);
                        }
                        published = true;
                    }
                }
                if published {
                    last_refresh = Some(Instant::now());
                }
                if final_pass {
                    break;
                }
            }
        });
        Self {
            tasks,
            wakeup,
            stopping,
            thread: Mutex::new(Some(thread)),
        }
    }

    /// Call only after score producers stop. The final wake publishes pending
    /// updates before the thread exits; joining also releases its scratch buffers.
    pub(super) fn stop(&self) -> Option<std::thread::JoinHandle<()>> {
        self.stopping.store(true, AtomicOrdering::Release);
        let _ = self.wakeup.try_send(());
        self.thread.lock().take()
    }

    fn register(&self, board: &Leaderboard) {
        self.tasks.lock().push(WorkerTask {
            data: Arc::downgrade(&board.data),
            state: Arc::downgrade(&board.state),
            players: Arc::downgrade(&board.players),
        });
        let _ = self.wakeup.try_send(());
    }
}

/// A consistent published ranking. Scores are separate from the immediate score
/// array so reads never combine an old ordering with a newly updated score.
pub(super) struct LeaderboardInner {
    buckets: BucketLayout,
    scores: Scores,
    pub(super) sorted_players: Vec<u32>,
    scratch: Vec<u32>,
    updated_players: Vec<u32>,
    pub(super) ranks: Vec<u32>,
    pub(super) bucket_counts: BTreeMap<i64, usize>,
}

impl LeaderboardInner {
    fn reserve_players(&mut self, count: usize) {
        self.scores.reserve_players(count);
        reserve_vec(&mut self.sorted_players, count);
        reserve_vec(&mut self.scratch, count);
        reserve_vec(&mut self.ranks, count);
        // This buffer is deliberately trimmed to 8,192 after publication.
        reserve_vec(&mut self.updated_players, count.min(8_192));
        // BTreeMap bucket nodes are allocated on demand; it has no reserve API.
    }

    fn new(width: ScoreWidth) -> Self {
        Self {
            buckets: BucketLayout::default(),
            scores: Scores::new(width),
            sorted_players: Vec::new(),
            scratch: Vec::new(),
            updated_players: Vec::new(),
            ranks: Vec::new(),
            bucket_counts: BTreeMap::new(),
        }
    }

    fn apply(&mut self, players: &PlayerIds, changes: &mut HashMap<u32, LeaderboardOp>) {
        if changes.is_empty() {
            return;
        }
        if changes.len() <= SPARSE_UPDATE_LIMIT {
            self.apply_sparse(players, changes);
            return;
        }
        let mut updated = std::mem::take(&mut self.updated_players);
        updated.clear();
        updated.reserve(changes.len());
        for (&index, &op) in changes.iter() {
            let new = op.latest_score();
            self.set_score(index, new);
            if new.is_some() {
                updated.push(index);
            }
        }
        self.ranks.resize(self.scores.slots(), 0);
        for &index in changes.keys() {
            if let Some(rank) = self.ranks.get_mut(index as usize) {
                *rank = 0;
            }
        }
        // Reuse rank zero as a temporary dirty marker, avoiding a hash lookup
        // for every unchanged player during the linear merge.
        self.sorted_players
            .retain(|&index| self.ranks[index as usize] != 0);
        let compare = |a: &u32, b: &u32| {
            self.scores
                .get(*b)
                .cmp(&self.scores.get(*a))
                .then_with(|| players.ids[*b as usize].cmp(&players.ids[*a as usize]))
        };
        // Sort only changed players, then merge with the unchanged ordering.
        // Avoid O(N log N) full re-sorts, or O(N) shifting for every single update.
        updated.sort_unstable_by(compare);
        self.scratch.clear();
        self.scratch
            .reserve(self.sorted_players.len() + updated.len());
        let mut existing = self.sorted_players.iter().copied().peekable();
        let mut incoming = updated.iter().copied().peekable();
        while let (Some(a), Some(b)) = (existing.peek(), incoming.peek()) {
            if compare(a, b) != Ordering::Greater {
                self.scratch.push(existing.next().unwrap());
            } else {
                self.scratch.push(incoming.next().unwrap());
            }
        }
        self.scratch.extend(existing);
        self.scratch.extend(incoming);
        updated.clear();
        if updated.capacity() > 8_192 {
            updated.shrink_to(8_192);
        }
        self.updated_players = updated;
        std::mem::swap(&mut self.sorted_players, &mut self.scratch);
        self.scratch.clear();
        debug_assert_eq!(self.sorted_players.len(), self.scores.len);
        for (rank, &index) in self.sorted_players.iter().enumerate() {
            self.ranks[index as usize] = u32::try_from(rank + 1).expect("rank exceeds u32");
        }
        changes.clear();
    }

    fn set_score(&mut self, index: u32, new: Option<i64>) {
        let old_bucket = self
            .scores
            .get(index)
            .map(|score| self.buckets.bucket_for(score));
        let new_bucket = new.map(|score| self.buckets.bucket_for(score));
        if old_bucket != new_bucket {
            if let Some(bucket) = old_bucket {
                if let Some(count) = self.bucket_counts.get_mut(&bucket) {
                    *count -= 1;
                    if *count == 0 {
                        self.bucket_counts.remove(&bucket);
                    }
                }
            }
            if let Some(bucket) = new_bucket {
                *self.bucket_counts.entry(bucket).or_insert(0) += 1;
            }
        }
        self.scores.set(index, new);
    }

    fn refresh_ranks(&mut self, start: usize, end: usize) {
        for position in start..end {
            let index = self.sorted_players[position];
            self.ranks[index as usize] = u32::try_from(position + 1).expect("rank exceeds u32");
        }
    }

    fn apply_sparse(&mut self, players: &PlayerIds, changes: &mut HashMap<u32, LeaderboardOp>) {
        for (index, op) in changes.drain() {
            let old = self.scores.get(index);
            let new = op.latest_score();
            if old == new {
                continue;
            }
            let higher_than_new = |other: &u32| {
                (
                    self.scores.get(*other).expect("ranked player has a score"),
                    players.ids[*other as usize],
                ) > (new.unwrap_or(0), players.ids[index as usize])
            };
            match (old, new) {
                (Some(old_xp), Some(new_xp)) => {
                    let old_position = self.ranks[index as usize] as usize - 1;
                    let new_position = sparse_update_position(
                        &self.sorted_players,
                        old_position,
                        new_xp > old_xp,
                        higher_than_new,
                    );
                    self.set_score(index, new);
                    if new_position == old_position {
                        // Published scores and bucket counts still change, but
                        // neither ordering nor rank positions need rewriting.
                        continue;
                    }
                    if new_position < old_position {
                        self.sorted_players[new_position..=old_position].rotate_right(1);
                    } else if new_position > old_position {
                        self.sorted_players[old_position..=new_position].rotate_left(1);
                    }
                    self.refresh_ranks(
                        old_position.min(new_position),
                        old_position.max(new_position) + 1,
                    );
                }
                (None, Some(_)) => {
                    let position = self.sorted_players.partition_point(higher_than_new);
                    self.set_score(index, new);
                    self.ranks.resize(self.scores.slots(), 0);
                    self.sorted_players.insert(position, index);
                    self.refresh_ranks(position, self.sorted_players.len());
                }
                (Some(_), None) => {
                    let position = self.ranks[index as usize] as usize - 1;
                    self.set_score(index, None);
                    self.sorted_players.remove(position);
                    self.ranks[index as usize] = 0;
                    self.refresh_ranks(position, self.sorted_players.len());
                }
                (None, None) => unreachable!(),
            }
        }
        debug_assert_eq!(self.sorted_players.len(), self.scores.len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_player_reuses_identity_but_reads_current_board_presence_and_rank() {
        let players = Arc::new(PlayerRegistry::default());
        let (first, _rx) = Leaderboard::paused(players.clone(), ScoreWidth::Wide);
        let player = first.resolve_player(7);
        assert_eq!(first.get_rank_resolved(&player), None);
        first.update_resolved(&player, 100);
        assert_eq!(
            first.get_rank_resolved(&player),
            None,
            "no rank until published"
        );
        first.update(8, 200);
        flush(&first);
        assert_eq!(first.get_rank_resolved(&player), Some(2));
        first.update(8, 0);
        flush(&first);
        assert_eq!(
            first.get_rank_resolved(&player),
            Some(1),
            "other players invalidate rank, not identity"
        );

        // Dynamically created boards and registry growth share the same stable
        // slot, but membership and published ranks remain independent per board.
        let (second, _rx) = Leaderboard::paused(players.clone(), ScoreWidth::Skill);
        assert_eq!(second.get_rank_resolved(&player), None);
        for id in 100..1000 {
            players.register(id);
        }
        second.update_resolved(&player, 300);
        flush(&second);
        assert_eq!(second.get_value(&7), Some(300));
        assert_eq!(second.get_rank_resolved(&player), Some(1));
        first.remove(7);
        assert_eq!(first.get_rank_resolved(&player), None);
        assert_eq!(second.get_rank_resolved(&player), Some(1));
        first.update_resolved(&player, -100);
        flush(&first);
        assert_eq!(first.get_rank_resolved(&player), first.get_rank(7));
        assert_eq!(first.get_value(&7), Some(-100));
    }

    #[test]
    #[should_panic(expected = "player handle belongs to another ranking system")]
    fn resolved_player_cannot_alias_another_registry_slot() {
        let (first, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let (second, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let player = first.resolve_player(7);
        second.update_resolved(&player, 100);
    }

    #[test]
    fn neighbor_fast_path_matches_directional_search_for_every_destination() {
        for len in 1..=64 {
            let sorted: Vec<u32> = (0..len).collect();
            for old_position in 0..len as usize {
                for boundary in 0..=len {
                    // The predicate is true before the insertion boundary.
                    // Excluding the player's own slot adjusts downward moves.
                    let increasing = boundary as usize <= old_position;
                    let expected = if increasing {
                        boundary as usize
                    } else {
                        boundary as usize - 1
                    };
                    assert_eq!(
                        sparse_update_position(&sorted, old_position, increasing, |other| *other
                            < boundary),
                        expected,
                        "len={len} old={old_position} boundary={boundary}",
                    );
                }
            }
        }
    }

    #[test]
    fn unchanged_position_needs_at_most_one_comparison() {
        let sorted: Vec<u32> = (0..4096).collect();
        for position in [0, 1, 2048, 4095] {
            for increasing in [true, false] {
                let mut comparisons = 0;
                let boundary = position + usize::from(!increasing);
                let result = sparse_update_position(&sorted, position, increasing, |other| {
                    comparisons += 1;
                    (*other as usize) < boundary
                });
                assert_eq!(result, position);
                assert!(
                    comparisons <= 1,
                    "unchanged rank must not binary-search the board"
                );
            }
        }
    }

    #[test]
    fn custom_buckets_track_bulk_updates_moves_and_removals() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        board.data.write().buckets = BucketLayout::Fixed(std::num::NonZeroU32::new(1).unwrap());
        for id in 1..=150 {
            board.update(id, id);
        }
        let changes = std::mem::take(&mut board.state.lock().pending);
        Leaderboard::apply_changes(&board.data, &board.players, changes);
        let distribution = board.bucket_distribution();
        assert_eq!(distribution.len(), 150);
        for (level, bucket) in (1..=150).zip(distribution) {
            assert_eq!(
                (bucket.min_xp, bucket.max_xp, bucket.count),
                (level, level, 1)
            );
        }
        board.update(1, 150);
        board.remove(2);
        let changes = std::mem::take(&mut board.state.lock().pending);
        Leaderboard::apply_changes(&board.data, &board.players, changes);
        let distribution = board.bucket_distribution();
        assert_eq!(distribution.len(), 148);
        assert_eq!(distribution.last().unwrap().count, 2);
        assert_eq!(
            distribution
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>(),
            149
        );
    }

    #[test]
    fn ranking_system_applies_bucket_config_to_aggregate_and_dynamic_boards() {
        let config =
            serde_json::from_str(r#"{"bucket_widths":{"level":5,"skill_1":100}}"#).unwrap();
        let system = super::super::RankingSystem::with_bucket_config(config);
        system.ensure_skill_leaderboard(1);
        system.ensure_skill_leaderboard(2);
        let bounds = |board: &Leaderboard| {
            let buckets = board.data.read().buckets;
            buckets.bucket_bounds(buckets.bucket_for(149))
        };
        assert_eq!(bounds(&system.level_leaderboard), (145, 149));
        assert_eq!(bounds(&system.global_leaderboard), (0, 999));
        assert_eq!(
            bounds(&system.skill_leaderboards.get(&1).unwrap()),
            (100, 199)
        );
        assert_eq!(
            bounds(&system.skill_leaderboards.get(&2).unwrap()),
            (0, 999)
        );
    }

    #[test]
    fn unchanged_rank_still_publishes_scores_and_bucket_changes() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let mut expected = BTreeMap::from([(1, 1_000_000), (2, 5_000), (3, -1_000_000)]);
        for (&id, &xp) in &expected {
            board.update(id, xp);
        }
        flush(&board);
        for xp in [10_000, 100, -100, -5_000, 5_000] {
            board.update(2, xp);
            expected.insert(2, xp);
            flush(&board);
            assert_eq!(board.get_rank(2), Some(2));
            verify(&board, &expected);
        }
    }

    fn flush(board: &Leaderboard) {
        let changes = std::mem::take(&mut board.state.lock().pending);
        Leaderboard::apply_changes(&board.data, &board.players, changes);
    }

    fn verify(board: &Leaderboard, reference: &BTreeMap<i64, i64>) {
        let mut expected = reference
            .iter()
            .map(|(&id, &xp)| (xp, id))
            .collect::<Vec<_>>();
        expected.sort_unstable_by(|a, b| b.cmp(a));
        let actual = board.get_range(0, usize::MAX);
        assert_eq!(
            actual
                .iter()
                .map(|entry| (entry.xp, entry.user_id))
                .collect::<Vec<_>>(),
            expected
        );
        let mut buckets = BTreeMap::new();
        for (position, &(xp, id)) in expected.iter().enumerate() {
            assert_eq!(board.get_rank(id), Some(position + 1));
            assert_eq!(board.get_value(&id), Some(xp));
            assert_eq!(actual[position].rank, position + 1);
            *buckets
                .entry(BucketLayout::default().bucket_for(xp))
                .or_insert(0) += 1;
        }
        let data = board.data.read();
        assert_eq!(data.bucket_counts, buckets);
        assert_eq!(data.scores.len, reference.len());
        assert!(
            data.sorted_players
                .iter()
                .all(|&index| data.scores.get(index).is_some())
        );
    }

    #[test]
    fn shared_indices_are_stable_and_absence_is_independent_for_each_board() {
        let players = Arc::new(PlayerRegistry::default());
        let (skill, _rx) = Leaderboard::paused(Arc::clone(&players), ScoreWidth::Skill);
        let (aggregate, _rx2) = Leaderboard::paused(Arc::clone(&players), ScoreWidth::Wide);
        skill.update(i64::MAX - 1, 0);
        aggregate.update(42, 100);
        aggregate.update(i64::MAX - 1, 200);
        let index = players.get(i64::MAX - 1).unwrap();
        assert_eq!(index, 0);
        assert_eq!(players.get(42), Some(1));
        assert!(skill.has(&(i64::MAX - 1)));
        assert!(!skill.has(&42));
        skill.remove(i64::MAX - 1);
        assert_eq!(players.get(i64::MAX - 1), Some(index));
        skill.update(-7, 10);
        skill.update(i64::MAX - 1, 0);
        assert_eq!(players.get(i64::MAX - 1), Some(index));
        assert_eq!(players.get(-7), Some(2));
        flush(&skill);
        flush(&aggregate);
        verify(&skill, &BTreeMap::from([(i64::MAX - 1, 0), (-7, 10)]));
        verify(
            &aggregate,
            &BTreeMap::from([(i64::MAX - 1, 200), (42, 100)]),
        );
    }

    #[test]
    fn equal_scores_are_ordered_by_real_player_id_not_registration_index() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Skill);
        let mut expected = BTreeMap::new();
        for id in [300, -20, i64::MAX - 1, 0, 42] {
            board.update(id, 0);
            expected.insert(id, 0);
        }
        flush(&board);
        verify(&board, &expected);
        assert_eq!(board.get_rank(i64::MAX - 1), Some(1));
        assert_eq!(board.get_rank(-20), Some(5));
        let page = board.get_range(1, 2);
        assert_eq!(
            page.iter()
                .map(|entry| (entry.user_id, entry.rank))
                .collect::<Vec<_>>(),
            [(300, 2), (42, 3)]
        );
        assert!(board.get_range(usize::MAX, 10).is_empty());
    }

    #[test]
    fn skill_storage_is_narrow_and_unexpected_wide_scores_are_not_truncated() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Skill);
        board.update(1, i64::from(i32::MAX));
        board.update(2, i64::from(i32::MIN));
        flush(&board);
        assert!(matches!(
            board.state.lock().scores.values,
            ScoreValues::Skill(_)
        ));
        assert!(matches!(
            board.data.read().scores.values,
            ScoreValues::Skill(_)
        ));
        board.update(3, i64::from(i32::MAX) + 1);
        flush(&board);
        assert!(matches!(
            board.state.lock().scores.values,
            ScoreValues::Wide(_)
        ));
        assert!(matches!(
            board.data.read().scores.values,
            ScoreValues::Wide(_)
        ));
        verify(
            &board,
            &BTreeMap::from([
                (1, i64::from(i32::MAX)),
                (2, i64::from(i32::MIN)),
                (3, i64::from(i32::MAX) + 1),
            ]),
        );
    }

    #[test]
    fn aggregates_preserve_all_i64_scores_and_zero_membership() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let expected = BTreeMap::from([
            (1, i64::MAX),
            (2, i64::MIN),
            (3, 0),
            (4, i64::from(i32::MAX) + 5),
        ]);
        for (&id, &xp) in &expected {
            board.update(id, xp);
        }
        flush(&board);
        verify(&board, &expected);
        assert!(
            board
                .bucket_distribution()
                .iter()
                .any(|bucket| bucket.max_xp == i64::MAX)
        );
        assert_eq!(board.get_value(&5), None);
        assert_eq!(board.get_rank(5), None);
    }

    #[test]
    fn presence_bitset_and_rank_tombstones_work_across_word_boundaries() {
        let players = Arc::new(PlayerRegistry::default());
        for id in 0..150 {
            players.register(id);
        }
        let (board, _rx) = Leaderboard::paused(players, ScoreWidth::Skill);
        let mut expected = BTreeMap::new();
        for id in [0, 63, 64, 65, 127, 128, 149] {
            board.update(id, 0);
            expected.insert(id, 0);
        }
        flush(&board);
        for id in [63, 64, 128] {
            board.remove(id);
            expected.remove(&id);
        }
        flush(&board);
        verify(&board, &expected);
        for id in [1, 62, 63, 64, 66, 126, 128, 148] {
            assert_eq!(board.get_value(&id), None);
            assert_eq!(board.get_rank(id), None);
        }
        board.update(64, 0);
        expected.insert(64, 0);
        flush(&board);
        verify(&board, &expected);
    }

    #[test]
    fn published_scores_and_ordering_stay_consistent_while_updates_are_pending() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Skill);
        board.update(1, 100);
        board.update(2, 200);
        flush(&board);
        board.update(1, 300);
        assert_eq!(board.get_value(&1), Some(300));
        assert_eq!(board.get_rank(1), Some(2));
        assert_eq!(
            board
                .get_range(0, 2)
                .iter()
                .map(|entry| (entry.user_id, entry.xp))
                .collect::<Vec<_>>(),
            [(2, 200), (1, 100)]
        );
        flush(&board);
        verify(&board, &BTreeMap::from([(1, 300), (2, 200)]));
    }

    #[test]
    fn new_player_can_receive_experience_updates_before_their_first_rank_is_published() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        board.update(1, 100);
        flush(&board);
        assert_eq!(board.get_rank(2), None); // handler's prev_rank
        board.update(2, 150);
        assert_eq!(board.get_value(&2), Some(150));
        assert_eq!(board.get_rank(2), None); // handler's immediate post_rank
        board.update(2, 200); // another live update before first publication
        assert_eq!(board.get_rank(2), None);
        flush(&board);
        assert_eq!(board.get_rank(2), Some(1));
        assert_eq!(board.get_rank(1), Some(2));
        verify(&board, &BTreeMap::from([(1, 100), (2, 200)]));
    }

    #[test]
    fn batched_merges_match_a_full_sort_after_mixed_changes_and_removals() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Skill);
        let mut reference = BTreeMap::new();
        let mut seed = 42u64;
        for step in 0..10_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = ((seed >> 32) % 150) as i64 * 1_000_000_000_000;
            if seed % 5 == 0 {
                board.remove(id);
                reference.remove(&id);
            } else {
                let xp = ((seed >> 16) % 100_000) as i64;
                board.update(id, xp);
                reference.insert(id, xp);
            }
            if step % 17 == 0 {
                flush(&board);
                verify(&board, &reference);
            }
        }
        flush(&board);
        verify(&board, &reference);
    }

    #[test]
    fn ranking_system_shares_one_registry_for_all_aggregate_and_dynamic_skill_boards() {
        let system = super::super::RankingSystem::default();
        system.ensure_skill_leaderboard(10);
        system.ensure_skill_leaderboard(11);
        let skill = system.skill_leaderboards.get(&10).unwrap();
        skill.update(123, 0);
        for board in [
            &system.global_leaderboard,
            &system.profession_leaderboard,
            &system.adventure_leaderboard,
            &system.xp_per_hour,
            &system.level_leaderboard,
            &system.time_played,
            &system.time_signed_in,
        ] {
            assert!(Arc::ptr_eq(&skill.players, &board.players));
            board.update(123, i64::from(i32::MAX) + 1);
            assert_eq!(board.players.get(123), skill.players.get(123));
        }
        assert!(Arc::ptr_eq(
            &skill.players,
            &system.skill_leaderboards.get(&11).unwrap().players
        ));
        drop(skill);
        system.ensure_skill_leaderboard(10);
        assert_eq!(
            system.skill_leaderboards.get(&10).unwrap().get_value(&123),
            Some(0)
        );
    }
    #[test]
    fn production_sized_boards_use_compact_array_payloads() {
        use std::mem::size_of;
        fn score_bytes(scores: &Scores) -> usize {
            let values = match &scores.values {
                ScoreValues::Skill(values) => values.capacity() * size_of::<i32>(),
                ScoreValues::Wide(values) => values.capacity() * size_of::<i64>(),
            };
            values + scores.present.capacity() * size_of::<u64>()
        }
        fn array_bytes(board: &Leaderboard) -> usize {
            let state = board.state.lock();
            let data = board.data.read();
            score_bytes(&state.scores)
                + score_bytes(&data.scores)
                + (data.sorted_players.capacity()
                    + data.scratch.capacity()
                    + data.ranks.capacity()
                    + data.updated_players.capacity())
                    * size_of::<u32>()
        }
        const PLAYERS: usize = 48_506;
        let players = Arc::new(PlayerRegistry::default());
        let (skill, _rx) = Leaderboard::paused(Arc::clone(&players), ScoreWidth::Skill);
        let (aggregate, _rx2) = Leaderboard::paused(players, ScoreWidth::Wide);
        for id in 0..PLAYERS as i64 {
            skill.update(id, id % 10_000);
            aggregate.update(id, id * 100_000);
        }
        for board in [&skill, &aggregate] {
            flush(board);
            for id in 0..=SPARSE_UPDATE_LIMIT as i64 {
                board.update(id, 100_000 + id);
            }
            flush(board);
            assert_eq!(board.get_range(0, usize::MAX).len(), PLAYERS);
        }
        let skill_bytes = array_bytes(&skill);
        let aggregate_bytes = array_bytes(&aggregate);
        println!(
            "{PLAYERS} players: skill array payload = {skill_bytes} bytes; aggregate array payload = {aggregate_bytes} bytes (excludes shared registry, pending changes, allocator overhead)"
        );
        assert!(skill_bytes < PLAYERS * 32);
        assert!(aggregate_bytes < PLAYERS * 48);
    }

    #[test]
    fn sparse_updates_cross_ties_and_preserve_unaffected_rank_positions() {
        let (board, _rx) =
            Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide);
        let mut reference = (1..=500)
            .map(|id| (id, id / 10))
            .collect::<BTreeMap<_, _>>();
        for (&id, &xp) in &reference {
            board.update(id, xp);
        }
        flush(&board);
        for (id, xp) in [
            (1, 100),
            (500, -100),
            (250, 25),
            (251, 25),
            (1, 0),
            (500, 50),
            (499, 0),
        ] {
            board.update(id, xp);
            reference.insert(id, xp);
            flush(&board);
            verify(&board, &reference);
        }
        for id in [1, 250, 500] {
            board.remove(id);
            reference.remove(&id);
            flush(&board);
            verify(&board, &reference);
        }
    }

    #[test]
    fn both_sides_of_sparse_batch_threshold_match_full_sort() {
        for batch_size in [
            SPARSE_UPDATE_LIMIT - 1,
            SPARSE_UPDATE_LIMIT,
            SPARSE_UPDATE_LIMIT + 1,
        ] {
            let (board, _rx) =
                Leaderboard::paused(Arc::new(PlayerRegistry::default()), ScoreWidth::Skill);
            let mut reference = (1..=600)
                .map(|id| (id, id * 10))
                .collect::<BTreeMap<_, _>>();
            for (&id, &xp) in &reference {
                board.update(id, xp);
            }
            flush(&board);
            for id in 1..=batch_size as i64 {
                if id % 3 == 0 {
                    board.remove(id);
                    reference.remove(&id);
                } else {
                    board.update(id, 10_000 - id);
                    reference.insert(id, 10_000 - id);
                }
            }
            flush(&board);
            verify(&board, &reference);
        }
    }

    fn wait_for(mut ready: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !ready() {
            assert!(
                Instant::now() < deadline,
                "shared ranking worker did not finish"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn shared_worker_publishes_independent_boards_and_dynamic_registrations() {
        let worker = RankingWorker::new();
        let players = Arc::new(PlayerRegistry::default());
        let first = Leaderboard::with_worker(Arc::clone(&players), ScoreWidth::Skill, &worker);
        let second = Leaderboard::with_worker(Arc::clone(&players), ScoreWidth::Wide, &worker);
        for id in 1..=300 {
            first.update(id, id);
            second.update(id, id * 2);
        }
        wait_for(|| {
            first.data.read().sorted_players.len() == 300
                && second.data.read().sorted_players.len() == 300
        });
        let third = Leaderboard::with_worker(players, ScoreWidth::Skill, &worker);
        first.update(1, 0);
        second.remove(1);
        third.update(1, 500);
        wait_for(|| {
            first
                .get_range(299, 1)
                .first()
                .is_some_and(|entry| entry.user_id == 1 && entry.xp == 0)
                && second.data.read().sorted_players.len() == 299
                && third.get_rank(1) == Some(1)
        });
        verify(
            &first,
            &(1..=300)
                .map(|id| (id, if id == 1 { 0 } else { id }))
                .collect(),
        );
        verify(&second, &(2..=300).map(|id| (id, id * 2)).collect());
        verify(&third, &BTreeMap::from([(1, 500)]));
    }

    #[test]
    fn stopping_shared_worker_publishes_pending_changes_and_joins() {
        let worker = RankingWorker::new();
        let board = Leaderboard::with_worker(
            Arc::new(PlayerRegistry::default()),
            ScoreWidth::Skill,
            &worker,
        );
        for id in 1..=100 {
            board.update(id, id);
        }
        worker.stop().unwrap().join().unwrap();
        assert_eq!(board.get_rank(100), Some(1));
        assert!(board.state.lock().pending.is_empty());
        assert!(worker.stop().is_none());
    }

    #[test]
    fn stop_during_publication_refreshes_already_visited_boards() {
        let worker = RankingWorker::new();
        let players = Arc::new(PlayerRegistry::default());
        let first = Leaderboard::with_worker(players.clone(), ScoreWidth::Skill, &worker);
        let second = Leaderboard::with_worker(players, ScoreWidth::Skill, &worker);
        let blocked = second.data.write();
        first.update(1, 1);
        second.update(2, 2);
        // The publisher has visited first and drained second's pending map,
        // but is blocked applying second. Queue first's final score now.
        wait_for(|| first.get_rank(1) == Some(1) && second.state.lock().pending.is_empty());
        first.update(1, 9);
        let thread = worker.stop().unwrap();
        drop(blocked);
        thread.join().unwrap();
        assert_eq!(first.get_range(0, 1)[0].xp, 9);
        assert!(first.state.lock().pending.is_empty());
    }

    #[test]
    fn shared_worker_does_not_keep_dropped_boards_alive() {
        let worker = RankingWorker::new();
        let board = Leaderboard::with_worker(
            Arc::new(PlayerRegistry::default()),
            ScoreWidth::Skill,
            &worker,
        );
        board.update(1, 100);
        let data = Arc::downgrade(&board.data);
        let state = Arc::downgrade(&board.state);
        drop(board);
        wait_for(|| data.upgrade().is_none() && state.upgrade().is_none());
        let _ = worker.wakeup.try_send(());
        wait_for(|| worker.tasks.lock().is_empty());
    }

    #[test]
    fn board_can_outlive_scheduler_owner_without_losing_pending_changes() {
        let worker = RankingWorker::new();
        let board = Leaderboard::with_worker(
            Arc::new(PlayerRegistry::default()),
            ScoreWidth::Skill,
            &worker,
        );
        drop(worker);
        board.update(1, 0);
        board.update(1, 100);
        wait_for(|| {
            board
                .get_range(0, 1)
                .first()
                .is_some_and(|entry| entry.xp == 100)
        });
        verify(&board, &BTreeMap::from([(1, 100)]));
    }

    #[test]
    fn memory_stats_expose_sparse_slot_overhead_and_all_array_buffers() {
        let players = Arc::new(PlayerRegistry::default());
        for id in 0..1_000 {
            players.register(id);
        }
        let (board, _rx) = Leaderboard::paused(players, ScoreWidth::Skill);
        board.update(999, 0);
        let pending = board.memory_stats();
        assert_eq!(pending.active_players, 1);
        assert_eq!(pending.score_slots, 1_000);
        assert_eq!(pending.pending_changes, 1);
        flush(&board);
        let published = board.memory_stats();
        assert_eq!(published.active_players, 1);
        assert_eq!(published.pending_changes, 0);
        assert!(published.buffer_bytes > pending.buffer_bytes);
        assert!(published.buffer_bytes >= 1_000 * (4 + 4 + 4));
    }

    #[test]
    fn capacity_hint_reserves_without_creating_players_or_scores() {
        let players = Arc::new(PlayerRegistry::default());
        players.reserve_players(1_000);
        let (board, _rx) = Leaderboard::paused(Arc::clone(&players), ScoreWidth::Skill);
        assert_eq!(players.len(), 0);
        assert!(players.inner.read().ids.capacity() >= 1_000);
        assert!(players.inner.read().by_id.capacity() >= 1_000);
        assert_eq!(board.memory_stats().active_players, 0);
        assert_eq!(board.memory_stats().score_slots, 0);
        assert!(board.get_range(0, 10).is_empty());
        assert!(board.state.lock().pending.capacity() >= 1_000);
        let before = board.memory_stats().buffer_bytes;
        for id in 0..1_000 {
            board.update(id, id);
        }
        flush(&board);
        assert_eq!(board.memory_stats().buffer_bytes, before);
        verify(&board, &(0..1_000).map(|id| (id, id)).collect());
    }

    #[test]
    fn capacity_hint_can_be_exceeded_or_applied_to_populated_boards() {
        let players = Arc::new(PlayerRegistry::default());
        players.reserve_players(1);
        let (board, _rx) = Leaderboard::paused(Arc::clone(&players), ScoreWidth::Wide);
        for id in 0..200 {
            board.update(id, id);
        }
        flush(&board);
        let expected = (0..200).map(|id| (id, id)).collect();
        verify(&board, &expected);
        players.reserve_players(1_000);
        board.reserve_players(1_000);
        let bytes = board.memory_stats().buffer_bytes;
        board.reserve_players(0);
        assert_eq!(board.memory_stats().buffer_bytes, bytes);
        verify(&board, &expected);
    }

    #[test]
    fn ranking_system_hint_reaches_existing_and_future_skill_boards() {
        let system = super::super::RankingSystem::default();
        system.ensure_skill_leaderboard(1);
        system.reserve_player_capacity(1_000);
        system.ensure_skill_leaderboard(2);
        assert_eq!(system.players.len(), 0);
        for board in [
            &system.global_leaderboard,
            &system.profession_leaderboard,
            &system.adventure_leaderboard,
            &system.level_leaderboard,
            &system.xp_per_hour,
            &system.time_played,
            &system.time_signed_in,
        ] {
            assert!(board.data.read().ranks.capacity() >= 1_000);
            assert_eq!(board.memory_stats().active_players, 0);
        }
        for id in [1, 2] {
            let board = system.skill_leaderboards.get(&id).unwrap();
            assert!(board.data.read().ranks.capacity() >= 1_000);
            assert_eq!(board.memory_stats().score_slots, 0);
        }
    }
}

pub(crate) struct Leaderboard {
    pub(super) players: Arc<PlayerRegistry>,
    pub(super) state: Arc<Mutex<BoardState>>,
    pub(super) data: Arc<RwLock<LeaderboardInner>>,
    tx: Sender<()>,
}

pub(super) struct BoardMemory {
    pub active_players: usize,
    pub score_slots: usize,
    pub buffer_bytes: usize,
    pub pending_changes: usize,
    pub pending_capacity: usize,
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Leaderboard {
    pub(super) fn reserve_players(&self, count: usize) {
        {
            let mut state = self.state.lock();
            state.scores.reserve_players(count);
            // Pending maps are swapped through the shared publisher and large
            // batches are trimmed there. Avoid full-player maps on every board.
            let pending_count = count.min(MAX_RETAINED_PENDING_CAPACITY);
            let additional = pending_count.saturating_sub(state.pending.len());
            state.pending.reserve(additional);
        }
        self.data.write().reserve_players(count);
    }

    pub(super) fn memory_stats(&self) -> BoardMemory {
        let mut stats = {
            let state = self.state.lock();
            BoardMemory {
                active_players: state.scores.len,
                score_slots: state.scores.slots(),
                buffer_bytes: state.scores.buffer_bytes(),
                pending_changes: state.pending.len(),
                pending_capacity: state.pending.capacity(),
            }
        };
        let data = self.data.read();
        stats.buffer_bytes += data.scores.buffer_bytes()
            + (data.sorted_players.capacity()
                + data.scratch.capacity()
                + data.updated_players.capacity()
                + data.ranks.capacity())
                * std::mem::size_of::<u32>();
        stats
    }
    pub fn new() -> Self {
        Self::with_players(Arc::new(PlayerRegistry::default()), ScoreWidth::Wide)
    }

    pub(super) fn paused(players: Arc<PlayerRegistry>, width: ScoreWidth) -> (Self, Receiver<()>) {
        let (tx, rx) = bounded(1);
        (Self::build(players, width, tx), rx)
    }

    fn build(players: Arc<PlayerRegistry>, width: ScoreWidth, tx: Sender<()>) -> Self {
        let count = players.capacity_hint.load(AtomicOrdering::Relaxed);
        let board = Self {
            players,
            state: Arc::new(Mutex::new(BoardState {
                scores: Scores::new(width),
                pending: HashMap::new(),
            })),
            data: Arc::new(RwLock::new(LeaderboardInner::new(width))),
            tx,
        };
        board.reserve_players(count);
        board
    }

    #[cfg(test)]
    pub(super) fn with_worker(
        players: Arc<PlayerRegistry>,
        width: ScoreWidth,
        worker: &RankingWorker,
    ) -> Self {
        Self::with_worker_and_buckets(players, width, worker, BucketLayout::default())
    }

    pub(super) fn with_worker_and_buckets(
        players: Arc<PlayerRegistry>,
        width: ScoreWidth,
        worker: &RankingWorker,
        buckets: BucketLayout,
    ) -> Self {
        let board = Self::build(players, width, worker.wakeup.clone());
        board.data.write().buckets = buckets;
        worker.register(&board);
        board
    }

    pub(super) fn with_players(players: Arc<PlayerRegistry>, width: ScoreWidth) -> Self {
        let (board, rx) = Self::paused(players, width);
        let data = Arc::clone(&board.data);
        let state = Arc::clone(&board.state);
        let players = Arc::clone(&board.players);
        std::thread::spawn(move || Self::worker_loop(data, state, players, rx));
        board
    }

    pub(super) fn worker_loop(
        data: Arc<RwLock<LeaderboardInner>>,
        state: Arc<Mutex<BoardState>>,
        players: Arc<PlayerRegistry>,
        rx: Receiver<()>,
    ) {
        let mut last_refresh: Option<Instant> = None;
        let mut changes = HashMap::new();
        while rx.recv().is_ok() {
            if let Some(last_refresh) = last_refresh
                && let Some(wait) = RANK_REFRESH_INTERVAL.checked_sub(last_refresh.elapsed())
            {
                std::thread::sleep(wait);
            }
            std::mem::swap(&mut changes, &mut state.lock().pending);
            if !changes.is_empty() {
                Self::apply_reused(&data, &players, &mut changes);
                // Recycle normal batches rather than allocating/freeing a new
                // map across producer/worker threads every refresh. Do not keep
                // an unusually large startup map indefinitely.
                if changes.capacity() > MAX_RETAINED_PENDING_CAPACITY {
                    changes.shrink_to(4_096);
                }
                last_refresh = Some(Instant::now());
            }
        }
    }

    // Deterministic tests and standalone benchmarks publish owned batches.
    #[allow(dead_code)]
    pub(super) fn apply_changes(
        data: &RwLock<LeaderboardInner>,
        players: &PlayerRegistry,
        mut changes: HashMap<u32, LeaderboardOp>,
    ) {
        Self::apply_reused(data, players, &mut changes);
    }

    fn apply_reused(
        data: &RwLock<LeaderboardInner>,
        players: &PlayerRegistry,
        changes: &mut HashMap<u32, LeaderboardOp>,
    ) {
        if !changes.is_empty() {
            // No producer holds the registry lock while locking board state.
            let mut data = data.write();
            let players = players.inner.read();
            data.apply(&players, changes);
        }
    }

    pub(crate) fn update(&self, user_id: i64, new_xp: i64) {
        let index = self.players.register(user_id);
        self.update_index(index, new_xp);
    }

    pub(super) fn resolve_player(&self, user_id: i64) -> ResolvedPlayer<'_> {
        ResolvedPlayer {
            registry: self.players.as_ref(),
            index: self.players.register(user_id),
        }
    }

    pub(super) fn update_resolved(&self, player: &ResolvedPlayer<'_>, new_xp: i64) {
        self.update_index(player.index_for(self), new_xp);
    }

    fn update_index(&self, index: u32, new_xp: i64) {
        let mut state = self.state.lock();
        let old = state.scores.get(index);
        if old == Some(new_xp) {
            return;
        }
        state.scores.set(index, Some(new_xp));
        Self::queue_change(&mut state, index, old, Some(new_xp));
        drop(state);
        let _ = self.tx.try_send(());
    }

    fn queue_change(
        state: &mut BoardState,
        index: u32,
        previous: Option<i64>,
        latest: Option<i64>,
    ) {
        let previous = state
            .pending
            .get(&index)
            .copied()
            .map(LeaderboardOp::previous_score)
            .unwrap_or(previous);
        if previous == latest {
            state.pending.remove(&index);
        } else {
            let op = match (previous, latest) {
                (None, Some(new_xp)) => LeaderboardOp::Insert { new_xp },
                (Some(old_xp), Some(new_xp)) => LeaderboardOp::Update { old_xp, new_xp },
                (Some(xp), None) => LeaderboardOp::Remove { xp },
                (None, None) => unreachable!(),
            };
            state.pending.insert(index, op);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn has(&self, user_id: &i64) -> bool {
        self.get_value(user_id).is_some()
    }

    pub(crate) fn get_value(&self, user_id: &i64) -> Option<i64> {
        let index = self.players.get(*user_id)?;
        self.state.lock().scores.get(index)
    }

    /// Return the last published rank, not a read-your-write rank. In particular,
    /// a newly inserted player has a score immediately but no rank until the
    /// asynchronous publisher processes their insertion.
    pub(crate) fn get_rank(&self, user_id: i64) -> Option<usize> {
        let index = self.players.get(user_id)?;
        self.get_rank_index(index)
    }

    pub(super) fn get_rank_resolved(&self, player: &ResolvedPlayer<'_>) -> Option<usize> {
        self.get_rank_index(player.index_for(self))
    }

    fn get_rank_index(&self, index: u32) -> Option<usize> {
        self.state.lock().scores.get(index)?;
        let rank = self
            .data
            .read()
            .ranks
            .get(index as usize)
            .copied()
            .unwrap_or(0);
        (rank != 0).then_some(rank as usize)
    }

    #[allow(dead_code)]
    pub(crate) fn remove(&self, user_id: i64) {
        let Some(index) = self.players.get(user_id) else {
            return;
        };
        let mut state = self.state.lock();
        let Some(old) = state.scores.get(index) else {
            return;
        };
        state.scores.set(index, None);
        Self::queue_change(&mut state, index, Some(old), None);
        drop(state);
        let _ = self.tx.try_send(());
    }

    pub(crate) fn get_range(&self, offset: usize, limit: usize) -> Vec<LeaderboardEntry> {
        let data = self.data.read();
        let players = self.players.inner.read();
        data.sorted_players
            .iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(position, &index)| LeaderboardEntry {
                rank: offset + position + 1,
                user_id: players.ids[index as usize],
                xp: data.scores.get(index).expect("ranked player has a score"),
            })
            .collect()
    }

    pub fn bucket_distribution(&self) -> Vec<BucketDistribution> {
        let data = self.data.read();
        data.bucket_counts
            .iter()
            .map(|(&bucket, &count)| {
                let (min_xp, max_xp) = data.buckets.bucket_bounds(bucket);
                BucketDistribution {
                    bucket,
                    min_xp,
                    max_xp,
                    count,
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn score_entries(&self) -> Vec<(i64, i64)> {
        let state = self.state.lock();
        let players = self.players.inner.read();
        (0..state.scores.slots())
            .filter_map(|index| {
                state
                    .scores
                    .get(index as u32)
                    .map(|xp| (players.ids[index], xp))
            })
            .collect()
    }
}
