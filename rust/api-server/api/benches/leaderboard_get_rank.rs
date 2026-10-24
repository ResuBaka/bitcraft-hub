use api::leaderboard::LeaderboardBenchHarness;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

fn bench_get_rank(c: &mut Criterion) {
    let mut group = c.benchmark_group("leaderboard_get_rank");
    group.sample_size(30);

    for player_count in [10_000usize, 48_506usize, 100_000usize] {
        let harness = LeaderboardBenchHarness::new(player_count);
        let low_user = harness.user_at_percentile(10);
        let mid_user = harness.user_at_percentile(50);
        let high_user = harness.user_at_percentile(90);

        group.bench_with_input(
            BenchmarkId::new("single_lookup_p10", player_count),
            &player_count,
            |b, _| {
                b.iter(|| {
                    black_box(harness.get_rank(black_box(low_user)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("single_lookup_p50", player_count),
            &player_count,
            |b, _| {
                b.iter(|| {
                    black_box(harness.get_rank(black_box(mid_user)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("single_lookup_p90", player_count),
            &player_count,
            |b, _| {
                b.iter(|| {
                    black_box(harness.get_rank(black_box(high_user)));
                });
            },
        );
    }

    group.finish();
}

fn bench_reads_and_updates(c: &mut Criterion) {
    const PLAYERS: usize = 48_506;
    let harness = LeaderboardBenchHarness::new(PLAYERS);
    let user = harness.user_at_percentile(50);
    assert!(harness.get_rank(user).is_some());
    c.bench_function("leaderboard_score_lookup_48506", |b| {
        b.iter(|| black_box(harness.get_value(black_box(user))));
    });
    c.bench_function("leaderboard_top100_48506", |b| {
        b.iter(|| black_box(harness.get_range(0, 100)));
    });
    c.bench_function("leaderboard_page_offset24000_48506", |b| {
        b.iter(|| black_box(harness.get_range(24_000, 100)));
    });
    let mut group = c.benchmark_group("leaderboard_publish_48506");
    for batch in [1usize, 10, 100, 1_000, 5_000] {
        let mut harness = LeaderboardBenchHarness::new(PLAYERS);
        let users = (0..batch)
            .map(|i| ((i * 7_919) % PLAYERS + 1) as i64)
            .collect::<Vec<_>>();
        let mut round = 0;
        group.bench_function(BenchmarkId::new("batch", batch), |b| {
            b.iter(|| {
                round += 1;
                harness.update_and_publish(&users, 1_000 + (round % 2) * 10_000);
            })
        });
    }
    group.finish();
    c.bench_function("leaderboard_startup_48506", |b| {
        b.iter(|| black_box(LeaderboardBenchHarness::new(PLAYERS)));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(30).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(2));
    targets = bench_get_rank, bench_reads_and_updates
}
criterion_main!(benches);
