use game_module::module_bindings::*;
use spacetimedb_sdk::{
    __codegen::{
        self as sdk,
        __sats::{Serialize, bsatn},
        __ws::v2::ServerMessage,
    },
    EventTable, Table, TableWithPrimaryKey,
    unstable::streaming_decompression::streaming,
};
use std::{cell::Cell, env, fs, hint::black_box, time::Instant};

#[cfg(not(feature = "alloc-count"))]
#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

#[cfg(feature = "alloc-count")]
mod alloc {
    use mimalloc_safe::MiMalloc;
    use std::{
        alloc::{GlobalAlloc, Layout},
        cell::Cell,
    };
    thread_local! { pub static CALLS: Cell<u64> = const { Cell::new(0) }; }
    struct Counter;
    #[global_allocator]
    static GLOBAL: Counter = Counter;
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            unsafe { MiMalloc.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            unsafe { MiMalloc.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            CALLS.set(CALLS.get() + 1);
            unsafe { MiMalloc.realloc(p, l, n) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { MiMalloc.dealloc(p, l) }
        }
    }
}
thread_local! {
    static CALLS: Cell<u64> = const {Cell::new(0)};
    static HASH: Cell<u64> = const {Cell::new(0)};
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
        let mut h = HASH.get();
        for byte in [kind]
            .into_iter()
            .chain(table.bytes())
            .chain(bsatn::to_vec(a).unwrap())
            .chain(b.into_iter().flat_map(|r| bsatn::to_vec(r).unwrap()))
        {
            h = (h ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        HASH.set(h);
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
        player_housing_state
    );
    db.db
        .craft_event()
        .on_insert(|_, r| callback::<CHECK, TIME, _>(1, "craft_event", r, None));
    imp.replay_register();
    imp
}
fn apply(c: &Ctx, frame: &[u8]) {
    let ServerMessage::TransactionUpdate(tx) = streaming(black_box(frame)).unwrap() else {
        panic!("expected tx")
    };
    c.replay_tx(DbUpdate::try_from(tx).unwrap());
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
    let mut frames = Vec::new();
    while !src.is_empty() {
        let len = usize::try_from(u64::from_le_bytes(src[1..9].try_into().unwrap())).unwrap();
        let tag = src[0];
        let (wire, next) = src[9..].split_at(len);
        if tag == 4 && wire[0] == 0 {
            frames.push(wire);
        }
        src = next;
    }
    assert!(!frames.is_empty());
    let run = |c: &Ctx| {
        for f in &frames {
            apply(c, f);
        }
    };
    match env::args().nth(2).unwrap().as_str() {
        "check" => {
            let c = context::<true, false>();
            run(&c);
            println!(
                "frames={} calls={} hash={:016x}",
                frames.len(),
                CALLS.get(),
                HASH.get()
            );
        }
        "cpu" => {
            let c = context::<false, false>();
            run(&c);
            #[cfg(feature = "alloc-count")]
            alloc::CALLS.set(0);
            let t = cpu_ns();
            for _ in 0..4 {
                run(&c);
            }
            let elapsed = cpu_ns() - t;
            #[cfg(feature = "alloc-count")]
            println!("alloc_calls_per_pass={}", alloc::CALLS.get() / 4);
            println!(
                "cpu_ms_per_1000={:.5}",
                elapsed as f64 / (4. * frames.len() as f64 * 1000.)
            );
        }
        "burst" => {
            let c = context::<false, true>();
            run(&c);
            let mut lag = Vec::with_capacity(frames.len());
            let mut service = Vec::with_capacity(frames.len());
            for group in frames.chunks(32) {
                let ready = Instant::now();
                for f in group {
                    LAST.set(None);
                    let start = Instant::now();
                    apply(&c, f);
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
