use super::worker_channel::{UnboundedReceiver, UnboundedSender};
use super::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use std::future::Future;
use std::pin::Pin;
use tokio::time::Duration;
use tokio::time::{Instant, Sleep, sleep_until};

/// Event writers consume either a bounded Tokio queue or a shutdown-aware worker queue.
pub(crate) trait DbBatchInput<T> {
    fn collect_ready(
        &mut self,
        batch: &mut Vec<T>,
        limit: usize,
    ) -> impl Future<Output = usize> + Send;
}

impl<T: Send + 'static> DbBatchInput<T> for UnboundedReceiver<T> {
    async fn collect_ready(&mut self, batch: &mut Vec<T>, limit: usize) -> usize {
        self.recv_many(batch, limit).await
    }
}

impl<T: Send> DbBatchInput<T> for tokio::sync::mpsc::Receiver<T> {
    async fn collect_ready(&mut self, batch: &mut Vec<T>, limit: usize) -> usize {
        self.recv_many(batch, limit).await
    }
}

/// Drain ready rows, then collect until the first batch's deadline, size cap,
/// or end of input. An idle writer has no timer.
pub(crate) async fn collect_db_batch<T>(
    rx: &mut impl DbBatchInput<T>,
    batch: &mut Vec<T>,
    limit: usize,
    delay: Duration,
) -> usize {
    debug_assert!(batch.is_empty());
    let count = rx.collect_ready(batch, limit).await;
    if count == 0 || count == limit || delay.is_zero() {
        return count;
    }
    let timeout = tokio::time::sleep(delay);
    tokio::pin!(timeout);
    while batch.len() < limit {
        let remaining = limit - batch.len();
        tokio::select! {
            biased;
            _ = &mut timeout => break,
            n = rx.collect_ready(batch, remaining) => if n == 0 { break },
        }
    }
    batch.len()
}

/// An idle batch has no timer. The first message starts one fixed window;
/// subsequent messages do not extend it and starve the flush.
pub(crate) struct BatchTimer {
    delay: Duration,
    deadline: Option<Instant>,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl BatchTimer {
    pub(crate) fn new(delay: Duration) -> Self {
        Self {
            delay,
            deadline: None,
            sleep: None,
        }
    }

    pub(crate) fn start(&mut self) {
        if self.deadline.is_none() {
            let deadline = Instant::now() + self.delay;
            self.deadline = Some(deadline);
            self.sleep = Some(Box::pin(sleep_until(deadline)));
        }
    }

    pub(crate) fn is_due(&self) -> bool {
        self.deadline.is_some_and(|end| Instant::now() >= end)
    }

    pub(crate) async fn elapsed(&mut self) {
        // Cancelling this select branch drops only a borrow, not the registered
        // Sleep. It is pinned once per active batch and never reset by messages.
        match self.sleep.as_mut() {
            Some(sleep) => sleep.as_mut().await,
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
    let receive_metrics = WorkerReceiveMetrics::new(worker.worker_name());
    loop {
        let mut timer = BatchTimer::new(worker.batch_delay());

        loop {
            tokio::select! {
                msg = worker.rx().recv() => {
                    let Some(msg) = msg else { break; };
                    timer.start();
                    receive_metrics.record(1);
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

    #[tokio::test(start_paused = true)]
    async fn event_batch_deadline_starts_at_first_row_and_does_not_slide() {
        let (tx, mut rx) = unbounded_channel();
        let mut batch = Vec::new();
        let delay = Duration::from_secs(2);
        {
            let receive = collect_db_batch(&mut rx, &mut batch, 10, delay);
            tokio::pin!(receive);
            assert!(futures::poll!(&mut receive).is_pending());
            tokio::time::advance(delay * 3).await;
            tx.send(1).unwrap();
            assert!(futures::poll!(&mut receive).is_pending());
            tokio::time::advance(delay / 2).await;
            tx.send(2).unwrap();
            assert!(futures::poll!(&mut receive).is_pending());
            tokio::time::advance(delay / 2).await;
            assert_eq!(futures::poll!(&mut receive), std::task::Poll::Ready(2));
        }
        assert_eq!(batch, [1, 2]);
    }

    #[tokio::test]
    async fn full_event_batch_returns_with_sender_still_alive() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        for row in 1..=3 {
            tx.send(row).await.unwrap();
        }
        let mut batch = Vec::new();
        assert_eq!(
            futures::poll!(Box::pin(collect_db_batch(
                &mut rx,
                &mut batch,
                2,
                Duration::from_secs(60)
            ))),
            std::task::Poll::Ready(2)
        );
        assert_eq!(batch, [1, 2]);
        assert_eq!(rx.try_recv().unwrap(), 3);
    }

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
            // Keep the worker's reusable allocation across batch flushes.
            self.flushed.send(self.messages.split_off(0)).unwrap();
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
        assert!(timer.sleep.is_none());
        assert!(futures::poll!(Box::pin(timer.elapsed())).is_pending());
        assert!(
            timer.sleep.is_none(),
            "idle batches do not allocate a Sleep"
        );
        timer.start();
        let deadline = timer.deadline;
        timer.start();
        assert_eq!(timer.deadline, deadline);
    }

    #[tokio::test]
    async fn cancelling_elapsed_reuses_one_pinned_sleep_and_fixed_deadline() {
        let mut timer = BatchTimer::new(Duration::from_secs(60));
        timer.start();
        let deadline = timer.deadline;
        let allocation = timer.sleep.as_ref().unwrap().as_ref().get_ref() as *const Sleep;
        for _ in 0..128 {
            // Each poll drops a pending elapsed future, like a select whose
            // receive branch won. The underlying timer must survive unchanged.
            assert!(futures::poll!(Box::pin(timer.elapsed())).is_pending());
            timer.start();
            assert_eq!(timer.deadline, deadline);
            assert_eq!(
                timer.sleep.as_ref().unwrap().as_ref().get_ref() as *const Sleep,
                allocation
            );
        }
    }

    #[tokio::test]
    async fn messages_after_deadline_do_not_rearm_the_timer() {
        let mut timer = BatchTimer::new(Duration::from_millis(5));
        timer.start();
        let deadline = timer.deadline.unwrap();
        tokio::time::sleep_until(deadline).await;
        timer.start();
        assert_eq!(timer.deadline, Some(deadline));
        tokio::time::timeout(Duration::from_secs(1), timer.elapsed())
            .await
            .unwrap();
        assert!(timer.sleep.as_ref().unwrap().is_elapsed());
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
        assert!(worker.messages.capacity() > 0); // flush preserved the allocation
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
