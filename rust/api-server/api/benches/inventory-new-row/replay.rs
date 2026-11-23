//! The fixture stays private. Decode and pair rows once, before the callback boundary.
use super::*;
use rustix::time::{ClockId, clock_gettime};
use spacetimedb_sdk::__codegen::{self as sdk, __ws::v2::ServerMessage};
use spacetimedb_sdk::unstable::streaming_decompression::streaming;
use std::hint::black_box;

enum Change {
    Insert(InventoryState),
    Remove(InventoryState),
    Update(InventoryState, InventoryState),
}

fn load(wire: &[u8]) -> (usize, Vec<Change>) {
    let mut bytes = wire;
    let mut changes = Vec::new();
    let mut frames = 0;
    while !bytes.is_empty() {
        let n = usize::try_from(u64::from_le_bytes(bytes[1..9].try_into().unwrap())).unwrap();
        let tag = bytes[0];
        let (frame, rest) = bytes[9..].split_at(n);
        bytes = rest;
        if tag != 4 || frame[0] != 0 {
            continue;
        }
        let ServerMessage::TransactionUpdate(tx) = streaming(frame).unwrap() else {
            panic!("expected transaction")
        };
        frames += 1;
        let mut diff = sdk::TableUpdate::<InventoryState>::default();
        for table in sdk::transaction_update_iter_table_updates(tx) {
            if table.table_name.as_ref() == "inventory_state" {
                diff.append(sdk::TableUpdate::parse_table_update(table).unwrap());
            }
        }
        // Same PK pairing as the cacheless SDK: first matching delete, FIFO
        // on duplicate keys, unmatched inserts, unmatched deletes, then updates.
        let mut pairs = Vec::new();
        for insert in diff.inserts {
            if let Some(i) = diff
                .deletes
                .iter()
                .position(|d| d.row.entity_id == insert.row.entity_id)
            {
                pairs.push(Change::Update(diff.deletes.remove(i).row, insert.row));
            } else {
                changes.push(Change::Insert(insert.row));
            }
        }
        changes.extend(diff.deletes.into_iter().map(|d| Change::Remove(d.row)));
        changes.extend(pairs);
    }
    assert!(changes.iter().any(|c| matches!(c, Change::Update(..))));
    (frames, changes)
}

fn cpu_ns() -> u64 {
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

// An opt-in instrumented lib-test executable, separate from timing builds.
#[cfg(feature = "bench")]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! {
        static COUNTS: Cell<(u64,u64)> = const { Cell::new((0,0)) };
    }
    pub fn get() -> (u64, u64) {
        COUNTS.get()
    }
    struct Count;
    #[global_allocator]
    static GLOBAL: Count = Count;
    fn add(n: usize) {
        let (a, b) = COUNTS.get();
        COUNTS.set((a + 1, b + n as u64));
    }
    unsafe impl GlobalAlloc for Count {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            add(l.size());
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            add(l.size());
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            add(n);
            unsafe { System.realloc(p, l, n) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
    }
}

fn counts() -> (u64, u64) {
    #[cfg(feature = "bench")]
    {
        allocations::get()
    }
    #[cfg(not(feature = "bench"))]
    {
        (0, 0)
    }
}

trait Delivery {
    fn project(change: &Change) -> Self;
    fn consume(self) -> (u8, InventoryState, entity::shared::Region);
}

impl Delivery for SpacetimeUpdateMessages<InventoryState> {
    fn project(change: &Change) -> Self {
        match change {
            Change::Insert(r) => Self::Insert {
                new: r.clone(),
                database_name: 14,
                event: None,
            },
            Change::Remove(r) => Self::Remove {
                delete: r.clone(),
                database_name: 14,
                event: None,
            },
            Change::Update(old, new) => Self::Update {
                old: old.clone(),
                new: new.clone(),
                database_name: 14,
                event: None,
            },
        }
    }
    fn consume(self) -> (u8, InventoryState, entity::shared::Region) {
        match self {
            Self::Insert {
                new, database_name, ..
            } => (1, new, database_name),
            Self::Remove {
                delete,
                database_name,
                ..
            } => (2, delete, database_name),
            Self::Update {
                new, database_name, ..
            } => (3, new, database_name),
            Self::Initial { .. } => unreachable!(),
        }
    }
}

async fn pass<M: Delivery + Send + 'static>(
    changes: &[Change],
    repeats: usize,
    verify: bool,
) -> (u64, u64, u64) {
    let (tx, mut rx) = crate::websocket::worker_channel::unbounded_channel::<M>();
    let start = cpu_ns();
    let (calls, bytes) = counts();
    for _ in 0..repeats {
        for chunk in changes.chunks(32) {
            for c in chunk {
                tx.send(M::project(black_box(c))).unwrap();
            }
            for c in chunk {
                let (kind, row, region) = rx.recv().await.unwrap().consume();
                if verify {
                    let (expected_kind, expected) = match c {
                        Change::Insert(r) => (1, r),
                        Change::Remove(r) => (2, r),
                        Change::Update(_, r) => (3, r),
                    };
                    assert_eq!((kind, &row, region), (expected_kind, expected, 14));
                }
                drop(black_box((kind, row, region)));
            }
        }
    }
    let (end_calls, end_bytes) = counts();
    (cpu_ns() - start, end_calls - calls, end_bytes - bytes)
}

impl Delivery for InventoryMessage {
    fn project(change: &Change) -> Self {
        use crate::websocket::ListenerRow;
        match change {
            Change::Insert(r) => Self::Insert {
                new: ListenerRow::live(r),
                database_name: 14,
                event: None,
            },
            Change::Remove(r) => Self::Remove {
                delete: ListenerRow::live(r),
                database_name: 14,
                event: None,
            },
            Change::Update(old, new) => Self::Update {
                old: ListenerRow::old(old),
                new: ListenerRow::live(new),
                database_name: 14,
                event: None,
            },
        }
    }
    fn consume(self) -> (u8, InventoryState, entity::shared::Region) {
        match self {
            Self::Insert {
                new, database_name, ..
            } => (1, new.unwrap(), database_name),
            Self::Remove {
                delete,
                database_name,
                ..
            } => (2, delete.unwrap(), database_name),
            Self::Update {
                new, database_name, ..
            } => (3, new.unwrap(), database_name),
            Self::Initial { .. } => unreachable!(),
        }
    }
}

#[tokio::test]
#[ignore = "private inventory callback-to-worker replay, pin CPU affinity"]
async fn inventory_new_row_delivery() {
    type Before = SpacetimeUpdateMessages<InventoryState>;
    type After = InventoryMessage;
    let wire = std::fs::read(std::env::var_os("INVENTORY_WIRE").unwrap()).unwrap();
    let (frames, changes) = load(&wire);
    drop(wire);
    let mut kinds = [0; 3];
    let mut old_pockets = 0;
    for c in &changes {
        kinds[match c {
            Change::Insert(_) => 0,
            Change::Remove(_) => 1,
            Change::Update(old, _) => {
                old_pockets += old.pockets.len();
                2
            }
        }] += 1;
    }
    println!(
        "frames={frames} inserts={} deletes={} updates={} old_pockets={old_pockets} baseline_bytes={} candidate_bytes={}",
        kinds[0],
        kinds[1],
        kinds[2],
        size_of::<Before>(),
        size_of::<After>()
    );
    pass::<Before>(&changes, 1, true).await;
    pass::<After>(&changes, 1, true).await;
    let repeats = (250_000usize).div_ceil(changes.len());
    let pairs = if cfg!(feature = "bench") { 2 } else { 12 };
    for i in 0..pairs {
        for new in [i % 2 != 0, i % 2 == 0] {
            let (ns, calls, bytes) = if new {
                pass::<After>(&changes, repeats, false).await
            } else {
                pass::<Before>(&changes, repeats, false).await
            };
            println!(
                "pair={i} candidate={new} ns={ns} calls={calls} bytes={bytes} repeats={repeats} rows={}",
                changes.len()
            );
        }
    }
}
