use super::*;
use rustix::time::{ClockId, clock_gettime};
use sea_orm::{DbBackend, MockDatabase};
use std::{hint::black_box, time::Duration};

fn cpu() -> Duration {
    clock_gettime(ClockId::ThreadCPUTime).try_into().unwrap()
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "release CPU replay; see README.md"]
async fn coalesced_flush_cpu() {
    const OWNER: i64 = 9_001_234_567;
    for matching in [false, true] {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let _client = if matching {
            Some(
                state
                    .subscribe_test_client(&[("player_craft_progress", Some(OWNER))])
                    .await,
            )
        } else {
            None
        };
        for n in [0_usize, 1, 2, 144, 472, 538] {
            let template: HashMap<_, _> = (0..n as i64)
                .map(|i| {
                    let id = 7_100_000_000 + i;
                    (
                        id,
                        ProgressiveActionState {
                            entity_id: id as u64,
                            building_entity_id: 1_000_000 + i as u64 % 70,
                            function_type: 3,
                            progress: 100 + i as i32,
                            recipe_id: 4,
                            craft_count: 5,
                            last_crit_outcome: 0,
                            owner_entity_id: OWNER as u64,
                            lock_expiration:
                                spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(
                                    1_795_456_789_123_456 + i,
                                ),
                            preparation: false,
                        },
                    )
                })
                .collect();
            // Amortize clock reads. Keep the group bounded even when a view
            // subscribes to every row and the outbound queue must retain them.
            let group = (4096 / n.max(1)).clamp(1, 1024);
            let mut maps = vec![template.clone(); group];
            let mut elapsed = Duration::ZERO;
            const GROUPS: usize = 32;
            for round in 0..=GROUPS {
                let start = cpu();
                for map in &mut maps {
                    progress(black_box(&state), black_box(map)).await;
                }
                let delta = cpu() - start;
                if round > 0 {
                    elapsed += delta;
                }
                let mut delivered = 0;
                while let Ok(msg) = rx.try_recv() {
                    assert!(matches!(
                        msg,
                        OutboundWebSocketMessages::CraftProgressChanged { .. }
                    ));
                    delivered += 1;
                }
                assert_eq!(delivered, if matching { n * group } else { 0 });
                for map in &mut maps {
                    assert!(map.is_empty());
                    map.clone_from(&template);
                }
            }
            println!(
                "sample,{matching},{n},{},{:}",
                GROUPS * group,
                elapsed.as_nanos()
            );
        }
        assert!(db.into_transaction_log().is_empty());
    }
}
