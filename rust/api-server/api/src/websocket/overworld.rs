use game_module::module_bindings::*;
use spacetimedb_sdk::{DbContext, Event};
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

fn chunk_index(coordinates: ChunkCoordinatesMessage) -> u64 {
    if coordinates.x < 0 || coordinates.z < 0 || coordinates.dimension == 0 {
        return 0;
    }
    (u64::from(coordinates.dimension) - 1) * 1_000_000
        + coordinates.z as u64 * 1_000
        + coordinates.x as u64
        + 1
}

fn location_query(index: u64) -> String {
    format!("SELECT * FROM location_state WHERE location_state.chunk_index = {index}")
}

pub(super) async fn subscribe_applied(ctx: &DbConnection, query: String) -> anyhow::Result<()> {
    let (tx, rx) = oneshot::channel();
    let completion = Arc::new(Mutex::new(Some(tx)));
    let failed = completion.clone();
    ctx.subscription_builder()
        .on_applied(move |_| {
            if let Some(tx) = completion.lock().unwrap().take() {
                let _ = tx.send(Ok(()));
            }
        })
        .on_error(move |_, error| {
            if let Some(tx) = failed.lock().unwrap().take() {
                let _ = tx.send(Err(anyhow::anyhow!("Subscription failed: {error}")));
            }
        })
        .subscribe(query);
    rx.await
        .map_err(|_| anyhow::anyhow!("Subscription ended before applying"))?
}

async fn subscribe_rows<F, Fut>(region: &WorldRegionState, mut subscribe: F) -> anyhow::Result<()>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let min_x = i32::from(region.region_min_chunk_x);
    let min_z = i32::from(region.region_min_chunk_z);
    for x in min_x..min_x + i32::from(region.region_width_chunks) {
        let row = (min_z..min_z + i32::from(region.region_height_chunks))
            .map(|z| subscribe(chunk_index(ChunkCoordinatesMessage { x, z, dimension: 1 })));
        futures::future::try_join_all(row).await?;
    }
    Ok(())
}

/// Called alongside the SDK event loop, once per regional connection.
pub(super) async fn subscribe(ctx: &DbConnection) -> anyhow::Result<()> {
    // This application runs without the SDK row cache. Capture the initial
    // region rows directly instead of looking them up in ctx.db afterwards.
    let regions = Arc::new(Mutex::new(Vec::new()));
    let initial_regions = regions.clone();
    let table = ctx.db.world_region_state();
    let callback = table.on_initial_owned(move |ctx: &EventContext, rows| {
        if matches!(ctx.event, Event::SubscribeApplied) {
            initial_regions.lock().unwrap().extend(rows);
        }
    });
    let result = subscribe_applied(ctx, "SELECT * FROM world_region_state".to_owned()).await;
    table.remove_on_initial(callback);
    result?;
    let regions = std::mem::take(&mut *regions.lock().unwrap());
    anyhow::ensure!(
        regions.len() == 1,
        "Expected one world region, received {}",
        regions.len()
    );
    subscribe_rows(&regions[0], |index| {
        subscribe_applied(ctx, location_query(index))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_indices_match_coordinate_encoding() {
        for (x, z, dimension, expected) in [
            (0, 0, 1, 1),
            (12, 34, 1, 34_013),
            (12, 34, 2, 1_034_013),
            (-1, 0, 1, 0),
            (0, -1, 1, 0),
            (0, 0, 0, 0),
        ] {
            assert_eq!(
                chunk_index(ChunkCoordinatesMessage { x, z, dimension }),
                expected
            );
        }
        assert_eq!(
            location_query(34_013),
            "SELECT * FROM location_state WHERE location_state.chunk_index = 34013"
        );
    }

    fn region() -> WorldRegionState {
        WorldRegionState {
            id: 0,
            region_min_chunk_x: 2,
            region_min_chunk_z: 3,
            region_width_chunks: 2,
            region_height_chunks: 2,
            region_index: 0,
            region_count: 1,
            region_count_sqrt: 1,
        }
    }

    #[tokio::test]
    async fn chunks_use_separate_queries_and_exclusive_bounds() {
        let mut queries = Vec::new();
        subscribe_rows(&region(), |index| {
            queries.push(location_query(index));
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert_eq!(
            queries,
            [
                "SELECT * FROM location_state WHERE location_state.chunk_index = 3003",
                "SELECT * FROM location_state WHERE location_state.chunk_index = 4003",
                "SELECT * FROM location_state WHERE location_state.chunk_index = 3004",
                "SELECT * FROM location_state WHERE location_state.chunk_index = 4004",
            ]
        );
    }

    #[tokio::test]
    async fn failed_row_does_not_start_next_row() {
        let mut started = Vec::new();
        let result = subscribe_rows(&region(), |index| {
            started.push(index);
            std::future::ready(Err(anyhow::anyhow!("failed")))
        })
        .await;
        assert!(result.is_err());
        assert_eq!(started, [3003, 4003]);
    }

    #[tokio::test]
    async fn rows_wait_for_all_chunks_before_starting_the_next_row() {
        let completed = Arc::new(Mutex::new(Vec::new()));
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let region = region();
        let subscriptions = subscribe_rows(&region, |index| {
            let completed = completed.clone();
            let barrier = barrier.clone();
            async move {
                if index % 1000 == 4 {
                    assert_eq!(completed.lock().unwrap().len(), 2);
                }
                barrier.wait().await;
                completed.lock().unwrap().push(index);
                Ok(())
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), subscriptions)
            .await
            .expect("chunks within a row must run concurrently")
            .unwrap();
        let mut completed = completed.lock().unwrap().clone();
        completed.sort_unstable();
        assert_eq!(completed, [3003, 3004, 4003, 4004]);
    }

    #[tokio::test]
    async fn empty_region_does_not_issue_queries() {
        let mut called = false;
        let mut region = region();
        region.region_width_chunks = 0;
        subscribe_rows(&region, |_| {
            called = true;
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert!(!called);
    }
}
