use super::{SpacetimeUpdateMessages, record_worker_received};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration;
use tokio::time::sleep;

pub(crate) trait BatchedWorker<T> {
    fn worker_name(&self) -> &'static str;

    fn batch_delay(&self) -> Duration;

    fn should_flush(&self) -> bool;

    fn is_idle(&self) -> bool;

    fn reset_batch(&mut self);

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<T>);

    async fn flush(&mut self);
}

pub(crate) async fn run_batched_worker<T, W>(
    worker: &mut W,
    rx: &mut UnboundedReceiver<SpacetimeUpdateMessages<T>>,
) where
    T: Send + 'static,
    W: BatchedWorker<T> + Send,
{
    loop {
        let timer = sleep(worker.batch_delay());
        tokio::pin!(timer);

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
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

        if worker.is_idle() && rx.is_closed() {
            break;
        }
    }
}
