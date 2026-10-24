use super::worker_channel::{UnboundedReceiver, UnboundedSender};
use super::{SpacetimeUpdateMessages, record_worker_received};
use std::future::Future;
use tokio::time::Duration;
use tokio::time::{Instant, sleep_until};

/// An idle batch has no timer. The first message starts one fixed window;
/// subsequent messages do not extend it and starve the flush.
pub(crate) struct BatchTimer {
    delay: Duration,
    deadline: Option<Instant>,
}

impl BatchTimer {
    pub(crate) fn new(delay: Duration) -> Self {
        Self {
            delay,
            deadline: None,
        }
    }

    pub(crate) fn start(&mut self) {
        self.deadline
            .get_or_insert_with(|| Instant::now() + self.delay);
    }

    pub(crate) async fn elapsed(&self) {
        match self.deadline {
            Some(deadline) => sleep_until(deadline).await,
            None => std::future::pending().await,
        }
    }
}

pub(crate) trait BatchedWorker {
    type Entity;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>>;

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>>;

    fn start(self, shutdown: &crate::shutdown::Shutdown)
    where
        Self: Sized + Send + 'static,
        Self::Entity: Send + 'static,
    {
        shutdown.spawn_worker(async move {
            let mut worker = self;
            run_batched_worker(&mut worker).await;
        });
    }

    fn worker_name(&self) -> &'static str;

    fn batch_delay(&self) -> Duration;

    fn should_flush(&self) -> bool;

    fn is_idle(&self) -> bool;

    fn reset_batch(&mut self);

    fn handle_message(
        &mut self,
        msg: SpacetimeUpdateMessages<Self::Entity>,
    ) -> impl Future<Output = ()> + Send;

    fn flush(&mut self) -> impl Future<Output = ()> + Send;
}

pub(crate) async fn run_batched_worker<W>(worker: &mut W)
where
    W: BatchedWorker + Send,
    W::Entity: Send + 'static,
{
    tracing::info!(worker = worker.worker_name(), "Worker started");
    loop {
        let mut timer = BatchTimer::new(worker.batch_delay());

        loop {
            tokio::select! {
                msg = worker.rx().recv() => {
                    let Some(msg) = msg else { break; };
                    timer.start();
                    record_worker_received(worker.worker_name(), 1);
                    worker.handle_message(msg).await;
                    if worker.should_flush() {
                        break;
                    }
                }
                _ = timer.elapsed() => {
                    break;
                }
                else => {
                    break;
                }
            }
        }

        worker.flush().await;
        worker.reset_batch();

        if worker.is_idle() && worker.rx().is_closed() && worker.rx().is_empty() {
            tracing::warn!(
                worker = worker.worker_name(),
                "Worker is idle and worker closed, exiting"
            );
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::worker_channel::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    use super::*;

    struct TestWorker {
        rx: UnboundedReceiver<SpacetimeUpdateMessages<u8>>,
        flushed: UnboundedSender<Vec<u8>>,
        messages: Vec<u8>,
        delay: Duration,
        limit: usize,
    }

    impl BatchedWorker for TestWorker {
        type Entity = u8;
        fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<u8>> {
            &mut self.rx
        }
        fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<u8>> {
            panic!("not needed by runner")
        }
        fn worker_name(&self) -> &'static str {
            "test_worker"
        }
        fn batch_delay(&self) -> Duration {
            self.delay
        }
        fn should_flush(&self) -> bool {
            self.messages.len() >= self.limit
        }
        fn is_idle(&self) -> bool {
            self.messages.is_empty()
        }
        fn reset_batch(&mut self) {}
        async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<u8>) {
            let SpacetimeUpdateMessages::Insert { new, .. } = msg else {
                panic!("unexpected message")
            };
            self.messages.push(new);
        }
        async fn flush(&mut self) {
            self.flushed
                .send(self.messages.drain(..).collect())
                .unwrap();
        }
    }

    fn message(new: u8) -> SpacetimeUpdateMessages<u8> {
        SpacetimeUpdateMessages::Insert {
            event: None,
            new,
            database_name: 1,
        }
    }

    #[tokio::test]
    async fn batch_timer_is_unarmed_until_first_message_and_does_not_extend() {
        let mut timer = BatchTimer::new(Duration::from_secs(60));
        assert!(timer.deadline.is_none());
        assert!(futures::poll!(Box::pin(timer.elapsed())).is_pending());
        timer.start();
        let deadline = timer.deadline;
        timer.start();
        assert_eq!(timer.deadline, deadline);
    }

    #[tokio::test]
    async fn idle_workers_do_not_flush_before_first_message_or_between_batches() {
        let (tx, rx) = unbounded_channel();
        let (flushed, mut batches) = unbounded_channel();
        let mut worker = TestWorker {
            rx,
            flushed,
            messages: Vec::new(),
            delay: Duration::from_millis(5),
            limit: 10,
        };
        assert_eq!(worker.messages.capacity(), 0);
        let task = tokio::spawn(async move {
            run_batched_worker(&mut worker).await;
            worker
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), batches.recv())
                .await
                .is_err()
        );
        tx.send(message(1)).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), batches.recv())
                .await
                .unwrap(),
            Some(vec![1])
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(25), batches.recv())
                .await
                .is_err()
        );
        tx.send(message(2)).unwrap();
        drop(tx);
        let worker = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(batches.recv().await, Some(vec![2]));
        assert!(worker.messages.is_empty());
        assert!(worker.messages.capacity() > 0); // drain preserved the allocation
    }

    #[tokio::test]
    async fn closed_channel_drains_all_batches_and_flushes_partial_batch_without_waiting() {
        let (tx, rx) = unbounded_channel();
        let (flushed, mut batches) = unbounded_channel();
        for value in 0..5 {
            tx.send(message(value)).unwrap();
        }
        drop(tx);
        let mut worker = TestWorker {
            rx,
            flushed,
            messages: Vec::new(),
            delay: Duration::from_secs(60),
            limit: 2,
        };
        tokio::time::timeout(Duration::from_secs(1), run_batched_worker(&mut worker))
            .await
            .unwrap();
        assert_eq!(batches.try_recv().unwrap(), vec![0, 1]);
        assert_eq!(batches.try_recv().unwrap(), vec![2, 3]);
        assert_eq!(batches.try_recv().unwrap(), vec![4]);
        assert!(batches.try_recv().is_err());
    }

    #[tokio::test]
    async fn size_threshold_flushes_without_waiting_for_deadline() {
        let (tx, rx) = unbounded_channel();
        let (flushed, mut batches) = unbounded_channel();
        let mut worker = TestWorker {
            rx,
            flushed,
            messages: Vec::new(),
            delay: Duration::from_secs(60),
            limit: 2,
        };
        let task = tokio::spawn(async move {
            run_batched_worker(&mut worker).await;
        });
        tx.send(message(1)).unwrap();
        tx.send(message(2)).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), batches.recv())
                .await
                .unwrap(),
            Some(vec![1, 2])
        );
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }
}
