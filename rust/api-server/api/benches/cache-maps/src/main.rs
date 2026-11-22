use std::hint::black_box;
use std::sync::Barrier;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

// Approximate an owned item/cargo description without linking the server.
struct Row {
    name: String,
    description: String,
    model: String,
    icon: String,
    tag: String,
    attributes: [u32; 8],
}

impl Row {
    fn new(id: u32, revision: u32) -> Self {
        Self {
            name: format!("Item {id:08x}"),
            description: format!(
                "Description for item {id:08x} at revision {revision:08x}. \
                 Used for crafting and trading in the world."
            ),
            model: format!("models/item/{id:08x}"),
            icon: format!("icons/item/{id:08x}"),
            tag: format!("category/{:02x}", id % 32),
            attributes: [revision; 8],
        }
    }

    fn sample(&self) -> u64 {
        (self.name.len()
            + self.description.len()
            + self.model.len()
            + self.icon.len()
            + self.tag.len()) as u64
            + u64::from(self.attributes[0])
    }
}

trait View {
    fn put(&self, key: u32, row: Row);
    fn remove(&self, key: u32);
    fn sample(&self, key: u32) -> Option<u64>;
    fn scan(&self) -> (usize, u64);
}

trait Store: Default + Sync {
    fn view(&self) -> impl View + '_;
    fn len(&self) -> usize;

    // Return the read handle so the churn case can hold it until after writes.
    fn hold_read(&self, key: u32) -> impl Sized + '_ {
        let view = self.view();
        assert!(black_box(view.sample(key)).is_some());
        view
    }
}

impl Store for dashmap::DashMap<u32, Row> {
    fn view(&self) -> impl View + '_ {
        self
    }

    fn len(&self) -> usize {
        self.len()
    }
}

impl View for &dashmap::DashMap<u32, Row> {
    fn put(&self, key: u32, row: Row) {
        self.insert(key, row);
    }

    fn remove(&self, key: u32) {
        (*self).remove(&key);
    }

    fn sample(&self, key: u32) -> Option<u64> {
        self.get(&key).map(|row| row.sample())
    }

    fn scan(&self) -> (usize, u64) {
        self.iter()
            .fold((0, 0), |(count, sum), row| (count + 1, sum + row.sample()))
    }
}

impl Store for clashmap::ClashMap<u32, Row> {
    fn view(&self) -> impl View + '_ {
        self
    }

    fn len(&self) -> usize {
        self.len()
    }
}

impl View for &clashmap::ClashMap<u32, Row> {
    fn put(&self, key: u32, row: Row) {
        self.insert(key, row);
    }

    fn remove(&self, key: u32) {
        (*self).remove(&key);
    }

    fn sample(&self, key: u32) -> Option<u64> {
        self.get(&key).map(|row| row.sample())
    }

    fn scan(&self) -> (usize, u64) {
        self.iter()
            .fold((0, 0), |(count, sum), row| (count + 1, sum + row.sample()))
    }
}

impl Store for papaya::HashMap<u32, Row> {
    fn view(&self) -> impl View + '_ {
        self.pin()
    }

    fn len(&self) -> usize {
        self.len()
    }
}

impl View for papaya::HashMapRef<'_, u32, Row, std::hash::RandomState, papaya::LocalGuard<'_>> {
    fn put(&self, key: u32, row: Row) {
        self.insert(key, row);
    }

    fn remove(&self, key: u32) {
        papaya::HashMapRef::remove(self, &key);
    }

    fn sample(&self, key: u32) -> Option<u64> {
        self.get(&key).map(Row::sample)
    }

    fn scan(&self) -> (usize, u64) {
        self.iter().fold((0, 0), |(count, sum), (_, row)| {
            (count + 1, sum + row.sample())
        })
    }
}

impl Store for scc::HashMap<u32, Row> {
    fn view(&self) -> impl View + '_ {
        self
    }

    fn len(&self) -> usize {
        self.len()
    }
}

impl View for &scc::HashMap<u32, Row> {
    fn put(&self, key: u32, row: Row) {
        self.upsert_sync(key, row);
    }

    fn remove(&self, key: u32) {
        self.remove_sync(&key);
    }

    fn sample(&self, key: u32) -> Option<u64> {
        self.read_sync(&key, |_, row| row.sample())
    }

    fn scan(&self) -> (usize, u64) {
        let mut count = 0;
        let mut sum = 0;
        self.iter_sync(|_, row| {
            count += 1;
            sum += row.sample();
            true
        });
        (count, sum)
    }
}

impl Store for scc::HashIndex<u32, Row> {
    fn view(&self) -> impl View + '_ {
        self
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn hold_read(&self, key: u32) -> impl Sized + '_ {
        let guard = scc::Guard::new();
        assert!(black_box(self.peek(&key, &guard).map(Row::sample)).is_some());
        guard
    }
}

impl View for &scc::HashIndex<u32, Row> {
    fn put(&self, key: u32, row: Row) {
        use scc::hash_index::Entry;
        match self.entry_sync(key) {
            Entry::Occupied(mut entry) => entry.update(row),
            Entry::Vacant(entry) => {
                entry.insert_entry(row);
            }
        }
    }

    fn remove(&self, key: u32) {
        self.remove_sync(&key);
    }

    fn sample(&self, key: u32) -> Option<u64> {
        self.peek_with(&key, |_, row| row.sample())
    }

    fn scan(&self) -> (usize, u64) {
        let guard = scc::Guard::new();
        self.iter(&guard).fold((0, 0), |(count, sum), (_, row)| {
            (count + 1, sum + row.sample())
        })
    }
}

fn scc_simd() -> &'static str {
    if cfg!(all(target_arch = "x86_64", target_feature = "avx2")) {
        "avx2"
    } else if cfg!(all(target_arch = "x86_64", target_feature = "sse2")) {
        "sse2"
    } else if cfg!(all(target_arch = "aarch64", target_feature = "neon")) {
        "neon"
    } else {
        "fallback"
    }
}

#[derive(Clone, Copy)]
enum Case {
    Load,
    Mixed,
    Scan,
    Churn,
    Pinned,
}

struct Config {
    case: Case,
    rows: u32,
    ops: u32,
    readers: usize,
}

fn checkpoint(name: &str) {
    // Leave a visible plateau in the Heaptrack timeline before the next phase.
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        let memory: Vec<_> = status
            .lines()
            .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
            .collect();
        println!("phase={name} {}", memory.join(" "));
    } else {
        println!("phase={name}");
    }
    if env::var_os("CACHE_MAP_PROFILE_NO_PAUSE").is_none() {
        thread::sleep(Duration::from_millis(250));
    }
}

fn next_key(state: &mut u64, rows: u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state % u64::from(rows)) as u32
}

fn write_batch(map: &impl Store, rows: u32, ops: u32) {
    let mut rng = 0xc61d_a847;
    for i in 0..ops {
        let id = next_key(&mut rng, rows);
        let view = map.view();
        if i % 4 == 0 {
            view.remove(id);
        }
        view.put(id, Row::new(id, i + 1));
    }
}

fn mixed(map: &impl Store, cfg: &Config) {
    let start = Barrier::new(cfg.readers + 2);
    let writes = (cfg.ops / 20).max(1);
    thread::scope(|scope| {
        let readers: Vec<_> = (0..cfg.readers)
            .map(|index| {
                let start = &start;
                scope.spawn(move || {
                    let mut rng = 0x54ae_95df + index as u64;
                    start.wait();
                    let begin = Instant::now();
                    let mut sum = 0;
                    for _ in 0..cfg.ops {
                        let id = next_key(&mut rng, cfg.rows);
                        // Per-lookup pin/guard, as in an independent API lookup.
                        sum += map.view().sample(id).unwrap_or(0);
                    }
                    (begin.elapsed(), black_box(sum))
                })
            })
            .collect();
        let writer = scope.spawn(|| {
            start.wait();
            let begin = Instant::now();
            write_batch(map, cfg.rows, writes);
            begin.elapsed()
        });
        start.wait();
        let (slowest, checksum) = readers
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold((Duration::ZERO, 0u64), |(max, sum), (elapsed, value)| {
                (max.max(elapsed), sum.wrapping_add(value))
            });
        println!(
            "reads={} writes={writes} slowest_reader_ms={:.2} writer_ms={:.2} checksum={checksum}",
            cfg.readers as u64 * u64::from(cfg.ops),
            slowest.as_secs_f64() * 1e3,
            writer.join().unwrap().as_secs_f64() * 1e3,
        );
    });
}

fn churn(map: &impl Store, cfg: &Config, pinned: bool) {
    // Hold a reclamation guard for Papaya or HashIndex, but no entry locks.
    let reader = pinned.then(|| map.hold_read(0));
    let begin = Instant::now();
    write_batch(map, cfg.rows, cfg.ops);
    println!(
        "writes={} churn_ms={:.2}",
        cfg.ops,
        begin.elapsed().as_secs_f64() * 1e3,
    );
    checkpoint("churn_complete");
    drop(reader);
    // Both cases do the same follow-up work. Reclamation is not an explicit GC.
    write_batch(map, cfg.rows, (cfg.ops / 100).max(1));
    checkpoint("after_release_and_followup");
}

fn run<M: Store>(cfg: &Config) {
    checkpoint("before_load");
    let map = M::default();
    let begin = Instant::now();
    for id in 0..cfg.rows {
        map.view().put(id, Row::new(id, 0));
    }
    println!("load_ms={:.2}", begin.elapsed().as_secs_f64() * 1e3);
    checkpoint("loaded");
    match cfg.case {
        Case::Load => {}
        Case::Mixed => {
            mixed(&map, cfg);
            checkpoint("mixed_complete");
        }
        Case::Scan => {
            let begin = Instant::now();
            let mut checksum = 0u64;
            for _ in 0..cfg.ops {
                let (count, sum) = map.view().scan();
                assert_eq!(count, cfg.rows as usize);
                checksum = checksum.wrapping_add(black_box(sum));
            }
            println!(
                "scans={} scan_ms={:.2} checksum={checksum}",
                cfg.ops,
                begin.elapsed().as_secs_f64() * 1e3,
            );
            checkpoint("scan_complete");
        }
        Case::Churn => churn(&map, cfg, false),
        Case::Pinned => churn(&map, cfg, true),
    }
    assert_eq!(map.len(), cfg.rows as usize);
    drop(map);
    checkpoint("map_dropped");
}

fn main() -> Result<(), String> {
    const USAGE: &str = "usage: cache-map-profile <dashmap|clashmap|papaya|scc-map|scc-index> <load|mixed|scan|churn|pinned> [rows] [ops] [readers]";
    let args: Vec<_> = env::args().skip(1).collect();
    if args.as_slice() == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if !(2..=5).contains(&args.len()) {
        return Err(USAGE.into());
    }
    let case = match args[1].as_str() {
        "load" => Case::Load,
        "mixed" => Case::Mixed,
        "scan" => Case::Scan,
        "churn" => Case::Churn,
        "pinned" => Case::Pinned,
        _ => return Err(USAGE.into()),
    };
    let number = |index: usize, default: u32, name: &str| {
        args.get(index).map_or(Ok(default), |arg| {
            arg.parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
    };
    let cfg = Config {
        case,
        rows: number(2, 16_384, "rows")?,
        ops: number(
            3,
            if matches!(case, Case::Scan) {
                64
            } else {
                100_000
            },
            "ops",
        )?,
        readers: number(4, 4, "readers")? as usize,
    };
    if !matches!(
        args[0].as_str(),
        "dashmap" | "clashmap" | "papaya" | "scc-map" | "scc-index"
    ) {
        return Err(USAGE.into());
    }
    println!(
        "backend={} case={} rows={} ops={} readers={} dashmap_inline_more={} clashmap_inline={} scc_simd={}",
        args[0],
        args[1],
        cfg.rows,
        cfg.ops,
        cfg.readers,
        cfg!(feature = "dashmap-inline-more"),
        cfg!(feature = "clashmap-inline"),
        scc_simd(),
    );
    match args[0].as_str() {
        "dashmap" => run::<dashmap::DashMap<_, _>>(&cfg),
        "clashmap" => run::<clashmap::ClashMap<_, _>>(&cfg),
        "papaya" => run::<papaya::HashMap<_, _>>(&cfg),
        "scc-map" => run::<scc::HashMap<_, _>>(&cfg),
        "scc-index" => run::<scc::HashIndex<_, _>>(&cfg),
        _ => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise<M: Store>() -> (usize, u64) {
        let map = M::default();
        map.view().put(0, Row::new(0, 0));
        map.view().put(1, Row::new(1, 0));
        let original = map.view().sample(0).unwrap();
        map.view().put(0, Row::new(0, 1));
        assert_eq!(map.view().sample(0), Some(original + 1));
        map.view().remove(1);
        assert_eq!(map.view().sample(1), None);
        assert_eq!(map.len(), 1);
        let result = map.view().scan();
        assert_eq!(result, (1, original + 1));
        result
    }

    #[test]
    fn backends_agree_on_replacement_removal_and_scan() {
        let dashmap = exercise::<dashmap::DashMap<_, _>>();
        assert_eq!(dashmap, exercise::<clashmap::ClashMap<_, _>>());
        assert_eq!(dashmap, exercise::<papaya::HashMap<_, _>>());
        assert_eq!(dashmap, exercise::<scc::HashMap<_, _>>());
        assert_eq!(dashmap, exercise::<scc::HashIndex<_, _>>());
    }
}
