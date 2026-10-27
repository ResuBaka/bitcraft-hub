//! Hold the shared connection-build slot through the initial snapshot (and any
//! failed attempts), but never for the lifetime of an established connection.
use std::{future::Future, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

pub(super) async fn wait_previous(
    previous: Option<tokio::sync::oneshot::Receiver<()>>,
    stop: &CancellationToken,
) -> bool {
    if let Some(previous) = previous {
        tokio::select! { biased;
            _ = stop.cancelled() => return false,
            result = previous => {
                if result.is_err() {
                    // Preserve the existing finite retry budget: once a region
                    // gives up entirely, allow later regions to make progress.
                    tracing::warn!("Previous connection exhausted startup attempts; continuing with the next database");
                }
            }
        }
    }
    !stop.is_cancelled()
}

pub(super) async fn acquire(
    gate: &Arc<Semaphore>,
    stop: &CancellationToken,
    delay: Duration,
) -> Option<OwnedSemaphorePermit> {
    let permit = tokio::select! { biased;
        _ = stop.cancelled() => return None,
        permit = gate.clone().acquire_owned() => permit.ok()?,
    };
    // The delay starts only after the preceding connection finished its initial
    // subscription. Waiting tasks must not consume their delay in parallel.
    tokio::select! { biased;
        _ = stop.cancelled() => None,
        _ = tokio::time::sleep(delay) => Some(permit),
    }
}

pub(super) async fn run<F, R>(
    connection: F,
    ready: R,
    stop: &CancellationToken,
    permit: &mut Option<OwnedSemaphorePermit>,
) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
    R: Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(connection);
    tokio::select! {
        result = &mut connection => {
            result?;
            anyhow::ensure!(stop.is_cancelled(), "Connection ended before its initial subscription applied");
            Ok(())
        }
        result = ready => {
            match result {
                Ok(()) => {
                    // The caller keeps retrying with this same slot on failure.
                    // Only success lets the next database begin building up.
                    drop(permit.take());
                    connection.await
                }
                Err(error) => {
                    // Continue driving the *same* SDK runner to disconnect; do
                    // not leave a failed subscription running alongside a retry.
                    stop.cancel();
                    if let Err(disconnect_error) = connection.await {
                        tracing::warn!(%disconnect_error, "Failed to close connection after initial subscription error");
                    }
                    Err(error)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn initial_connections_wait_for_the_previous_configured_database() {
        let stop = CancellationToken::new();
        assert!(wait_previous(None, &stop).await);
        let (completed, previous) = oneshot::channel();
        let waiting = wait_previous(Some(previous), &stop);
        tokio::pin!(waiting);
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        completed.send(()).unwrap();
        assert!(waiting.await);

        let (exhausted, previous) = oneshot::channel();
        drop(exhausted);
        assert!(wait_previous(Some(previous), &stop).await);
        let (_pending, previous) = oneshot::channel();
        stop.cancel();
        assert!(!wait_previous(Some(previous), &stop).await);
    }

    #[tokio::test]
    async fn startup_delay_begins_after_previous_slot_is_released() {
        let gate = Arc::new(Semaphore::new(1));
        let previous = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        let delay = Duration::from_millis(20);
        let pending = acquire(&gate, &stop, delay);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        tokio::time::sleep(delay).await;
        assert!(futures::poll!(pending.as_mut()).is_pending());
        let released = tokio::time::Instant::now();
        drop(previous);
        let permit = pending.await.unwrap();
        assert!(released.elapsed() >= delay);
        assert!(gate.try_acquire().is_err());
        drop(permit);
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn callback_success_releases_slot_while_connection_keeps_running() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
        let (applied, ready) = oneshot::channel();
        let (disconnected, finished) = oneshot::channel();
        let stop = CancellationToken::new();
        let task = tokio::spawn(async move {
            let mut permit = Some(permit);
            run(
                async {
                    finished.await?;
                    Ok(())
                },
                async {
                    ready.await?;
                    Ok(())
                },
                &stop,
                &mut permit,
            )
            .await
            .unwrap();
            assert!(permit.is_none());
        });
        assert!(gate.try_acquire().is_err());
        applied.send(()).unwrap();
        let next = tokio::time::timeout(Duration::from_secs(1), gate.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap();
        assert!(
            !task.is_finished(),
            "startup must not wait for the established connection to end"
        );
        disconnected.send(()).unwrap();
        task.await.unwrap();
        drop(next);
    }

    #[tokio::test]
    async fn failed_subscription_drains_connection_and_retains_slot_for_retry() {
        let gate = Arc::new(Semaphore::new(1));
        let mut permit = Some(gate.clone().acquire_owned().await.unwrap());
        let stop = CancellationToken::new();
        let mut drained = false;
        let error = run(
            async {
                stop.cancelled().await;
                drained = true;
                Ok(())
            },
            async { anyhow::bail!("subscription rejected") },
            &stop,
            &mut permit,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("subscription rejected"));
        assert!(drained);
        assert!(permit.is_some());
        assert!(gate.try_acquire().is_err());
        // A subsequent successful attempt, not the failure, releases the slot.
        let retry_stop = CancellationToken::new();
        let retry = run(
            std::future::pending(),
            async { Ok(()) },
            &retry_stop,
            &mut permit,
        );
        tokio::pin!(retry);
        assert!(futures::poll!(retry.as_mut()).is_pending());
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn early_disconnect_is_not_initial_success() {
        let gate = Arc::new(Semaphore::new(1));
        let mut permit = Some(gate.clone().acquire_owned().await.unwrap());
        let stop = CancellationToken::new();
        assert!(
            run(async { Ok(()) }, std::future::pending(), &stop, &mut permit)
                .await
                .is_err()
        );
        assert!(permit.is_some());
        assert!(gate.try_acquire().is_err());
    }

    #[tokio::test]
    async fn sdk_driver_is_polled_while_waiting_for_its_callback() {
        let gate = Arc::new(Semaphore::new(1));
        let mut permit = Some(gate.clone().acquire_owned().await.unwrap());
        let stop = CancellationToken::new();
        let (applied, ready) = oneshot::channel();
        let driven = std::cell::Cell::new(false);
        let driver = async {
            driven.set(true);
            applied.send(()).unwrap();
            stop.cancelled().await;
            Ok(())
        };
        let connection = run(
            driver,
            async {
                ready.await?;
                Ok(())
            },
            &stop,
            &mut permit,
        );
        tokio::pin!(connection);
        assert!(futures::poll!(connection.as_mut()).is_pending());
        // run's select may have polled readiness before the driver sent it.
        assert!(futures::poll!(connection.as_mut()).is_pending());
        assert_eq!(gate.available_permits(), 1);
        stop.cancel();
        connection.await.unwrap();
        assert!(driven.get());
    }

    #[tokio::test]
    async fn shutdown_cancels_both_slot_wait_and_startup_delay() {
        let gate = Arc::new(Semaphore::new(1));
        let previous = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        let pending = acquire(&gate, &stop, Duration::from_secs(60));
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        stop.cancel();
        assert!(pending.await.is_none());
        drop(previous);

        let stop = CancellationToken::new();
        let pending = acquire(&gate, &stop, Duration::from_secs(60));
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        assert_eq!(gate.available_permits(), 0);
        stop.cancel();
        assert!(pending.await.is_none());
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn shutdown_before_initial_callback_drains_without_reporting_failure() {
        let gate = Arc::new(Semaphore::new(1));
        let mut permit = Some(gate.clone().acquire_owned().await.unwrap());
        let stop = CancellationToken::new();
        stop.cancel();
        run(async { Ok(()) }, std::future::pending(), &stop, &mut permit)
            .await
            .unwrap();
    }
}
