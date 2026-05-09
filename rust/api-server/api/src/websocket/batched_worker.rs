use super::{SpacetimeUpdateMessages, record_worker_received};
use std::future::Future;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Duration;
use tokio::time::sleep;

pub(crate) trait BatchedWorker<T> {
    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<T>>;

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<T>>;

    fn start(self)
    where
        Self: Sized + Send + 'static,
        T: Send + 'static,
    {
        tokio::spawn(async move {
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
        msg: SpacetimeUpdateMessages<T>,
    ) -> impl Future<Output = ()> + Send;

    fn flush(&mut self) -> impl Future<Output = ()> + Send;
}

pub(crate) async fn run_batched_worker<T, W>(worker: &mut W)
where
    T: Send + 'static,
    W: BatchedWorker<T> + Send,
{
    loop {
        let timer = sleep(worker.batch_delay());
        tokio::pin!(timer);

        loop {
            tokio::select! {
                Some(msg) = worker.rx().recv() => {
                    record_worker_received(worker.worker_name(), 1);
                    worker.handle_message(msg).await;
                    if worker.should_flush() {
                        break;
                    }
                }
                _ = &mut timer => {
                    break;
                }
                else => {
                    break;
                }
            }
        }

        worker.flush().await;
        worker.reset_batch();

        if worker.is_idle() && worker.rx().is_closed() {
            break;
        }
    }
}
