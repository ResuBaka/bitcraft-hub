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
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct ChannelControl {
    gate: Mutex<()>,
    closed: CancellationToken,
}

impl ChannelControl {
    pub(crate) fn close(&self) {
        let _guard = self.gate.lock();
        self.closed.cancel();
    }
}

pub(crate) struct UnboundedSender<T> {
    inner: backend::UnboundedSender<T>,
    control: Arc<ChannelControl>,
}

pub(crate) struct UnboundedReceiver<T> {
    inner: backend::UnboundedReceiver<T>,
    control: Arc<ChannelControl>,
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
            inner: self.inner.clone(),
            control: self.control.clone(),
        }
    }
}

impl<T> UnboundedSender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), SendError> {
        let _guard = self.control.gate.lock();
        if self.control.closed.is_cancelled() {
            return Err(SendError);
        }
        self.inner.send(value).map_err(|_| SendError)
    }
}

pub(crate) fn unbounded_channel<T: Send + 'static>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = backend::unbounded_channel();
    let control = Arc::new(ChannelControl::default());
    (
        UnboundedSender {
            inner: tx,
            control: control.clone(),
        },
        UnboundedReceiver { inner: rx, control },
    )
}

pub(crate) fn shutdown_channel<T: Send + 'static>(
    shutdown: &crate::shutdown::Shutdown,
) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = unbounded_channel();
    shutdown.register_input(&rx.control);
    (tx, rx)
}

impl<T: Send + 'static> UnboundedReceiver<T> {
    pub(crate) async fn recv(&mut self) -> Option<T> {
        tokio::select! { biased;
            _ = self.control.closed.cancelled() => self.try_recv().ok(),
            value = self.inner.recv() => value,
        }
    }

    pub(crate) fn try_recv(&mut self) -> Result<T, TryRecvError> {
        match self.inner.try_recv() {
            Ok(value) => Ok(value),
            Err(BackendTryRecvError::Empty) if !self.control.closed.is_cancelled() => {
                Err(TryRecvError::Empty)
            }
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
        self.control.closed.is_cancelled() || self.inner.is_closed()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn close_rejects_new_messages_but_drains_fifo_with_senders_alive() {
        let (tx, mut rx) = unbounded_channel();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        rx.control.close();
        assert!(tx.send(3).is_err());
        assert!(rx.is_closed());
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.recv().await, Some(2));
        assert!(rx.is_empty());
        assert_eq!(rx.recv().await, None);
    }

    #[tokio::test]
    async fn close_wakes_pending_receive_and_preserves_a_message_in_flight() {
        for with_message in [false, true] {
            let (tx, mut rx) = unbounded_channel();
            {
                let receive = rx.recv();
                futures::pin_mut!(receive);
                assert!(futures::poll!(receive.as_mut()).is_pending());
            }
            if with_message {
                tx.send(7).unwrap();
            }
            rx.control.close();
            assert_eq!(rx.recv().await, with_message.then_some(7));
            assert_eq!(rx.recv().await, None);
        }
    }
}
