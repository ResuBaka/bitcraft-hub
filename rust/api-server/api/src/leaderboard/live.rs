//! Versioned selected-board updates. Snapshot-only topics remain available
//! for older clients.
use super::{LeaderboardEntry, RankingSystem, experience_to_level};
use crate::AppState;
use crate::websocket::{BroadcastTopics, OutboundWebSocketMessages};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use axum_codec::Codec;
use serde::{Deserialize, Serialize};
use service::Query;
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::time::{Duration, MissedTickBehavior};
use ts_rs::TS;

const BUILT_INS: [(i32, &str); 7] = [
    (-1, "Professions"),
    (-2, "Skills"),
    (-3, "Experience"),
    (-4, "Level"),
    (-5, "Experience Per Hour"),
    (-6, "Time Played"),
    (-7, "Time Online"),
];

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardBoard {
    id: i32,
    name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardBoardRow {
    // Use a decimal string so every player ID is exact in the browser.
    player_id: String,
    player_name: Option<String>,
    signed_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    experience: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    time_played: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardBoardUpdate {
    pub(crate) board_id: i32,
    epoch: String,
    revision: u32,
    rows: Vec<LeaderboardBoardRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardBoardDelta {
    pub(crate) board_id: i32,
    epoch: String,
    base_revision: u32,
    revision: u32,
    upserts: Vec<LeaderboardBoardRow>,
    // Indexes into the old order followed by new IDs from `upserts`.
    // Besides rank changes, an omitted index identifies a removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    order: Option<Vec<u8>>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct LeaderboardBoardSnapshot {
    board_id: i32,
    rows: Vec<LeaderboardBoardRow>,
}

pub(crate) fn topic(id: i32) -> BroadcastTopics {
    BroadcastTopics::new([("leaderboard_board".into(), Some(i64::from(id)))])
}

pub(crate) const DELTA_TOPIC: &str = "leaderboard_delta";

pub(crate) fn delta_topic(id: i32) -> BroadcastTopics {
    BroadcastTopics::new([(DELTA_TOPIC.into(), Some(i64::from(id)))])
}

fn boards(state: &AppState) -> Vec<LeaderboardBoard> {
    let mut skills: Vec<_> = state
        .skill_desc
        .iter()
        .filter_map(|skill| {
            let id = i32::try_from(skill.id).ok()?;
            (id >= 0 && skill.skill_category != 0).then(|| LeaderboardBoard {
                id,
                name: skill.name.clone(),
            })
        })
        .collect();
    skills.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    BUILT_INS
        .into_iter()
        .map(|(id, name)| LeaderboardBoard {
            id,
            name: name.into(),
        })
        .chain(skills)
        .collect()
}

pub(crate) async fn list_boards(State(state): State<AppState>) -> Codec<Vec<LeaderboardBoard>> {
    Codec(boards(&state))
}

pub(crate) async fn get_board(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Codec<LeaderboardBoardSnapshot>, (StatusCode, &'static str)> {
    if !boards(&state).iter().any(|board| board.id == id) {
        return Err((StatusCode::NOT_FOUND, "Unknown leaderboard board"));
    }
    let mut result = load_boards(&state, [id]).await.map_err(|error| {
        tracing::warn!(?error, "Failed to load leaderboard board");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to load leaderboard board",
        )
    })?;
    Ok(Codec(LeaderboardBoardSnapshot {
        board_id: id,
        rows: result.remove(&id).unwrap_or_default(),
    }))
}

fn entries(ranking: &RankingSystem, id: i32) -> Vec<LeaderboardEntry> {
    let board = match id {
        -1 => &ranking.profession_leaderboard,
        -2 => &ranking.adventure_leaderboard,
        -3 => &ranking.global_leaderboard,
        -4 => &ranking.level_leaderboard,
        -5 => &ranking.xp_per_hour,
        -6 => &ranking.time_played,
        -7 => &ranking.time_signed_in,
        _ => {
            return ranking
                .skill_leaderboard(i64::from(id))
                .map(|board| board.get_range(0, 100))
                .unwrap_or_default();
        }
    };
    board.get_range(0, 100)
}

fn row(
    id: i32,
    entry: LeaderboardEntry,
    player_name: Option<String>,
    signed_in: bool,
) -> LeaderboardBoardRow {
    let (experience, level, time_played) = match id {
        -4 => (None, Some(entry.xp as i32), None),
        -7 | -6 => (None, None, Some(entry.xp)),
        0.. => (Some(entry.xp), Some(experience_to_level(entry.xp)), None),
        _ => (Some(entry.xp), None, None),
    };
    LeaderboardBoardRow {
        player_id: entry.user_id.to_string(),
        player_name,
        signed_in,
        experience,
        level,
        time_played,
    }
}

async fn load_boards(
    state: &AppState,
    ids: impl IntoIterator<Item = i32>,
) -> Result<HashMap<i32, Vec<LeaderboardBoardRow>>, sea_orm::DbErr> {
    let entries_by_board: Vec<_> = ids
        .into_iter()
        .map(|id| (id, entries(&state.ranking_system, id)))
        .collect();
    let player_ids: HashSet<_> = entries_by_board
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(|entry| entry.user_id))
        .collect();
    let names: HashMap<_, _> = if player_ids.is_empty() {
        HashMap::new()
    } else {
        Query::find_player_username_by_ids(&state.conn, player_ids.into_iter().collect())
            .await?
            .into_iter()
            .map(|player| (player.entity_id, player.username))
            .collect()
    };
    Ok(entries_by_board
        .into_iter()
        .map(|(id, entries)| {
            let rows = entries
                .into_iter()
                .map(|entry| {
                    let name = names.get(&entry.user_id).cloned();
                    let signed_in = state
                        .player_state
                        .get(&entry.user_id)
                        .is_some_and(|player| player.signed_in);
                    row(id, entry, name, signed_in)
                })
                .collect();
            (id, rows)
        })
        .collect())
}

// Keep enough revisions for a short reconnect without unlimited per-board history.
const RETAINED_REVISIONS: usize = 16;

struct BoardRevision {
    revision: u32,
    rows: Vec<LeaderboardBoardRow>,
}

#[derive(Default)]
struct PublishedBoard {
    history: VecDeque<BoardRevision>,
}

impl PublishedBoard {
    fn update(&mut self, rows: Vec<LeaderboardBoardRow>) -> Option<u32> {
        let previous = self.history.back();
        if previous.is_some_and(|latest| latest.rows == rows) {
            return None;
        }
        let revision = previous.map_or(1, |latest| latest.revision + 1);
        if self.history.len() == RETAINED_REVISIONS {
            self.history.pop_front();
        }
        self.history.push_back(BoardRevision { revision, rows });
        Some(revision)
    }

    fn snapshot(board_id: i32, epoch: &str, latest: &BoardRevision) -> LeaderboardBoardUpdate {
        LeaderboardBoardUpdate {
            board_id,
            epoch: epoch.into(),
            revision: latest.revision,
            rows: latest.rows.clone(),
        }
    }

    fn frame(
        &self,
        board_id: i32,
        epoch: &str,
        base_revision: Option<u32>,
    ) -> Option<OutboundWebSocketMessages> {
        let latest = self.history.back()?;
        if base_revision == Some(latest.revision) {
            return None;
        }
        let snapshot = OutboundWebSocketMessages::LeaderboardBoardReset(Self::snapshot(
            board_id, epoch, latest,
        ));
        let Some(base) = self
            .history
            .iter()
            .find(|base| Some(base.revision) == base_revision)
        else {
            return Some(snapshot);
        };

        let previous: HashMap<_, _> = base
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.player_id.as_str(), (index, row)))
            .collect();
        let upserts = latest
            .rows
            .iter()
            .filter(|row| previous.get(row.player_id.as_str()).map(|(_, old)| *old) != Some(*row))
            .cloned()
            .collect();
        let same_order = base
            .rows
            .iter()
            .map(|row| &row.player_id)
            .eq(latest.rows.iter().map(|row| &row.player_id));
        let order = (!same_order).then(|| {
            let mut next_new_index = base.rows.len();
            latest
                .rows
                .iter()
                .map(|row| {
                    let index = match previous.get(row.player_id.as_str()) {
                        Some((index, _)) => *index,
                        None => {
                            let index = next_new_index;
                            next_new_index += 1;
                            index
                        }
                    };
                    u8::try_from(index).expect("top 100 ranks fit in a byte")
                })
                .collect()
        });
        let delta = OutboundWebSocketMessages::LeaderboardBoardDelta(LeaderboardBoardDelta {
            board_id,
            epoch: epoch.into(),
            base_revision: base.revision,
            revision: latest.revision,
            upserts,
            order,
        });

        // Don't send a larger update than a full snapshot, in either supported encoding.
        let msgpack_len =
            |message: &OutboundWebSocketMessages| Codec(message).to_msgpack().unwrap().len();
        let json_len =
            |message: &OutboundWebSocketMessages| serde_json::to_vec(message).unwrap().len();
        if msgpack_len(&delta) < msgpack_len(&snapshot) && json_len(&delta) < json_len(&snapshot) {
            Some(delta)
        } else {
            Some(snapshot)
        }
    }
}

pub(crate) struct LiveBoards {
    epoch: String,
    published: dashmap::DashMap<i32, PublishedBoard>,
}

impl Default for LiveBoards {
    fn default() -> Self {
        Self {
            epoch: nanoid::nanoid!(),
            published: Default::default(),
        }
    }
}

impl LiveBoards {
    pub(crate) fn sync(
        &self,
        board_id: i32,
        epoch: Option<&str>,
        revision: Option<u32>,
    ) -> Option<OutboundWebSocketMessages> {
        let base = if epoch == Some(self.epoch.as_str()) {
            revision
        } else {
            None
        };
        self.published
            .get(&board_id)?
            .frame(board_id, &self.epoch, base)
    }

    fn publish(
        &self,
        board_id: i32,
        rows: Vec<LeaderboardBoardRow>,
        snapshots: bool,
    ) -> [Option<OutboundWebSocketMessages>; 2] {
        let mut board = self.published.entry(board_id).or_default();
        let Some(revision) = board.update(rows) else {
            return [None, None];
        };
        [
            snapshots.then(|| {
                OutboundWebSocketMessages::LeaderboardBoardUpdated(PublishedBoard::snapshot(
                    board_id,
                    &self.epoch,
                    board.history.back().unwrap(),
                ))
            }),
            board.frame(board_id, &self.epoch, revision.checked_sub(1)),
        ]
    }
}

pub(super) fn start(state: AppState) {
    let shutdown = state.shutdown.clone();
    shutdown.spawn_worker(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = state.shutdown.stop.cancelled() => break,
                _ = tick.tick() => {}
            }
            let catalog = boards(&state);
            let interested: Vec<_> = catalog
                .into_iter()
                .map(|board| board.id)
                .filter(|id| {
                    state.tx.is_interested(&topic(*id)) || state.tx.is_interested(&delta_topic(*id))
                })
                .collect();
            if interested.is_empty() {
                continue;
            }
            // A periodic tick also catches visible username/sign-in changes,
            // which needn't trigger a ranking publication.
            match load_boards(&state, interested).await {
                Ok(boards) => {
                    for (board_id, rows) in boards {
                        let snapshots = state.tx.is_interested(&topic(board_id));
                        // Build the versioned frame before branching on current
                        // interest; a client may subscribe as the board is cached.
                        for frame in state
                            .live_leaderboards
                            .publish(board_id, rows, snapshots)
                            .into_iter()
                            .flatten()
                        {
                            let _ = state.tx.send(frame);
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "Failed to publish selected leaderboard boards")
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_visible_board_changes_advance_its_revision() {
        let mut first = PublishedBoard::default();
        let mut second = PublishedBoard::default();
        let entry = LeaderboardEntry {
            rank: 1,
            user_id: 9_007_199_254_740_993,
            xp: 520,
        };
        let row = row(42, entry, Some("Miner".into()), false);
        assert_eq!(row.player_id, "9007199254740993");
        assert_eq!(row.level, Some(2));
        assert_eq!(first.update(vec![row.clone()]), Some(1));
        assert_eq!(first.update(vec![row.clone()]), None);
        let mut signed_in = row;
        signed_in.signed_in = true;
        assert_eq!(first.update(vec![signed_in]), Some(2));
        assert_eq!(first.update(vec![]), Some(3));
        assert_eq!(first.update(vec![]), None);
        assert_eq!(second.update(vec![]), Some(1));
    }

    fn sample_rows() -> Vec<LeaderboardBoardRow> {
        (1..=100)
            .map(|id| {
                row(
                    42,
                    LeaderboardEntry {
                        rank: id as usize,
                        user_id: id,
                        xp: 1_500,
                    },
                    Some(format!("Miner {id}")),
                    false,
                )
            })
            .collect()
    }

    #[test]
    fn one_xp_update_sends_one_changed_row_without_order() {
        let live = LiveBoards::default();
        let mut rows = sample_rows();
        let [
            None,
            Some(OutboundWebSocketMessages::LeaderboardBoardReset(initial)),
        ] = live.publish(42, rows.clone(), false)
        else {
            panic!("expected initial snapshot")
        };
        rows[1].experience = Some(1_501);
        let [
            None,
            Some(frame @ OutboundWebSocketMessages::LeaderboardBoardDelta(_)),
        ] = live.publish(42, rows.clone(), false)
        else {
            panic!("expected delta")
        };
        assert!(Codec(&frame).to_msgpack().unwrap().len() < 250);
        let OutboundWebSocketMessages::LeaderboardBoardDelta(delta) = frame else {
            unreachable!()
        };
        assert_eq!(delta.epoch, initial.epoch);
        assert_eq!((delta.base_revision, delta.revision), (1, 2));
        assert_eq!(delta.upserts, rows[1..2]);
        assert_eq!(delta.order, None);
        assert!(
            live.publish(42, rows, false)
                .into_iter()
                .all(|frame| frame.is_none())
        );
    }

    #[test]
    fn resume_merges_changes_and_resets_when_history_is_missing() {
        let live = LiveBoards::default();
        let mut rows = sample_rows();
        live.publish(42, rows.clone(), false);
        rows[1].experience = Some(1_501);
        live.publish(42, rows.clone(), false);
        rows.pop();
        rows.swap(0, 1);
        let new_row = row(
            42,
            LeaderboardEntry {
                rank: 3,
                user_id: 101,
                xp: 1_500,
            },
            Some("New miner".into()),
            false,
        );
        rows.insert(2, new_row.clone());
        live.publish(42, rows.clone(), false);

        let Some(OutboundWebSocketMessages::LeaderboardBoardDelta(delta)) =
            live.sync(42, Some(&live.epoch), Some(1))
        else {
            panic!("expected merged delta")
        };
        assert_eq!((delta.base_revision, delta.revision), (1, 3));
        assert_eq!(delta.upserts, [rows[0].clone(), new_row]);
        let expected_order = [1, 0, 100].into_iter().chain(2..99).collect::<Vec<_>>();
        assert_eq!(delta.order.unwrap(), expected_order);
        assert!(live.sync(42, Some(&live.epoch), Some(3)).is_none());
        for old_epoch in [None, Some("before-restart")] {
            assert!(matches!(
                live.sync(42, old_epoch, Some(3)),
                Some(OutboundWebSocketMessages::LeaderboardBoardReset(_))
            ));
        }
        for i in 0..RETAINED_REVISIONS {
            rows[0].experience = Some(2_000 + i as i64);
            live.publish(42, rows.clone(), false);
        }
        assert!(matches!(live.sync(42, Some(&live.epoch), Some(1)),
            Some(OutboundWebSocketMessages::LeaderboardBoardReset(snapshot)) if snapshot.rows == rows));
    }

    #[test]
    fn a_board_wide_change_prefers_a_smaller_snapshot() {
        let live = LiveBoards::default();
        let mut rows = sample_rows();
        live.publish(42, rows.clone(), false);
        for row in &mut rows {
            row.experience = Some(2000);
        }
        let [
            _,
            Some(OutboundWebSocketMessages::LeaderboardBoardReset(snapshot)),
        ] = live.publish(42, rows.clone(), false)
        else {
            panic!("expected smaller snapshot")
        };
        assert_eq!(snapshot.rows, rows);
    }

    #[tokio::test]
    async fn snapshots_only_reach_subscribers_to_the_selected_board() {
        let clients = crate::ClientsState::new();
        let (selected_tx, selected_rx) = kanal::bounded_async(2);
        let (other_tx, other_rx) = kanal::bounded_async(2);
        for (client, tx, id) in [("selected", selected_tx, -1), ("other", other_tx, 42)] {
            let client = client.to_owned();
            clients
                .add_client(client.clone(), tx, crate::WebsocketEncoding::MessagePack)
                .await;
            clients
                .add_topic_to_client(&client, "leaderboard_board", Some(id))
                .await;
        }
        let message = OutboundWebSocketMessages::LeaderboardBoardUpdated(LeaderboardBoardUpdate {
            board_id: -1,
            epoch: "test".into(),
            revision: 1,
            rows: Vec::new(),
        });
        clients
            .broadcast_topics(message.topics().unwrap(), &message)
            .await;
        assert!(selected_rx.try_recv().unwrap().is_some());
        assert!(other_rx.try_recv().unwrap().is_none());
    }
}
