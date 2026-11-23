//! Worker input channel backend. Default: Kanal; feature enables the original
//! Tokio implementation for an otherwise identical A/B memory comparison.
#[cfg(not(feature = "tokio-worker-queues"))]
#[path = "worker_channel_kanal.rs"]
mod kanal_backend;
#[cfg(not(feature = "tokio-worker-queues"))]
use kanal_backend as backend;
#[cfg(feature = "tokio-worker-queues")]
use tokio::sync::mpsc as backend;

#[cfg(not(feature = "tokio-worker-queues"))]
use backend::TryRecvError as BackendTryRecvError;
#[cfg(feature = "tokio-worker-queues")]
use backend::error::TryRecvError as BackendTryRecvError;
use parking_lot::Mutex;
use std::sync::Arc;

pub(crate) trait SealInput: Send + Sync {
    fn close(&self);
}

// All producers share the *only* backend sender. Sealing under the send lock
// drops it, so the backend itself wakes the receiver, drains, and reports EOF.
struct Input<T> {
    sender: Mutex<Option<backend::UnboundedSender<T>>>,
}

impl<T: Send> SealInput for Input<T> {
    fn close(&self) {
        self.sender.lock().take();
    }
}

pub(crate) struct UnboundedSender<T> {
    input: Arc<Input<T>>,
}

pub(crate) struct UnboundedReceiver<T> {
    inner: backend::UnboundedReceiver<T>,
}

#[derive(Debug)]
pub(crate) struct SendError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TryRecvError {
    Empty,
    Disconnected,
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
        }
    }
}

impl<T> UnboundedSender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), SendError> {
        self.input
            .sender
            .lock()
            .as_ref()
            .ok_or(SendError)?
            .send(value)
            .map_err(|_| SendError)
    }
}

pub(crate) fn unbounded_channel<T: Send + 'static>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = backend::unbounded_channel();
    (
        UnboundedSender {
            input: Arc::new(Input {
                sender: Mutex::new(Some(tx)),
            }),
        },
        UnboundedReceiver { inner: rx },
    )
}

pub(crate) fn shutdown_channel<T: Send + 'static>(
    shutdown: &crate::shutdown::Shutdown,
) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = unbounded_channel();
    let input = Arc::downgrade(&tx.input);
    shutdown.register_input(input);
    (tx, rx)
}

impl<T: Send + 'static> UnboundedReceiver<T> {
    pub(crate) async fn recv(&mut self) -> Option<T> {
        self.inner.recv().await
    }

    pub(crate) fn try_recv(&mut self) -> Result<T, TryRecvError> {
        match self.inner.try_recv() {
            Ok(value) => Ok(value),
            Err(BackendTryRecvError::Empty) => Err(TryRecvError::Empty),
            Err(_) => Err(TryRecvError::Disconnected),
        }
    }

    pub(crate) async fn recv_many(&mut self, buffer: &mut Vec<T>, limit: usize) -> usize {
        if limit == 0 {
            return 0;
        }
        let Some(first) = self.recv().await else {
            return 0;
        };
        buffer.push(first);
        let mut count = 1;
        while count < limit {
            match self.try_recv() {
                Ok(value) => {
                    buffer.push(value);
                    count += 1;
                }
                Err(_) => break,
            }
        }
        count
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

pub(crate) const BACKEND: &str = if cfg!(feature = "tokio-worker-queues") {
    "tokio"
} else {
    "kanal"
};

#[cfg(all(test, target_os = "linux"))]
#[path = "../../benches/worker-queue/replay.rs"]
mod replay;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recv_many_yields_with_a_continuous_backlog() {
        let (tx, mut rx) = unbounded_channel();
        for value in 0..1_024 {
            tx.send(value).unwrap();
        }
        // A ready queue must eventually exhaust the task's cooperative budget,
        // allowing unrelated workers and timers on this thread to run.
        let yielded = tokio::spawn(async move {
            let mut buffer = Vec::new();
            for expected in 0..1_024 {
                buffer.clear();
                let received = {
                    let receive = rx.recv_many(&mut buffer, 1);
                    futures::pin_mut!(receive);
                    futures::poll!(receive.as_mut())
                };
                match received {
                    std::task::Poll::Pending => return true,
                    std::task::Poll::Ready(count) => {
                        assert_eq!(count, 1);
                        assert_eq!(buffer, [expected]);
                    }
                }
            }
            false
        })
        .await
        .unwrap();
        assert!(yielded, "recv_many monopolized the task with a ready queue");
    }

    #[tokio::test]
    async fn close_rejects_new_messages_but_drains_fifo_with_senders_alive() {
        let (tx, mut rx) = unbounded_channel();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.input.close();
        assert!(tx.send(3).is_err());
        assert!(rx.is_closed());
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.recv().await, Some(2));
        assert!(rx.is_empty());
        assert_eq!(rx.recv().await, None);
    }

    #[tokio::test]
    async fn close_wakes_parked_receive_and_preserves_a_message_in_flight() {
        for with_message in [false, true] {
            let (tx, mut rx) = unbounded_channel();
            let (parked, registered) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                let first = {
                    let receive = rx.recv();
                    futures::pin_mut!(receive);
                    assert!(futures::poll!(receive.as_mut()).is_pending());
                    parked.send(()).unwrap();
                    receive.await
                };
                (first, rx.recv().await)
            });
            registered.await.unwrap();
            if with_message {
                tx.send(7).unwrap();
            }
            tx.input.close();
            let received = tokio::time::timeout(std::time::Duration::from_secs(2), task)
                .await
                .expect("closing must wake a parked worker")
                .unwrap();
            assert_eq!(received, (with_message.then_some(7), None));
        }
    }

    #[tokio::test]
    async fn racing_seal_drains_exactly_the_accepted_messages_per_producer() {
        let (tx, mut rx) = unbounded_channel();
        let start = std::sync::Barrier::new(3);
        let accepted = std::thread::scope(|scope| {
            let producers = [0, 1].map(|id| {
                let producer = tx.clone();
                let start = &start;
                scope.spawn(move || {
                    producer.send((id, 0)).unwrap();
                    start.wait();
                    (1..1024)
                        .find(|&seq| producer.send((id, seq)).is_err())
                        .unwrap_or(1024)
                })
            });
            start.wait();
            tx.input.close();
            producers.map(|p| p.join().unwrap())
        });
        assert!(tx.send((0, 1024)).is_err());
        let mut drained = [0; 2];
        while let Some((id, seq)) = rx.recv().await {
            assert_eq!(seq, drained[id]);
            drained[id] += 1;
        }
        assert_eq!(drained, accepted);
    }
}
