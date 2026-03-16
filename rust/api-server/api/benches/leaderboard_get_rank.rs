use api::leaderboard::LeaderboardBenchHarness;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_get_rank(c: &mut Criterion) {
    let mut group = c.benchmark_group("leaderboard_get_rank");
    group.sample_size(30);

    for player_count in [10_000usize, 100_000usize] {
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

criterion_group!(benches, bench_get_rank);
criterion_main!(benches);
