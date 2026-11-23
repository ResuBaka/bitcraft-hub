//! Private same-wire A/B; decode and callback reconstruction are outside timing.
use super::*;
use parking_lot::RwLock;
use rustix::time::{ClockId, clock_gettime};
use spacetimedb_sdk::{
    __codegen::{__ws::v2::ServerMessage, TableUpdate},
    unstable::streaming_decompression::streaming,
};
use std::{collections::HashSet, hint::black_box};

const REGION: Region = 14;
type Row = PlayerActionState;

enum Op {
    Put(Row),
    Remove(u64, u64),
}
#[derive(Default)]
struct Trace {
    initial: Vec<Row>,
    ops: Vec<Op>,
    transactions: usize,
    updates: usize,
    inserts: usize,
    deletes: usize,
    moves: usize,
}

fn load() -> Trace {
    let bytes =
        std::fs::read(std::env::var_os("ACTIVITY_WIRE").expect("set ACTIVITY_WIRE")).unwrap();
    let mut wire = bytes.as_slice();
    let mut trace = Trace::default();
    while !wire.is_empty() {
        let len = u64::from_le_bytes(wire[1..9].try_into().unwrap()) as usize;
        let (frame, rest) = wire[9..].split_at(len);
        wire = rest;
        match streaming(frame).unwrap() {
            ServerMessage::SubscribeApplied(s) => {
                for t in s.rows.tables {
                    if &t.table[..] == "player_action_state" {
                        assert!(trace.ops.is_empty(), "initial rows after transactions");
                        let mut rows = TableUpdate::<Row>::default();
                        rows.append_initial_rows(t.rows).unwrap();
                        trace
                            .initial
                            .extend(rows.inserts.into_iter().map(|r| r.row));
                    }
                }
            }
            ServerMessage::TransactionUpdate(tx) => {
                let mut rows = TableUpdate::<Row>::default();
                for t in tx.query_sets.into_iter().flat_map(|q| q.tables) {
                    if &t.table_name[..] == "player_action_state" {
                        rows.append(TableUpdate::parse_table_update(t).unwrap());
                    }
                }
                if !rows.has_changes() {
                    continue;
                }
                trace.transactions += 1;
                // SDK dispatch: unmatched inserts, unmatched deletes, then updates.
                // Pair each insert with the first remaining delete of the same PK.
                let mut updates = Vec::new();
                for r in rows.inserts {
                    let row = r.row;
                    if let Some(i) = rows
                        .deletes
                        .iter()
                        .position(|d| d.row.auto_id == row.auto_id)
                    {
                        updates.push((rows.deletes.remove(i).row, row));
                    } else {
                        trace.inserts += 1;
                        trace.ops.push(Op::Put(row));
                    }
                }
                for d in rows.deletes {
                    trace.deletes += 1;
                    trace.ops.push(Op::Remove(d.row.entity_id, d.row.auto_id));
                }
                for (old, row) in updates {
                    trace.updates += 1;
                    if old.entity_id != row.entity_id {
                        trace.moves += 1;
                        trace.ops.push(Op::Remove(old.entity_id, old.auto_id));
                    }
                    trace.ops.push(Op::Put(row));
                }
            }
            _ => {}
        }
    }
    assert!(!trace.initial.is_empty() && !trace.ops.is_empty());
    trace
}

trait Index: Default {
    fn start(&self);
    fn put(&self, region: Region, row: Row);
    fn remove(&self, region: Region, player: u64, id: u64);

    fn seed(rows: &[Row]) -> Self {
        let index = Self::default();
        index.start();
        for row in rows {
            index.put(REGION, row.clone());
        }
        index
    }
    fn apply(&self, op: &Op) {
        match op {
            Op::Put(row) => self.put(REGION, row.clone()),
            Op::Remove(player, id) => self.remove(REGION, *player, *id),
        }
    }
}

// The nested map and lock from the pre-change PlayerActivity (e75ebb23).
#[derive(Default)]
struct OldRegion {
    _ready: bool,
    players: HashMap<u64, HashMap<u64, Row>>,
}
#[derive(Default)]
struct Old(RwLock<HashMap<Region, OldRegion>>);

impl Index for Old {
    fn start(&self) {
        self.0.write().insert(REGION, OldRegion::default());
    }
    fn put(&self, region: Region, row: Row) {
        if let Some(s) = self.0.write().get_mut(&region) {
            s.players
                .entry(row.entity_id)
                .or_default()
                .insert(row.auto_id, row);
        }
    }
    fn remove(&self, region: Region, player: u64, id: u64) {
        if let Some(s) = self.0.write().get_mut(&region)
            && let Some(rows) = s.players.get_mut(&player)
        {
            rows.remove(&id);
            if rows.is_empty() {
                s.players.remove(&player);
            }
        }
    }
}
impl Index for PlayerActivity {
    fn start(&self) {
        self.reset(REGION);
    }
    fn put(&self, region: Region, row: Row) {
        self.insert(region, row);
    }
    fn remove(&self, region: Region, player: u64, id: u64) {
        PlayerActivity::remove(self, region, player, id);
    }
}

fn verify(trace: &Trace) {
    let old = Old::seed(&trace.initial);
    let new = PlayerActivity::seed(&trace.initial);
    let check = |player| {
        let a = old.0.read();
        let b = new.regions.read();
        let a = a[&REGION].players.get(&player);
        let b = b[&REGION].players.get(&player);
        // Include every field; avoid printing private rows on mismatch.
        assert!(match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.len() == b.len() && a.values().all(|r| b.contains(r)),
            _ => false,
        });
    };
    let players: HashSet<_> = trace.initial.iter().map(|r| r.entity_id).collect();
    let two = old.0.read()[&REGION]
        .players
        .values()
        .filter(|rows| rows.len() == 2)
        .count();
    for &p in &players {
        check(p);
    }
    let mut active = HashSet::new();
    for op in &trace.ops {
        old.apply(op);
        new.apply(op);
        let p = match op {
            Op::Put(r) => r.entity_id,
            Op::Remove(p, _) => *p,
        };
        check(p);
        active.insert(p);
    }
    let final_players: Vec<_> = old.0.read()[&REGION].players.keys().copied().collect();
    assert_eq!(
        final_players.len(),
        new.regions.read()[&REGION].players.len()
    );
    for p in final_players {
        check(p);
    }
    println!(
        "snapshot_rows={} players={} players_with_two={} touched_players={} transactions={} inserts={} deletes={} updates={} owner_changes={}",
        trace.initial.len(),
        players.len(),
        two,
        active.len(),
        trace.transactions,
        trace.inserts,
        trace.deletes,
        trace.updates,
        trace.moves
    );
}

fn cpu_ns() -> u64 {
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}
fn sample<I: Index>(trace: &Trace, passes: usize) -> f64 {
    let mut elapsed = 0;
    for _ in 0..passes {
        let index = I::seed(&trace.initial);
        let start = cpu_ns();
        for op in &trace.ops {
            index.apply(op);
        }
        black_box(&index);
        elapsed += cpu_ns() - start;
    }
    elapsed as f64 / (passes * trace.ops.len()) as f64
}

#[test]
#[ignore = "offline A/B on private region-14 wire"]
fn captured_updates() {
    let trace = load();
    verify(&trace);
    let passes = 1_000_000usize.div_ceil(trace.ops.len());
    sample::<Old>(&trace, 1);
    sample::<PlayerActivity>(&trace, 1);
    for pair in 0..9 {
        let (old, new) = if pair % 2 == 0 {
            (
                sample::<Old>(&trace, passes),
                sample::<PlayerActivity>(&trace, passes),
            )
        } else {
            let new = sample::<PlayerActivity>(&trace, passes);
            (sample::<Old>(&trace, passes), new)
        };
        println!("pair={pair} passes={passes} map_ns_per_op={old:.3} vec_ns_per_op={new:.3}");
    }
}
