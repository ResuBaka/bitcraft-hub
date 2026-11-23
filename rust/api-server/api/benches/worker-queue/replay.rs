use super::*;
use rustix::time::{ClockId, clock_gettime};

fn cpu_ns() -> u64 {
    let t = clock_gettime(ClockId::ThreadCPUTime);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

// Two concurrent producers publishing to one receiver, without sleeps. Count
// CPU on all three threads; verify per-producer FIFO and that EOF drains fully.
#[tokio::test]
#[ignore = "cross-thread queue benchmark; allow at least three physical cores"]
async fn bench_concurrent_worker_queue() {
    const N: usize = 131_072;
    for pass in 0..2 {
        let (tx, mut rx) = unbounded_channel();
        let start = Arc::new(std::sync::Barrier::new(3));
        let producers = [0, 1].map(|id| {
            let tx = tx.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                let t = cpu_ns();
                for seq in 0..N {
                    tx.send((id, seq)).unwrap();
                }
                cpu_ns() - t
            })
        });
        drop(tx);
        let mut buffer = Vec::with_capacity(64);
        let mut counts = [0; 2];
        start.wait();
        let t = cpu_ns();
        while rx.recv_many(&mut buffer, 64).await != 0 {
            for (producer, seq) in buffer.drain(..) {
                assert_eq!(counts[producer], seq);
                counts[producer] += 1;
            }
        }
        let elapsed = cpu_ns() - t
            + producers
                .into_iter()
                .map(|p| p.join().unwrap())
                .sum::<u64>();
        assert_eq!(counts, [N, N]);
        if pass == 1 {
            println!(
                "concurrent cpu_ns_per_message={:.2}",
                elapsed as f64 / (2 * N) as f64
            );
        }
    }
}

// Current-thread runtime: the clock charges both producers and consumers,
// including polling, waking and yielding for the handoff workload.
#[tokio::test]
#[ignore = "queue CPU microbenchmark; run pinned with --nocapture"]
async fn bench_worker_queue_hot_paths() {
    const N: u64 = 262_144;
    for mode in ["ready", "batch1", "batch32", "handoff"] {
        for pass in 0..2 {
            let (tx, mut rx) = unbounded_channel();
            let mut sum = 0u64;
            let mut buffer = Vec::with_capacity(64);
            let start = cpu_ns();
            match mode {
                "ready" => {
                    for i in 0..N {
                        tx.send(i).unwrap();
                        sum += rx.recv().await.unwrap();
                    }
                }
                "handoff" => {
                    let producer = async {
                        for i in 0..N {
                            tx.send(i).unwrap();
                            tokio::task::yield_now().await;
                        }
                    };
                    let consumer = async {
                        for _ in 0..N {
                            sum += rx.recv().await.unwrap();
                        }
                    };
                    tokio::join!(biased; consumer, producer);
                }
                _ => {
                    let burst = if mode == "batch1" { 1 } else { 32 };
                    for first in (0..N).step_by(burst) {
                        for i in first..first + burst as u64 {
                            tx.send(i).unwrap();
                        }
                        assert_eq!(rx.recv_many(&mut buffer, 64).await, burst);
                        sum += buffer.drain(..).sum::<u64>();
                    }
                }
            }
            let elapsed = cpu_ns() - start;
            assert_eq!(sum, (N - 1) * N / 2);
            if pass == 1 {
                println!("{mode} cpu_ns_per_message={:.2}", elapsed as f64 / N as f64);
            }
        }
    }
}
