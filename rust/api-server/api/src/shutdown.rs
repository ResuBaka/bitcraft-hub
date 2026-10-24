//! Stop producers first, then seal and drain worker input, then DB writers.
use crate::websocket::worker_channel::ChannelControl;
use parking_lot::Mutex;
use std::{
    future::Future,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[derive(Clone, Default)]
pub(crate) struct Shutdown {
    pub(crate) stop: CancellationToken,
    pub(crate) broadcast_stop: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    workers: TaskTracker,
    pub(crate) clients: TaskTracker,
    inputs: Arc<Mutex<Vec<Weak<ChannelControl>>>>,
}

impl Shutdown {
    pub(crate) fn register_input(&self, control: &Arc<ChannelControl>) {
        self.inputs.lock().push(Arc::downgrade(control));
    }

    pub(crate) fn spawn_connection(
        &self,
        task: impl Future<Output = ()> + Send + 'static,
    ) -> JoinHandle<()> {
        self.connections.spawn(self.run_until_forced(task))
    }

    pub(crate) fn spawn_worker(
        &self,
        task: impl Future<Output = ()> + Send + 'static,
    ) -> JoinHandle<()> {
        self.workers.spawn(self.run_until_forced(task))
    }

    fn run_until_forced(
        &self,
        task: impl Future<Output = ()> + Send + 'static,
    ) -> impl Future<Output = ()> + Send + 'static {
        let force = self.force.clone();
        async move {
            tokio::select! { biased;
                _ = force.cancelled() => {},
                _ = task => {},
            }
        }
    }

    pub(crate) async fn drain(&self) {
        self.stop.cancel();
        self.connections.close();
        self.connections.wait().await;
        tracing::info!("SpacetimeDB producers stopped; sealing worker input queues");
        // Every producer has returned, including initial snapshot callbacks.
        // Self-owned senders must not keep workers alive indefinitely.
        self.seal_inputs();
        self.workers.close();
        // Includes database writers and workers spawned by tracked workers.
        self.workers.wait().await;
        tracing::info!("Worker and database-write queues drained");
        self.broadcast_stop.cancel();
        self.clients.close();
        self.clients.wait().await;
    }

    pub(crate) async fn force_stop(&self) {
        self.stop.cancel();
        self.force.cancel();
        self.seal_inputs();
        self.broadcast_stop.cancel();
        self.connections.close();
        self.workers.close();
        self.connections.wait().await;
        self.workers.wait().await;
    }

    fn seal_inputs(&self) {
        for control in self
            .inputs
            .lock()
            .drain(..)
            .filter_map(|weak| weak.upgrade())
        {
            control.close();
        }
    }
}

pub(crate) async fn wait_for_trigger(after: Option<Duration>) -> &'static str {
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result { tracing::error!(%error, "Could not listen for Ctrl+C"); }
            "SIGINT"
        },
        _ = terminate_signal() => "SIGTERM",
        _ = deadline(after) => "run duration elapsed",
    }
}

async fn deadline(after: Option<Duration>) {
    match after {
        Some(duration) => tokio::time::sleep(duration).await,
        None => std::future::pending().await,
    }
}

async fn terminate_signal() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "Could not listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::worker_channel::shutdown_channel;

    #[tokio::test]
    async fn stops_producers_then_drains_inputs_and_database_writers_with_self_owned_senders() {
        let shutdown = Shutdown::default();
        let (tx, mut rx) = shutdown_channel(&shutdown);
        let self_sender = tx.clone();
        tx.send(1).unwrap();
        let stop = shutdown.stop.clone();
        shutdown.spawn_connection(async move {
            stop.cancelled().await;
            tx.send(2).unwrap(); // accepted before producer finishes
        });
        let (writes, mut database) = tokio::sync::mpsc::channel(1);
        let saved = Arc::new(Mutex::new(Vec::new()));
        let observed = saved.clone();
        shutdown.spawn_worker(async move {
            while let Some(value) = database.recv().await {
                observed.lock().push(value);
            }
        });
        shutdown.spawn_worker(async move {
            while let Some(value) = rx.recv().await {
                writes.send(value).await.unwrap();
            }
            assert!(self_sender.send(3).is_err());
        });
        tokio::time::timeout(Duration::from_secs(2), shutdown.drain())
            .await
            .unwrap();
        assert_eq!(*saved.lock(), [1, 2]);
        assert!(shutdown.broadcast_stop.is_cancelled());
    }

    #[tokio::test]
    async fn forced_stop_cancels_and_drops_stalled_tasks() {
        let shutdown = Shutdown::default();
        let (tx, _rx) = shutdown_channel::<u8>(&shutdown);
        let (ready, started) = tokio::sync::oneshot::channel();
        let (dropped, gone) = tokio::sync::oneshot::channel();
        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }
        shutdown.spawn_worker(async move {
            let _guard = NotifyDrop(Some(dropped));
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), shutdown.drain())
                .await
                .is_err()
        );
        shutdown.force_stop().await;
        gone.await.unwrap();
        assert!(tx.send(1).is_err());
    }

    #[tokio::test]
    async fn deadline_can_be_disabled_or_immediate() {
        assert!(futures::poll!(Box::pin(deadline(None))).is_pending());
        deadline(Some(Duration::ZERO)).await;
    }
}
