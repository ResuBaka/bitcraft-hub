//! Offline region-14 replay. The input and row data stay private.
use super::*;
use crate::claims::claim_location::{ClaimAtLocation, mobile_tile};
use game_module::module_bindings::{
    ClaimTileState, DimensionDescriptionState, DimensionNetworkState, LocationState,
    MobileEntityState, PlayerState, SkillDesc,
};
use spacetimedb_sdk::{
    __codegen::{__sats::bsatn, __ws::v2::*},
    unstable::streaming_decompression::streaming,
};
use std::{hint::black_box, time::Instant};

const REGION: Region = 14;
type Position = (u32, i32, i32);
type SkillWrite = (i64, i64, i64);

#[derive(Default)]
struct Operations {
    observations: Vec<(u64, Position)>,
    boards: Vec<SkillWrite>,
    updates: usize,
    missing: usize,
    messages: Vec<(ExperienceState, ExperienceState, Option<Position>)>,
}

fn replay_input(state: &AppState) -> Operations {
    let bytes = std::fs::read(std::env::var_os("XP_WIRE").expect("set XP_WIRE")).unwrap();
    let mut frames = &bytes[..];
    let mut mobile = HashMap::<u64, Position>::new();
    let mut ops = Operations::default();
    state.claim_locations.reset(REGION);
    while !frames.is_empty() {
        let len = u64::from_le_bytes(frames[1..9].try_into().unwrap()) as usize;
        let (wire, rest) = frames[9..].split_at(len);
        frames = rest;
        match streaming(wire).unwrap() {
            ServerMessage::SubscribeApplied(sub) => {
                // All three location_state query results are one initial subscription.
                let mut loc = Vec::new();
                for t in sub.rows.tables {
                    macro_rules! rows {
                        ($ty:ty) => {
                            t.rows
                                .iter_slices()
                                .map(|r| bsatn::from_slice::<$ty>(r).unwrap())
                        };
                    }
                    match &t.table[..] {
                        "claim_tile_state" => state
                            .claim_locations
                            .claims_initial(REGION, &rows!(ClaimTileState).collect::<Vec<_>>()),
                        "location_state" => loc.extend(rows!(LocationState)),
                        "dimension_description_state" => {
                            state.claim_locations.descriptions_initial(
                                REGION,
                                &rows!(DimensionDescriptionState).collect::<Vec<_>>(),
                            )
                        }
                        "dimension_network_state" => state.claim_locations.networks_initial(
                            REGION,
                            &rows!(DimensionNetworkState).collect::<Vec<_>>(),
                        ),
                        "mobile_entity_state" => mobile.extend(
                            rows!(MobileEntityState)
                                .map(|r| (r.entity_id, (r.dimension, r.location_x, r.location_z))),
                        ),
                        "player_state" => {
                            for row in rows!(PlayerState) {
                                let model = entity::player_state::ModelBuilder::new(row)
                                    .with_region(REGION)
                                    .build();
                                state.player_state.insert(model.entity_id, model);
                            }
                        }
                        "skill_desc" => {
                            for r in rows!(SkillDesc) {
                                state.skill_desc.write().insert(i64::from(r.id), r.into());
                            }
                        }
                        _ => {}
                    }
                }
                if !loc.is_empty() {
                    state.claim_locations.locations_initial(REGION, &loc);
                    state.claim_locations.finish_initial(REGION);
                }
            }
            ServerMessage::TransactionUpdate(tx) => {
                let mut xp = Vec::new();
                let mut old = HashMap::new();
                for qs in tx.query_sets {
                    for t in qs.tables {
                        for rows in t.rows {
                            let TableUpdateRows::PersistentTable(p) = rows else {
                                continue;
                            };
                            match &t.table_name[..] {
                                "experience_state" => {
                                    for row in p.deletes.iter_slices() {
                                        let r: ExperienceState = bsatn::from_slice(row).unwrap();
                                        assert!(old.insert(r.entity_id, r).is_none());
                                    }
                                    xp.extend(
                                        p.inserts.iter_slices().map(|r| {
                                            bsatn::from_slice::<ExperienceState>(r).unwrap()
                                        }),
                                    );
                                }
                                "mobile_entity_state" => {
                                    for row in p.deletes.iter_slices() {
                                        let r: MobileEntityState = bsatn::from_slice(row).unwrap();
                                        mobile.remove(&r.entity_id);
                                    }
                                    for row in p.inserts.iter_slices() {
                                        let r: MobileEntityState = bsatn::from_slice(row).unwrap();
                                        mobile.insert(
                                            r.entity_id,
                                            (r.dimension, r.location_x, r.location_z),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                for new in xp {
                    let Some(previous) = old.remove(&new.entity_id) else {
                        continue;
                    };
                    ops.updates += 1;
                    let mut positive = false;
                    for skill in &previous.experience_stacks {
                        let Some(next) = new
                            .experience_stacks
                            .iter()
                            .find(|s| s.skill_id == skill.skill_id && s.quantity > skill.quantity)
                        else {
                            continue;
                        };
                        positive = true;
                        let key = i64::from(skill.skill_id);
                        if state
                            .skill_desc
                            .read()
                            .get(&key)
                            .is_some_and(|s| s.skill_category != 0)
                        {
                            ops.boards
                                .push((new.entity_id as i64, key, i64::from(next.quantity)));
                        }
                    }
                    if positive {
                        if let Some(pos) = mobile.get(&new.entity_id) {
                            ops.observations.push((new.entity_id, *pos));
                        } else {
                            ops.missing += 1;
                        }
                    }
                    let position = mobile.get(&new.entity_id).copied();
                    ops.messages.push((previous, new, position));
                }
            }
            _ => {}
        }
    }
    if std::env::var_os("XP_NO_HINTS").is_some() {
        state
            .claim_locations
            .disable_search_hints_for_replay(REGION);
    }
    ops
}

#[tokio::test]
#[ignore = "isolated level lookups on the private region-14 capture"]
async fn replay_level_lookups() {
    use crate::leaderboard::EXPERIENCE_PER_LEVEL;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(Default::default(), tx, Default::default());
    let ops = replay_input(&state);
    let mut values = Vec::new();
    for (old, new, _) in &ops.messages {
        for (a, b) in old.experience_stacks.iter().zip(&new.experience_stacks) {
            assert_eq!(a.skill_id, b.skill_id);
            if a.quantity != b.quantity {
                values.extend([i64::from(a.quantity), i64::from(b.quantity)]);
            }
        }
    }
    assert_eq!(values.len(), 2 * ops.updates);
    let binary_search = |xp| {
        EXPERIENCE_PER_LEVEL
            .partition_point(|&(_, t)| t <= xp)
            .checked_sub(1)
            .map_or(100, |i| EXPERIENCE_PER_LEVEL[i].0)
    };
    for &xp in &values {
        assert_eq!(binary_search(xp), experience_to_level(xp));
    }
    fn measure(values: &[i64], convert: impl Fn(i64) -> i32) -> f64 {
        let start = Instant::now();
        for _ in 0..128 {
            black_box(black_box(values).iter().copied().map(&convert).sum::<i32>());
        }
        start.elapsed().as_secs_f64() * 1e9 / (values.len() * 128) as f64
    }
    for pass in 0..9 {
        let old = || measure(&values, binary_search);
        let new = || measure(&values, experience_to_level);
        let (search, lookup) = if pass % 2 == 0 {
            (old(), new())
        } else {
            let lookup = new();
            (old(), lookup)
        };
        println!("pass={pass} search_ns={search:.3} lookup_ns={lookup:.3}");
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "worker-thread CPU on the private region-14 capture"]
async fn replay_xp_worker() {
    use rustix::time::{ClockId, clock_gettime};
    let cpu_ns = || {
        let t = clock_gettime(ClockId::ThreadCPUTime);
        t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
    };
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(Default::default(), tx, Default::default());
    let ops = replay_input(&state);
    let mut worker = ExperienceStateWorker::new(state.clone(), 3000, Duration::from_secs(5));
    for pass in 0..8 {
        let mut last = HashMap::new();
        // Clone incoming owned messages and prepare the small number of movement
        // changes outside timing. No per-event clock calls.
        let input: Vec<_> = ops
            .messages
            .iter()
            .map(|(old, new, pos)| {
                let changed = last.insert(new.entity_id, *pos) != Some(*pos);
                (old.clone(), new.clone(), changed.then_some(*pos))
            })
            .collect();
        state.mobile_entity_state.clear();
        worker.pending.upserts.clear();
        worker.pending.claim_history.clear();
        let start = cpu_ns();
        for (old, new, change) in input {
            if let Some(pos) = change {
                if let Some((dimension, location_x, location_z)) = pos {
                    state.mobile_entity_state.insert(
                        new.entity_id,
                        entity::mobile_entity_state::Model {
                            entity_id: new.entity_id,
                            dimension,
                            location_x,
                            location_z,
                            region: REGION,
                            chunk_index: 0,
                            timestamp: 0,
                            destination_x: location_x,
                            destination_z: location_z,
                            is_walking: false,
                        },
                    );
                } else {
                    state.mobile_entity_state.remove(&new.entity_id);
                }
            }
            worker.handle_update(new, REGION, old).await;
        }
        let elapsed = cpu_ns() - start;
        let history = &worker.pending.claim_history;
        let mut digest = 0u64;
        for event in history {
            for value in [
                event.player_id as u64,
                event.skill_id as u64,
                event.previous_experience as u64,
                event.experience as u64,
                match event.location {
                    ClaimAtLocation::Claim(id) => id,
                    ClaimAtLocation::Unclaimed => 1,
                    ClaimAtLocation::Unknown => 2,
                },
            ] {
                digest = digest.wrapping_mul(31).wrapping_add(value);
            }
        }
        assert_eq!(history.len(), ops.boards.len()); // checked for this capture
        if pass != 0 {
            println!(
                "worker_cpu_ns_per_update={:.2} history={} digest={digest:x}",
                elapsed as f64 / ops.updates as f64,
                history.len()
            );
        }
    }
}

#[tokio::test]
#[ignore = "subscribed parity pass on the private region-14 capture"]
async fn replay_xp_parity() {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(Default::default(), tx, Default::default());
    let ops = replay_input(&state);
    let mut worker = ExperienceStateWorker::new(state.clone(), 3000, Duration::from_secs(5));
    let ranking = &state.ranking_system;
    let mut players = HashSet::new();
    let mut topics = Vec::new();
    for (old, _, _) in &ops.messages {
        if players.insert(old.entity_id) {
            for es in &old.experience_stacks {
                ranking.ensure_skill_leaderboard(i64::from(es.skill_id));
            }
            worker.handle_insert(old.clone(), REGION).await;
            for topic in [
                "level",
                "experience",
                "total_experience",
                "total_adventure_experience",
                "total_profession_experience",
            ] {
                topics.push((topic, Some(old.entity_id as i64)));
            }
        }
    }
    // Publish the same nonempty initial ranks once. Actual rank publication
    // is asynchronous; racing it would make exact payload comparison flaky.
    ranking.shutdown().await;
    let _sub = state.subscribe_test_client(&topics).await;
    let mut scores = DefaultHasher::new();
    let mut events = DefaultHasher::new();
    let mut count = 0;
    for (old, new, _) in ops.messages {
        let id = new.entity_id as i64;
        let stacks = new.experience_stacks.clone();
        worker.handle_update(new, REGION, old).await;
        id.hash(&mut scores);
        for board in [
            &ranking.global_leaderboard,
            &ranking.level_leaderboard,
            &ranking.profession_leaderboard,
            &ranking.adventure_leaderboard,
            &ranking.xp_per_hour,
        ] {
            board.get_value(&id).hash(&mut scores);
        }
        for es in stacks {
            es.skill_id.hash(&mut scores);
            ranking
                .skill_leaderboard(i64::from(es.skill_id))
                .and_then(|board| board.get_value(&id))
                .hash(&mut scores);
        }
        count.hash(&mut events); // delimit events belonging to each update
        while let Ok(message) = notifications.try_recv() {
            serde_json::to_vec(&message).unwrap().hash(&mut events);
            count += 1;
        }
    }
    assert!(count > ops.updates);
    println!(
        "scores={:016x} notifications={count} payloads={:016x}",
        scores.finish(),
        events.finish()
    );
}

#[tokio::test]
#[ignore = "uses private region-14 capture; run pinned with --nocapture"]
async fn compare_xp_paths() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(Default::default(), tx, Default::default());
    let ops = replay_input(&state);
    assert!(!ops.observations.is_empty() && !ops.boards.is_empty());
    let mut last = HashMap::new();
    let mut repeated = 0;
    let mut consecutive = 0;
    let mut prev = None;
    let mut distinct = HashSet::new();
    for &(id, (dim, x, z)) in &ops.observations {
        let tile = (dim, if dim == 1 { mobile_tile(x, z) } else { (0, 0) });
        distinct.insert(tile);
        repeated += usize::from(last.insert(id, tile) == Some(tile));
        consecutive += usize::from(prev.replace(tile) == Some(tile));
    }
    println!(
        "xp={} located={} missing={} players={} distinct={} repeat_player={} consecutive={} skill_writes={}",
        ops.updates,
        ops.observations.len(),
        ops.missing,
        last.len(),
        distinct.len(),
        repeated,
        consecutive,
        ops.boards.len()
    );
    let claim = || {
        let mut hash = 0_u64;
        for &(_, (dim, x, z)) in &ops.observations {
            let key = match state.claim_locations.lookup(REGION, dim, x, z) {
                ClaimAtLocation::Claim(id) => id,
                ClaimAtLocation::Unclaimed => 1,
                ClaimAtLocation::Unknown => 2,
            };
            hash = hash.wrapping_mul(31).wrapping_add(key);
        }
        black_box(hash)
    };
    let boards = || {
        let ranking = &state.ranking_system;
        for &(player, skill, xp) in &ops.boards {
            ranking
                .ensure_skill_leaderboard(skill)
                .update_resolved(&ranking.global_leaderboard.resolve_player(player), xp);
        }
    };
    let hash = claim();
    boards();
    for block in 0..6 {
        for which in [block % 2, 1 - block % 2] {
            let start = Instant::now();
            for _ in 0..32 {
                if which == 0 {
                    assert_eq!(claim(), hash);
                } else {
                    boards();
                }
            }
            let elapsed = start.elapsed().as_nanos();
            let (name, count) = if which == 0 {
                ("claim", ops.observations.len())
            } else {
                ("skill_board", ops.boards.len())
            };
            println!(
                "{name} ns/op={:.2} hash={hash:x}",
                elapsed as f64 / (32 * count) as f64
            );
        }
    }
}
