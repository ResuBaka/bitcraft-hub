//! Handles belong to a worker/listener, not a global cache. They are registered
//! lazily so unused/error-only series stay absent. Production installs its
//! recorder before starting workers and registering listeners.
use std::sync::OnceLock;

pub(crate) struct WorkerReceiveMetrics {
    worker: &'static str,
    handles: OnceLock<(metrics::Counter, metrics::Gauge)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::{
        Counter, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit,
    };
    use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusRecorder};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct CountingRecorder {
        inner: PrometheusRecorder,
        registrations: Mutex<HashMap<Key, usize>>,
    }

    impl CountingRecorder {
        fn new() -> Self {
            Self {
                inner: PrometheusBuilder::new().build_recorder(),
                registrations: Mutex::new(HashMap::new()),
            }
        }

        fn count(&self, key: &Key) {
            *self
                .registrations
                .lock()
                .unwrap()
                .entry(key.clone())
                .or_default() += 1;
        }
    }

    impl Recorder for CountingRecorder {
        fn describe_counter(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
            self.inner.describe_counter(key, unit, description);
        }
        fn describe_gauge(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
            self.inner.describe_gauge(key, unit, description);
        }
        fn describe_histogram(&self, key: KeyName, unit: Option<Unit>, description: SharedString) {
            self.inner.describe_histogram(key, unit, description);
        }
        fn register_counter(&self, key: &Key, metadata: &Metadata<'_>) -> Counter {
            self.count(key);
            self.inner.register_counter(key, metadata)
        }
        fn register_gauge(&self, key: &Key, metadata: &Metadata<'_>) -> Gauge {
            self.count(key);
            self.inner.register_gauge(key, metadata)
        }
        fn register_histogram(&self, key: &Key, metadata: &Metadata<'_>) -> Histogram {
            self.count(key);
            self.inner.register_histogram(key, metadata)
        }
    }

    #[test]
    fn receive_handles_are_lazy_reused_and_keep_labels_and_counts() {
        let recorder = CountingRecorder::new();
        // Construct before a recorder exists: no no-op handles are cached.
        let received = WorkerReceiveMetrics::new("inventory");
        metrics::with_local_recorder(&recorder, || {
            received.record(0);
            assert!(recorder.registrations.lock().unwrap().is_empty());
            received.record(2);
            received.record(3);
            received.record(0);
        });
        let registrations = recorder.registrations.lock().unwrap();
        assert_eq!(registrations.len(), 2);
        assert!(registrations.values().all(|count| *count == 1));
        let rendered = recorder.inner.handle().render();
        assert!(rendered.contains("worker_queue_received_total{worker=\"inventory\"} 5"));
        assert!(rendered.contains("worker_queue_receive_batch_size{worker=\"inventory\"} 3"));
    }

    #[test]
    fn send_attempts_and_failures_keep_original_accounting_and_lazy_error_series() {
        let recorder = CountingRecorder::new();
        metrics::with_local_recorder(&recorder, || {
            let sent = WorkerSendMetrics::new("inventory");
            let (tx, rx) = super::super::worker_channel::unbounded_channel::<
                super::super::SpacetimeUpdateMessages<u8>,
            >();
            let message = || super::super::SpacetimeUpdateMessages::Insert {
                event: None,
                database_name: 7,
                new: 1,
            };
            sent.send(&tx, message());
            assert_eq!(recorder.registrations.lock().unwrap().len(), 1);
            drop(rx);
            sent.send(&tx, message());
            sent.send(&tx, message());
        });
        assert!(
            recorder
                .registrations
                .lock()
                .unwrap()
                .values()
                .all(|count| *count == 1)
        );
        let rendered = recorder.inner.handle().render();
        assert!(rendered.contains("worker_queue_sent_total{worker=\"inventory\"} 3"));
        assert!(rendered.contains("worker_queue_send_errors_total{worker=\"inventory\"} 2"));
    }

    #[test]
    fn listener_fixed_events_register_once_per_label_set() {
        let recorder = CountingRecorder::new();
        metrics::with_local_recorder(&recorder, || {
            for kind in ["insert", "update", "initial", "delete"] {
                let listener = ListenerMetrics::new("experience_state", "region_7".into(), kind);
                for _ in 0..3 {
                    listener.record(&spacetimedb_sdk::Event::Transaction);
                    listener.record(&spacetimedb_sdk::Event::SubscribeApplied);
                    listener.record(&spacetimedb_sdk::Event::UnsubscribeApplied);
                    listener.record(&spacetimedb_sdk::Event::Disconnected);
                }
            }
        });
        let registrations = recorder.registrations.lock().unwrap();
        assert_eq!(registrations.len(), 16);
        assert!(registrations.values().all(|count| *count == 1));
        for key in registrations.keys() {
            let labels: HashMap<_, _> = key
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect();
            assert_eq!(labels["table"], "experience_state");
            assert_eq!(labels["database"], "region_7");
            assert_eq!(labels.len(), 4);
        }
        assert_eq!(
            recorder
                .inner
                .handle()
                .render()
                .lines()
                .filter(|line| line.starts_with("game_message_events{") && line.ends_with(" 3"))
                .count(),
            16
        );
    }
}

impl WorkerReceiveMetrics {
    pub(crate) fn new(worker: &'static str) -> Self {
        Self {
            worker,
            handles: OnceLock::new(),
        }
    }

    pub(crate) fn record(&self, count: usize) {
        if count == 0 {
            return;
        }
        let (received, batch_size) = self.handles.get_or_init(|| {
            (
                metrics::counter!("worker_queue_received_total", "worker" => self.worker),
                metrics::gauge!("worker_queue_receive_batch_size", "worker" => self.worker),
            )
        });
        received.increment(count as u64);
        batch_size.set(count as f64);
    }
}

pub(super) struct WorkerSendMetrics {
    worker: &'static str,
    sent: OnceLock<metrics::Counter>,
    errors: OnceLock<metrics::Counter>,
}

impl WorkerSendMetrics {
    pub(super) fn new(worker: &'static str) -> Self {
        Self {
            worker,
            sent: OnceLock::new(),
            errors: OnceLock::new(),
        }
    }

    pub(super) fn send<T>(
        &self,
        tx: &super::worker_channel::UnboundedSender<super::SpacetimeUpdateMessages<T>>,
        message: super::SpacetimeUpdateMessages<T>,
    ) {
        self.sent
            .get_or_init(|| metrics::counter!("worker_queue_sent_total", "worker" => self.worker))
            .increment(1);
        if tx.send(message).is_err() {
            self.errors
                .get_or_init(
                    || metrics::counter!("worker_queue_send_errors_total", "worker" => self.worker),
                )
                .increment(1);
        }
    }
}

pub(super) struct ListenerMetrics {
    table: &'static str,
    database: String,
    kind: &'static str,
    // Fixed event kinds use direct slots rather than a per-message hash lookup.
    events: [OnceLock<metrics::Counter>; 6],
}

impl ListenerMetrics {
    pub(super) fn new(table: &'static str, database: String, kind: &'static str) -> Self {
        Self {
            table,
            database,
            kind,
            events: std::array::from_fn(|_| OnceLock::new()),
        }
    }

    pub(super) fn record(
        &self,
        event: &spacetimedb_sdk::Event<game_module::module_bindings::Reducer>,
    ) {
        use spacetimedb_sdk::__codegen::Reducer;
        use spacetimedb_sdk::Event;
        let (slot, reducer) = match event {
            Event::Reducer(event) => {
                // Reducer names are dynamic. Keep their original registration
                // path rather than introducing a locked cache in each callback.
                self.counter(event.reducer.reducer_name()).increment(1);
                return;
            }
            Event::SubscribeApplied => (0, "subscribe_applied"),
            Event::UnsubscribeApplied => (1, "unsubscribe_applied"),
            Event::Disconnected => (2, "disconnected"),
            Event::SubscribeError(_) => (3, "subscribe_error"),
            Event::Transaction => (4, "transaction"),
            _ => (5, "none"),
        };
        self.events[slot]
            .get_or_init(|| self.counter(reducer))
            .increment(1);
    }

    fn counter(&self, reducer: &str) -> metrics::Counter {
        metrics::counter!("game_message_events", "table" => self.table, "database" => self.database.clone(), "type" => self.kind, "reducer" => reducer.to_owned())
    }
}
