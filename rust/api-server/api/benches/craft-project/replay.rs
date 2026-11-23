//! Offline replay of the project listener; decoding, pairing and checks are untimed.
use super::*;
use rustix::time::{ClockId, clock_gettime};
use spacetimedb_sdk::{
    __codegen::{__ws::v2::ServerMessage, TableUpdate},
    unstable::streaming_decompression::streaming,
};
use std::hint::black_box;

enum Op<R> {
    Insert(R),
    Delete(R),
    Update(R, R),
}

// SDK callback order: unpaired inserts, unpaired deletes, then same-PK pairs.
fn ops<R>(rows: TableUpdate<R>, key: impl Fn(&R) -> u64) -> Vec<Op<R>> {
    let mut deletes = rows.deletes;
    let mut out = Vec::new();
    let mut updates = Vec::new();
    for r in rows.inserts {
        if let Some(i) = deletes.iter().position(|d| key(&d.row) == key(&r.row)) {
            updates.push(Op::Update(deletes.remove(i).row, r.row));
        } else {
            out.push(Op::Insert(r.row));
        }
    }
    out.extend(deletes.into_iter().map(|r| Op::Delete(r.row)));
    out.extend(updates);
    out
}

type Row = ProgressiveActionState;
fn apply<const NEW: bool>(ctx: &Context, op: &Op<Row>) {
    match op {
        Op::Insert(r) => ctx.project(r),
        Op::Delete(r) => {
            ctx.projects.remove(&r.entity_id);
        }
        Op::Update(old, new) if NEW => ctx.update_project(old, new),
        Op::Update(_, new) => ctx.project(new),
    }
}

#[derive(Default)]
struct Trace {
    initial: Vec<Row>,
    ops: Vec<Op<Row>>,
}

fn verified_trace() -> Trace {
    let bytes = std::fs::read(std::env::var_os("CRAFT_WIRE").expect("set CRAFT_WIRE")).unwrap();
    let mut wire = bytes.as_slice();
    let old = Context::default();
    let new = Context::default();
    let mut trace = Trace::default();
    let (mut hits, mut inserts, mut deletes, mut updates, mut changes) = (0, 0, 0, 0, 0);
    let time = Utc::now();
    while !wire.is_empty() {
        let len = u64::from_le_bytes(wire[1..9].try_into().unwrap()) as usize;
        let (frame, rest) = wire[9..].split_at(len);
        wire = rest;
        match streaming(frame).unwrap() {
            ServerMessage::SubscribeApplied(s) => {
                for t in s.rows.tables {
                    match &t.table[..] {
                        "progressive_action_state" => {
                            assert!(trace.ops.is_empty() && trace.initial.is_empty());
                            let mut rows = TableUpdate::<Row>::default();
                            rows.append_initial_rows(t.rows).unwrap();
                            trace.initial = rows.inserts.into_iter().map(|r| r.row).collect();
                            for ctx in [&old, &new] {
                                ctx.reset_projects(&trace.initial);
                            }
                        }
                        "building_state" => {
                            let mut rows = TableUpdate::<BuildingState>::default();
                            rows.append_initial_rows(t.rows).unwrap();
                            for ctx in [&old, &new] {
                                ctx.buildings.clear();
                                for r in &rows.inserts {
                                    ctx.building(&r.row);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ServerMessage::TransactionUpdate(tx) => {
                let mut projects = TableUpdate::<Row>::default();
                let mut buildings = TableUpdate::<BuildingState>::default();
                let mut events = TableUpdate::<CraftEvent>::default();
                for t in tx.query_sets.into_iter().flat_map(|q| q.tables) {
                    match &t.table_name[..] {
                        "progressive_action_state" => {
                            projects.append(TableUpdate::parse_table_update(t).unwrap())
                        }
                        "building_state" => {
                            buildings.append(TableUpdate::parse_table_update(t).unwrap())
                        }
                        "craft_event" => events.append(TableUpdate::parse_table_update(t).unwrap()),
                        _ => {}
                    }
                }
                // Generated bindings dispatch these tables in this order.
                for op in ops(buildings, |r| r.entity_id) {
                    for ctx in [&old, &new] {
                        match &op {
                            Op::Insert(r) | Op::Update(_, r) => ctx.building(r),
                            Op::Delete(r) => {
                                ctx.buildings.remove(&r.entity_id);
                            }
                        }
                    }
                }
                for e in events.inserts {
                    assert!(hit(&e.row, 14, time, &old) == hit(&e.row, 14, time, &new));
                    hits += 1;
                }
                for op in ops(projects, |r| r.entity_id) {
                    let key = match &op {
                        Op::Insert(r) => {
                            inserts += 1;
                            r.entity_id
                        }
                        Op::Delete(r) => {
                            deletes += 1;
                            r.entity_id
                        }
                        Op::Update(a, b) => {
                            // If this fails, skipping a write based only on the input is unsafe.
                            assert!(
                                new.projects.get(&a.entity_id).as_deref()
                                    == Some(&Project::from(a))
                            );
                            changes += usize::from(Project::from(a) != Project::from(b));
                            updates += 1;
                            b.entity_id
                        }
                    };
                    apply::<false>(&old, &op);
                    apply::<true>(&new, &op);
                    assert!(old.projects.get(&key).as_deref() == new.projects.get(&key).as_deref());
                    trace.ops.push(op);
                }
            }
            _ => {}
        }
    }
    assert!(hits > 0 && updates > 0 && !trace.initial.is_empty());
    println!(
        "snapshot={} inserts={inserts} deletes={deletes} updates={updates} changed_projection={changes} verified_hits={hits}",
        trace.initial.len(),
    );
    trace
}

fn cpu_ns() -> u64 {
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

fn sample<const NEW: bool>(trace: &Trace, passes: usize) -> f64 {
    let mut elapsed = 0;
    for _ in 0..passes {
        let ctx = Context::default();
        ctx.reset_projects(&trace.initial);
        let start = cpu_ns();
        for op in &trace.ops {
            apply::<NEW>(&ctx, black_box(op));
        }
        black_box(&ctx);
        elapsed += cpu_ns() - start;
    }
    elapsed as f64 / (trace.ops.len() * passes) as f64
}

#[test]
#[ignore = "offline A/B on private region-14 wire"]
fn captured_updates() {
    let trace = verified_trace();
    let passes = 1_000_000usize.div_ceil(trace.ops.len());
    sample::<false>(&trace, 1);
    sample::<true>(&trace, 1);
    for pair in 0..9 {
        let (old, new) = if pair % 2 == 0 {
            (
                sample::<false>(&trace, passes),
                sample::<true>(&trace, passes),
            )
        } else {
            let new = sample::<true>(&trace, passes);
            (sample::<false>(&trace, passes), new)
        };
        println!("pair={pair} passes={passes} unconditional_ns_op={old:.3} guarded_ns_op={new:.3}");
    }
}
