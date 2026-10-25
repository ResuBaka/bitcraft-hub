//! Kanal backend for unbounded worker-input queues in the RSS comparison.
//! Kanal 0.1.1 receive futures are NOT cancellation-safe in `tokio::select!`.
//! The persistent unfold stream owns each in-progress receive across cancelled
//! calls to `recv`, rather than constructing/dropping a Kanal receive per select.
use futures::{Stream, StreamExt, stream};
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll};

pub(crate) struct UnboundedSender<T> {
    inner: kanal::Sender<T>,
    outstanding: Arc<AtomicUsize>,
}

pub(crate) struct UnboundedReceiver<T> {
    inner: kanal::Receiver<T>,
    // Mutex makes the receiver Sync for workers with async &self helpers.
    // Receive methods have &mut self and use get_mut(), with no lock per poll.
    stream: parking_lot::Mutex<Pin<Box<dyn Stream<Item = T> + Send>>>,
    outstanding: Arc<AtomicUsize>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TryRecvError {
    Empty,
    Disconnected,
}

pub(crate) fn unbounded_channel<T: Send + 'static>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = kanal::unbounded();
    let async_rx = rx.clone().to_async();
    let stream = stream::unfold(async_rx, |rx| async move {
        let value = rx.recv().await.ok()?;
        Some((value, rx))
    })
    .fuse();
    let outstanding = Arc::new(AtomicUsize::new(0));
    (
        UnboundedSender {
            inner: tx,
            outstanding: Arc::clone(&outstanding),
        },
        UnboundedReceiver {
            inner: rx,
            stream: parking_lot::Mutex::new(Box::pin(stream)),
            outstanding,
        },
    )
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            outstanding: Arc::clone(&self.outstanding),
        }
    }
}

impl<T> UnboundedSender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), kanal::SendError> {
        // Count before publishing: a receiver may consume immediately. Includes
        // values held by an in-progress Kanal receive, not just its inner queue.
        self.outstanding.fetch_add(1, Ordering::Relaxed);
        let result = self.inner.send(value);
        if result.is_err() {
            self.outstanding.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }
}

impl<T: Send + 'static> UnboundedReceiver<T> {
    pub(crate) async fn recv(&mut self) -> Option<T> {
        // Kanal does not participate in Tokio's cooperative scheduling. Without
        // this, a worker with a continuously ready queue can run indefinitely
        // without yielding, starving other workers and timers on its thread.
        // Keep the receive itself in the persistent stream for cancellation safety.
        let value = tokio::task::coop::cooperative(self.stream.get_mut().next()).await;
        if value.is_some() {
            self.outstanding.fetch_sub(1, Ordering::Relaxed);
        }
        value
    }

    pub(crate) fn try_recv(&mut self) -> Result<T, TryRecvError> {
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        match self.stream.get_mut().as_mut().poll_next(&mut context) {
            Poll::Ready(Some(value)) => {
                self.outstanding.fetch_sub(1, Ordering::Relaxed);
                Ok(value)
            }
            Poll::Ready(None) => Err(TryRecvError::Disconnected),
            Poll::Pending => Err(TryRecvError::Empty),
        }
    }

    #[cfg(test)]
    pub(crate) async fn recv_many(&mut self, buffer: &mut Vec<T>, limit: usize) -> usize {
        if limit == 0 {
            return 0;
        }
        let Some(first) = self.recv().await else {
            return 0;
        };
        buffer.push(first);
        let mut received = 1;
        while received < limit {
            match self.try_recv() {
                Ok(value) => {
                    buffer.push(value);
                    received += 1;
                }
                Err(_) => break,
            }
        }
        received
    }

    pub(crate) fn is_closed(&self) -> bool {
        // Kanal's is_closed means BOTH ends closed, unlike Tokio's receiver API.
        self.inner.sender_count() == 0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.outstanding.load(Ordering::Relaxed) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_receive_retains_the_next_value_and_fifo_order() {
        let (tx, mut rx) = unbounded_channel();
        {
            let receive = rx.recv();
            futures::pin_mut!(receive);
            assert!(futures::poll!(receive.as_mut()).is_pending());
        } // Simulate a batch timer winning select! after receive registered.
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        drop(tx);
        assert!(rx.is_closed());
        assert!(!rx.is_empty()); // first value can be outside Kanal's queue
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.try_recv(), Ok(2));
        assert!(rx.is_empty());
        assert_eq!(rx.recv().await, None);
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    }

    #[tokio::test]
    async fn last_sender_drop_drains_and_receiver_drop_rejects_sends() {
        let (tx, mut rx) = unbounded_channel();
        let clone = tx.clone();
        tx.send(1).unwrap();
        drop(tx);
        assert!(!rx.is_closed());
        clone.send(2).unwrap();
        drop(clone);
        assert!(rx.is_closed());
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.recv().await, Some(2));
        assert_eq!(rx.recv().await, None);
        let (tx, rx) = unbounded_channel();
        drop(rx);
        assert!(tx.send(3).is_err());
    }

    #[tokio::test]
    async fn recv_many_limits_and_appends_without_losing_rows() {
        let (tx, mut rx) = unbounded_channel();
        for value in 1..=5 {
            tx.send(value).unwrap();
        }
        drop(tx);
        let mut buffer = vec![0];
        assert_eq!(rx.recv_many(&mut buffer, 0).await, 0);
        assert_eq!(rx.recv_many(&mut buffer, 3).await, 3);
        assert_eq!(rx.recv_many(&mut buffer, 3).await, 2);
        assert_eq!(rx.recv_many(&mut buffer, 3).await, 0);
        assert_eq!(buffer, [0, 1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn try_recv_pending_waker_can_be_replaced_by_async_receive() {
        let (tx, mut rx) = unbounded_channel();
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        let task = tokio::spawn(async move { rx.recv().await });
        tx.send(1).unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Some(1)
        );
    }

    #[tokio::test]
    async fn recv_many_wakes_after_draining_to_empty() {
        let (tx, mut rx) = unbounded_channel();
        for expected in 0..1_000 {
            tx.send(expected).unwrap();
            let mut buffer = Vec::new();
            assert_eq!(rx.recv_many(&mut buffer, 500).await, 1);
            assert_eq!(buffer, [expected]);

            // recv_many's final try_recv registered a no-op waker. Poll the
            // next receive to Pending BEFORE sending, so success requires a
            // real wake-up rather than finding an already available value.
            let (ready, started) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                let mut buffer = Vec::new();
                let count = {
                    let receive = rx.recv_many(&mut buffer, 500);
                    futures::pin_mut!(receive);
                    assert!(futures::poll!(receive.as_mut()).is_pending());
                    ready.send(()).unwrap();
                    receive.await
                };
                (rx, count, buffer)
            });
            started.await.unwrap();
            tx.send(expected).unwrap();
            let (receiver, count, buffer) =
                tokio::time::timeout(std::time::Duration::from_secs(1), task)
                    .await
                    .expect("recv_many did not wake after send")
                    .unwrap();
            rx = receiver;
            assert_eq!(count, 1);
            assert_eq!(buffer, [expected]);
        }
    }

    #[tokio::test]
    async fn concurrent_producer_and_repeated_receive_cancellation_do_not_lose_updates() {
        let (tx, mut rx) = unbounded_channel();
        let producer = std::thread::spawn(move || {
            for value in 0..5_000 {
                tx.send(value).unwrap();
                if value % 7 == 0 {
                    std::thread::yield_now();
                }
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            for expected in 0..5_000 {
                let ready = {
                    let receive = rx.recv();
                    futures::pin_mut!(receive);
                    futures::poll!(receive.as_mut())
                }; // cancel every pending caller while producer runs concurrently
                let value = match ready {
                    Poll::Ready(value) => value,
                    Poll::Pending => {
                        tokio::task::yield_now().await;
                        rx.recv().await
                    }
                };
                assert_eq!(value, Some(expected));
            }
            assert_eq!(rx.recv().await, None);
            assert!(rx.is_closed() && rx.is_empty());
        })
        .await
        .unwrap();
        producer.join().unwrap();
    }
}
