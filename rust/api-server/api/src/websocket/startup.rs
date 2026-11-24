//! Pipeline one regional WebSocket handshake ahead of the active initial snapshot.
use std::{future::Future, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

/// Drive the connected SDK while the PostgreSQL baseline is loading. Only an
/// explicit success opens subscriptions, including for later reconnects.
pub(super) async fn wait_for_db(
    ready: &mut tokio::sync::watch::Receiver<bool>,
    connection: impl Future<Output = anyhow::Result<()>>,
    stop: &CancellationToken,
) -> anyhow::Result<bool> {
    tokio::pin!(connection);
    tokio::select! { biased;
        _ = stop.cancelled() => {}
        result = ready.wait_for(|loaded| *loaded) => {
            if result.is_ok() {
                return Ok(true);
            }
        }
        result = &mut connection => {
            result?;
            anyhow::ensure!(stop.is_cancelled(), "Connection ended while loading app state");
            return Ok(false);
        }
    }
    // Shutdown or loader failure: disconnect and drain without subscribing.
    stop.cancel();
    connection.await?;
    Ok(false)
}

pub(super) async fn wait_previous(
    previous: Option<tokio::sync::oneshot::Receiver<()>>,
    stop: &CancellationToken,
) -> bool {
    if let Some(previous) = previous {
        tokio::select! { biased;
            _ = stop.cancelled() => return false,
            _ = previous => {}
        }
    }
    !stop.is_cancelled()
}

pub(super) async fn acquire(
    gate: &Arc<Semaphore>,
    stop: &CancellationToken,
) -> Option<OwnedSemaphorePermit> {
    tokio::select! { biased;
        _ = stop.cancelled() => None,
        permit = gate.clone().acquire_owned() => permit.ok(),
    }
}

pub(super) async fn admit_snapshot(
    handshake: OwnedSemaphorePermit,
    snapshots: &Arc<Semaphore>,
    stop: &CancellationToken,
) -> Option<OwnedSemaphorePermit> {
    // Keep the handshake slot while waiting: otherwise every later region
    // could open an idle socket behind one slow snapshot.
    let snapshot = acquire(snapshots, stop).await;
    drop(handshake);
    snapshot
}

pub(super) async fn run<F, R>(
    connection: F,
    ready: R,
    stop: &CancellationToken,
    permit: OwnedSemaphorePermit,
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
                    drop(permit);
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
    use tokio::time::Duration;

    #[tokio::test]
    async fn preconnected_sockets_are_driven_but_wait_for_the_database_baseline() {
        let (loader, mut ready) = tokio::sync::watch::channel(false);
        let mut reconnect = ready.clone();
        let stop = CancellationToken::new();
        let (connected, handshake) = oneshot::channel();
        let driver = async {
            connected.send(()).unwrap();
            std::future::pending().await
        };
        let waiting = wait_for_db(&mut ready, driver, &stop);
        tokio::pin!(waiting);
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        handshake.await.unwrap();
        loader.send(true).unwrap();
        assert!(waiting.await.unwrap());
        drop(loader);
        assert!(
            wait_for_db(&mut reconnect, std::future::pending(), &stop)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn failed_baseline_drains_waiting_socket_without_subscribing() {
        let (loader, mut ready) = tokio::sync::watch::channel(false);
        let stop = CancellationToken::new();
        let driver = async {
            stop.cancelled().await;
            Ok(())
        };
        let waiting = wait_for_db(&mut ready, driver, &stop);
        tokio::pin!(waiting);
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        drop(loader);
        assert!(!waiting.await.unwrap());
        assert!(stop.is_cancelled());
    }

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
    async fn preconnect_one_region_while_previous_snapshot_is_loading() {
        let handshakes = Arc::new(Semaphore::new(1));
        let snapshots = Arc::new(Semaphore::new(1));
        let stop = CancellationToken::new();
        let first = acquire(&handshakes, &stop).await.unwrap();
        let loading = admit_snapshot(first, &snapshots, &stop).await.unwrap();
        let second = acquire(&handshakes, &stop).await.unwrap();
        let pending = admit_snapshot(second, &snapshots, &stop);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        assert!(
            handshakes.try_acquire().is_err(),
            "third handshake must wait"
        );
        drop(loading);
        let permit = match futures::poll!(pending.as_mut()) {
            std::task::Poll::Ready(Some(permit)) => permit,
            _ => panic!("startup must not add a delay after the slot is released"),
        };
        assert!(snapshots.try_acquire().is_err());
        assert_eq!(handshakes.available_permits(), 1);
        drop(permit);
        assert_eq!(snapshots.available_permits(), 1);
    }

    #[tokio::test]
    async fn callback_success_releases_slot_while_connection_keeps_running() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
        let (applied, ready) = oneshot::channel();
        let (disconnected, finished) = oneshot::channel();
        let stop = CancellationToken::new();
        let task = tokio::spawn(async move {
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
                permit,
            )
            .await
            .unwrap();
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
    async fn failed_subscription_drains_before_releasing_slot_for_other_regions() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        let mut drained = false;
        let error = run(
            async {
                stop.cancelled().await;
                assert!(gate.try_acquire().is_err());
                drained = true;
                Ok(())
            },
            async { anyhow::bail!("subscription rejected") },
            &stop,
            permit,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("subscription rejected"));
        assert!(drained);
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn early_disconnect_is_not_initial_success() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        assert!(
            run(async { Ok(()) }, std::future::pending(), &stop, permit)
                .await
                .is_err()
        );
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn sdk_driver_is_polled_while_waiting_for_its_callback() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
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
            permit,
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
    async fn shutdown_releases_handshake_while_waiting_for_snapshot() {
        let handshakes = Arc::new(Semaphore::new(1));
        let gate = Arc::new(Semaphore::new(1));
        let previous = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        let handshake = acquire(&handshakes, &stop).await.unwrap();
        let pending = admit_snapshot(handshake, &gate, &stop);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        stop.cancel();
        assert!(pending.await.is_none());
        assert_eq!(handshakes.available_permits(), 1);
        drop(previous);

        assert!(acquire(&gate, &stop).await.is_none());
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn shutdown_before_initial_callback_drains_without_reporting_failure() {
        let gate = Arc::new(Semaphore::new(1));
        let permit = gate.clone().acquire_owned().await.unwrap();
        let stop = CancellationToken::new();
        stop.cancel();
        run(async { Ok(()) }, std::future::pending(), &stop, permit)
            .await
            .unwrap();
    }
}
