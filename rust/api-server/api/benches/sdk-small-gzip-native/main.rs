use game_module::module_bindings::*;
use spacetimedb_sdk::{
    __codegen::{
        self as sdk,
        __sats::{Serialize, bsatn},
    },
    EventTable, Table, TableWithPrimaryKey,
    unstable::streaming_decompression::ReplayDecoder,
};
use std::{cell::Cell, env, fs, hint::black_box, time::Instant};

#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;
use sha2::{Digest as _, Sha256};
use std::cell::RefCell;
fn hash(bytes: &[u8]) {
    HASH.with_borrow_mut(|h| {
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    });
}
thread_local! {
    static CALLS: Cell<u64> = const {Cell::new(0)};
    static HASH: RefCell<Sha256> = RefCell::default();
    static LAST: Cell<Option<Instant>> = const {Cell::new(None)};
}
fn callback<const CHECK: bool, const TIME: bool, R: Serialize>(
    kind: u8,
    table: &str,
    a: &R,
    b: Option<&R>,
) {
    if TIME {
        LAST.set(Some(Instant::now()));
    }
    CALLS.set(CALLS.get() + 1);
    if CHECK {
        hash(&[kind]);
        hash(table.as_bytes());
        hash(&bsatn::to_vec(a).unwrap());
        if let Some(b) = b {
            hash(&bsatn::to_vec(b).unwrap());
        }
    } else {
        black_box((a, b));
    }
}
type Ctx = sdk::DbContextImpl<RemoteModule>;
fn context<const CHECK: bool, const TIME: bool>() -> Ctx {
    let imp = Ctx::replay_context();
    let db: DbConnection = sdk::DbConnection::new(imp.clone());
    macro_rules! listen {
        ($($name:ident),+) => {$({
            let table = db.db.$name();
            table.on_insert(|_,r|callback::<CHECK,TIME,_>(1,stringify!($name),r,None));
            table.on_delete(|_,r|callback::<CHECK,TIME,_>(2,stringify!($name),r,None));
            table.on_update(|_,a,b|callback::<CHECK,TIME,_>(3,stringify!($name),a,Some(b)));
        })+}
    }
    listen!(
        player_action_state,
        mobile_entity_state,
        progressive_action_state,
        experience_state,
        stamina_state,
        claim_local_state,
        passive_craft_state,
        inventory_state,
        player_state,
        health_state,
        character_stats_state,
        public_progressive_action_state,
        deployable_state_v_2,
        sell_order_state,
        building_state,
        location_state,
        player_housing_state,
        vault_state,
        player_username_state,
        traveler_task_state,
        equipment_state,
        active_buff_state
    );
    db.db
        .craft_event()
        .on_insert(|_, r| callback::<CHECK, TIME, _>(1, "craft_event", r, None));
    macro_rules! initial { ($($name:ident),+) => {$({
        db.db.$name().on_initial_rows(|_, rows| {
            CALLS.set(CALLS.get()+1);
            if TIME { LAST.set(Some(Instant::now())); }
            if CHECK {
                hash(&[4]); hash(stringify!($name).as_bytes()); hash(&(rows.len() as u64).to_le_bytes());
                for row in rows.iter() { hash(&bsatn::to_vec(row).unwrap()); }
            } else { black_box(rows); }
        });
    })+} }
    initial!(
        active_buff_state,
        building_desc,
        building_nickname_state,
        building_state,
        buff_desc,
        buff_type_desc,
        buy_order_state,
        cargo_desc,
        character_stats_state,
        claim_local_state,
        claim_member_state,
        claim_state,
        claim_tech_desc,
        claim_tech_state,
        claim_tile_state,
        collectible_desc,
        crafting_recipe_desc,
        deployable_state_v_2,
        dimension_description_state,
        dimension_network_state,
        equipment_desc,
        equipment_state,
        experience_state,
        extraction_recipe_desc,
        food_desc,
        health_state,
        interior_network_desc,
        inventory_state,
        item_desc,
        item_list_desc,
        location_state,
        mobile_entity_state,
        npc_desc,
        passive_craft_state,
        permission_state,
        player_action_state,
        player_housing_state,
        player_state,
        player_username_state,
        portal_state,
        progressive_action_state,
        public_progressive_action_state,
        resource_desc,
        sell_order_state,
        skill_desc,
        stamina_state,
        traveler_task_desc,
        traveler_task_state,
        vault_state
    );
    imp.replay_register();
    imp
}
fn apply<const CHECK: bool>(c: &Ctx, decoder: &mut ReplayDecoder, frame: &[u8]) {
    let msg = decoder.decode(black_box(frame)).unwrap();
    if CHECK {
        hash(&[0xff]);
        hash(&bsatn::to_vec(&msg).unwrap());
    }
    c.replay_message(msg);
}
fn cpu_ns() -> i64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) },
        0
    );
    t.tv_sec * 1_000_000_000 + t.tv_nsec
}
fn p99(a: &mut [u128]) -> u128 {
    let n = (a.len() * 99).div_ceil(100) - 1;
    *a.select_nth_unstable(n).1
}
fn main() {
    let bytes = fs::read(env::args_os().nth(1).unwrap()).unwrap();
    let mut src = &bytes[..];
    let select = env::args().nth(3).unwrap();
    let mut decoder = ReplayDecoder::default();
    let mut frames = Vec::new();
    while !src.is_empty() {
        let len = usize::try_from(u64::from_le_bytes(src[1..9].try_into().unwrap())).unwrap();
        let tag = src[0];
        let (wire, next) = src[9..].split_at(len);
        if select == "mixed" || (select == "gzip-tx" && tag == 4 && wire[0] == 2) {
            frames.push(wire);
        }
        src = next;
    }
    assert!(!frames.is_empty());
    let run = |c: &Ctx, decoder: &mut ReplayDecoder| {
        for f in &frames {
            apply::<false>(c, decoder, f);
        }
    };
    match env::args().nth(2).unwrap().as_str() {
        "check" => {
            let c = context::<true, false>();
            for f in &frames {
                apply::<true>(&c, &mut decoder, f);
            }
            println!(
                "frames={} calls={} hash={:x}",
                frames.len(),
                CALLS.get(),
                HASH.with_borrow(|h| h.clone().finalize())
            );
        }
        "cpu" => {
            let c = context::<false, false>();
            run(&c, &mut decoder);
            let t = cpu_ns();
            for _ in 0..4 {
                run(&c, &mut decoder);
            }
            let elapsed = cpu_ns() - t;
            println!(
                "cpu_ms_per_1000={:.5}",
                elapsed as f64 / (4. * frames.len() as f64 * 1000.)
            );
        }
        "burst" => {
            let c = context::<false, true>();
            run(&c, &mut decoder);
            let mut lag = Vec::with_capacity(frames.len());
            let mut service = Vec::with_capacity(frames.len());
            for group in frames.chunks(32) {
                let ready = Instant::now();
                for f in group {
                    LAST.set(None);
                    let start = Instant::now();
                    apply::<false>(&c, &mut decoder, f);
                    if let Some(last) = LAST.get() {
                        lag.push((last - ready).as_nanos());
                        service.push((last - start).as_nanos());
                    }
                }
            }
            println!(
                "observed={} burst_p99_ns={} service_p99_ns={}",
                lag.len(),
                p99(&mut lag),
                p99(&mut service)
            );
        }
        _ => panic!("bad case"),
    }
}
