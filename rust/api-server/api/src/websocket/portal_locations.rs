use game_module::module_bindings::*;
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::collections::HashSet;
use std::future::Future;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

fn location_query(dimension: u32) -> String {
    format!("SELECT * FROM location_state WHERE location_state.dimension = {dimension}")
}

/// Register before the existing portal_state subscription is sent, so its
/// initial snapshot cannot arrive before we start listening. No SDK cache needed.
pub(super) fn listen(ctx: &DbConnection) -> UnboundedReceiver<u32> {
    let (tx, rx) = unbounded_channel();
    let table = ctx.db.portal_state();
    let initial = tx.clone();
    table.on_initial(move |_, rows| {
        for portal in rows {
            let _ = initial.send(portal.destination_dimension);
        }
    });
    let inserted = tx.clone();
    table.on_insert(move |ctx, portal| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            let _ = inserted.send(portal.destination_dimension);
        }
    });
    table.on_update(move |_, old, new| {
        if old.destination_dimension != new.destination_dimension {
            let _ = tx.send(new.destination_dimension);
        }
    });
    rx
}

async fn subscribe_dimensions<F, Fut>(
    mut dimensions: UnboundedReceiver<u32>,
    mut subscribe: F,
) -> anyhow::Result<()>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut subscribed = HashSet::new();
    while let Some(dimension) = dimensions.recv().await {
        // Zero is invalid; overworld locations already use chunk subscriptions.
        // Keep previously discovered destinations subscribed for this connection.
        if dimension > 1 && !subscribed.contains(&dimension) {
            subscribe(dimension).await?;
            // Only applied subscriptions count as subscribed. Repeated portals
            // and later updates cannot resubscribe a dimension on this connection.
            subscribed.insert(dimension);
        }
    }
    Ok(())
}

/// Drive alongside the SDK event loop for the lifetime of this connection.
pub(super) async fn subscribe(
    ctx: &DbConnection,
    dimensions: UnboundedReceiver<u32>,
) -> anyhow::Result<()> {
    subscribe_dimensions(dimensions, |dimension| async move {
        super::overworld::subscribe_applied(ctx, location_query(dimension)).await?;
        tracing::info!(dimension, "Subscribed to portal destination dimension");
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn destinations_are_unique_and_exclude_invalid_and_overworld_dimensions() {
        let (tx, rx) = unbounded_channel();
        for dimension in [0, 1, 42, 42, 7, 1, 7, u32::MAX] {
            tx.send(dimension).unwrap();
        }
        drop(tx);
        let mut subscribed = Vec::new();
        subscribe_dimensions(rx, |dimensions| {
            subscribed.push(dimensions);
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert_eq!(subscribed, [42, 7, u32::MAX]);
        assert_eq!(
            location_query(42),
            "SELECT * FROM location_state WHERE location_state.dimension = 42"
        );
    }

    #[tokio::test]
    async fn subscribes_to_new_destinations_after_the_initial_snapshot() {
        let (tx, rx) = unbounded_channel();
        tx.send(42).unwrap();
        let mut subscribed = Vec::new();
        let mut tx = Some(tx);
        subscribe_dimensions(rx, |dimensions| {
            subscribed.push(dimensions);
            if let Some(tx) = tx.take() {
                // Simulate a later portal insert/update after the first
                // subscription has applied, rather than only startup discovery.
                tx.send(43).unwrap();
                tx.send(42).unwrap();
            }
            std::future::ready(Ok(()))
        })
        .await
        .unwrap();
        assert_eq!(subscribed, [42, 43]);
    }

    #[tokio::test]
    async fn subscription_failures_propagate_without_starting_the_next_dimension() {
        let (tx, rx) = unbounded_channel();
        for dimension in 2..207 {
            tx.send(dimension).unwrap();
        }
        drop(tx);
        let mut subscribed = Vec::new();
        let result = subscribe_dimensions(rx, |dimensions| {
            subscribed.push(dimensions);
            std::future::ready(Err(anyhow::anyhow!("subscription failed")))
        })
        .await;
        assert!(result.is_err());
        assert_eq!(subscribed, [2]);
    }

    #[tokio::test]
    async fn each_unique_destination_waits_for_the_previous_subscription() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (tx, rx) = unbounded_channel();
        for dimension in 2..207 {
            for value in [0, 1, dimension, dimension] {
                tx.send(value).unwrap();
            }
        }
        drop(tx);
        let completed = AtomicUsize::new(0);
        let mut subscribed = Vec::new();
        subscribe_dimensions(rx, |dimension| {
            assert_eq!(completed.load(Ordering::SeqCst), dimension as usize - 2);
            subscribed.push(dimension);
            let completed = &completed;
            async move {
                tokio::task::yield_now().await;
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .await
        .unwrap();
        assert_eq!(completed.load(Ordering::SeqCst), 205);
        assert_eq!(subscribed, (2..207).collect::<Vec<_>>());
    }
}
