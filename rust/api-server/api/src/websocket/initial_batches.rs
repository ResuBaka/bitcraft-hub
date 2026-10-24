//! Move owned SDK snapshot rows into the application's reconciliation batches.
//! A private channel keeps all chunks in one reconciliation operation. Explicit
//! completion distinguishes a full snapshot from an interrupted producer.
use super::worker_channel::UnboundedSender;
use super::{SpacetimeUpdateMessages, send_worker_message};
use futures::{Stream, stream};
use sea_orm::DbErr;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

enum Part<T> {
    Rows(Vec<T>),
    Complete,
}

pub(crate) struct InitialBatches<T> {
    rx: UnboundedReceiver<Part<T>>,
    worker: &'static str,
}

impl<T> InitialBatches<T> {
    async fn next(&mut self) -> Result<Option<Vec<T>>, DbErr> {
        match self.rx.recv().await {
            Some(Part::Rows(rows)) => {
                metrics::counter!("worker_initial_batches_received_total", "worker" => self.worker)
                    .increment(1);
                Ok(Some(rows))
            }
            Some(Part::Complete) => {
                metrics::counter!("worker_initial_snapshots_completed_total", "worker" => self.worker).increment(1);
                Ok(None)
            }
            None => Err(DbErr::Custom(format!(
                "{} initial snapshot ended without completion",
                self.worker
            ))),
        }
    }
}

impl<T> Drop for InitialBatches<T> {
    fn drop(&mut self) {
        // Stop the producer first; account for queued batches discarded on a
        // reconciliation error rather than reporting them as a permanent queue.
        self.rx.close();
        while let Ok(part) = self.rx.try_recv() {
            if let Part::Rows(_) = part {
                metrics::counter!("worker_initial_batches_discarded_total", "worker" => self.worker).increment(1);
            }
        }
    }
}

pub(crate) fn send_initial<T: Send + 'static>(
    worker: &'static str,
    tx: &UnboundedSender<SpacetimeUpdateMessages<T>>,
    rows: Vec<T>,
    region: entity::shared::Region,
    batch_rows: usize,
) {
    if batch_rows == 0 {
        send_worker_message(
            worker,
            tx,
            SpacetimeUpdateMessages::Initial {
                data: rows,
                database_name: region,
                batches: None,
            },
        );
        return;
    }
    let mut rows = rows.into_iter();
    let (batch_tx, batch_rx) = unbounded_channel();
    send_worker_message(
        worker,
        tx,
        SpacetimeUpdateMessages::Initial {
            data: Vec::new(),
            database_name: region,
            batches: Some(InitialBatches {
                rx: batch_rx,
                worker,
            }),
        },
    );
    // This deliberately leaves queues unbounded for the first A/B experiment.
    // The consumer may release each vector while later chunks are being moved.
    loop {
        if batch_tx.is_closed() {
            return;
        }
        let chunk = rows.by_ref().take(batch_rows).collect::<Vec<_>>();
        if chunk.is_empty() {
            break;
        }
        metrics::counter!("worker_initial_batches_sent_total", "worker" => worker).increment(1);
        if batch_tx.send(Part::Rows(chunk)).is_err() {
            metrics::counter!("worker_initial_batches_send_errors_total", "worker" => worker)
                .increment(1);
            return;
        }
    }
    let _ = batch_tx.send(Part::Complete);
}

/// Flatten one owned chunk at a time. An unfinished stream is an error, so the
/// reconciler never mistakes a disconnected producer for a complete snapshot.
pub(crate) fn rows<T: Send>(
    data: Vec<T>,
    batches: Option<InitialBatches<T>>,
) -> impl Stream<Item = Result<T, DbErr>> + Send {
    stream::try_unfold(
        (data.into_iter(), batches),
        |(mut current, mut batches)| async move {
            loop {
                if let Some(row) = current.next() {
                    return Ok(Some((row, (current, batches))));
                }
                // Release the exhausted chunk's backing allocation before
                // waiting for another chunk, not after the entire snapshot.
                drop(current);
                match batches.as_mut() {
                    Some(source) => match source.next().await? {
                        Some(chunk) => current = chunk.into_iter(),
                        None => return Ok(None),
                    },
                    None => return Ok(None),
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;

    #[tokio::test]
    async fn owned_payloads_move_without_cloning_and_full_vectors_transfer_unchanged() {
        // Deliberately not Clone: this transport must only move payloads.
        #[derive(Debug)]
        struct Row {
            payload: String,
        }
        for batch_rows in [0, 2] {
            let input = (0..5)
                .map(|id| Row {
                    payload: format!("row-{id}"),
                })
                .collect::<Vec<_>>();
            let allocation = input.as_ptr();
            let payloads = input
                .iter()
                .map(|row| row.payload.as_ptr() as usize)
                .collect::<Vec<_>>();
            let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
            send_initial("test", &tx, input, 7, batch_rows);
            let SpacetimeUpdateMessages::Initial { data, batches, .. } = rx.recv().await.unwrap()
            else {
                panic!("expected initial")
            };
            if batch_rows == 0 {
                assert_eq!(data.as_ptr(), allocation);
            }
            let received = rows(data, batches).try_collect::<Vec<_>>().await.unwrap();
            assert_eq!(
                received
                    .iter()
                    .map(|row| row.payload.as_ptr() as usize)
                    .collect::<Vec<_>>(),
                payloads
            );
        }
    }

    #[tokio::test]
    async fn abandoned_owned_snapshots_drop_every_payload_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Row(Arc<AtomicUsize>);
        impl Drop for Row {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        for batch_rows in [0, 2] {
            for closed_before_send in [false, true] {
                let drops = Arc::new(AtomicUsize::new(0));
                let input = (0..5).map(|_| Row(Arc::clone(&drops))).collect();
                let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
                if closed_before_send {
                    drop(rx);
                    send_initial("test", &tx, input, 7, batch_rows);
                } else {
                    send_initial("test", &tx, input, 7, batch_rows);
                    assert_eq!(drops.load(Ordering::Relaxed), 0);
                    // Abandon a delivered snapshot, including queued chunks.
                    drop(rx.recv().await.unwrap());
                    drop(rx);
                }
                assert_eq!(drops.load(Ordering::Relaxed), 5);
            }
        }
    }

    #[tokio::test]
    async fn chunk_sizes_and_order_are_preserved_without_a_full_copy_in_the_envelope() {
        let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
        send_initial("test", &tx, vec![1, 2, 3, 4, 5], 7, 2);
        let SpacetimeUpdateMessages::Initial {
            data,
            database_name,
            batches,
        } = rx.recv().await.unwrap()
        else {
            panic!("expected initial")
        };
        assert!(data.is_empty());
        assert_eq!(data.capacity(), 0);
        assert_eq!(database_name, 7);
        let mut source = batches.unwrap();
        assert_eq!(source.next().await.unwrap(), Some(vec![1, 2]));
        assert_eq!(source.next().await.unwrap(), Some(vec![3, 4]));
        assert_eq!(source.next().await.unwrap(), Some(vec![5]));
        assert_eq!(source.next().await.unwrap(), None);
    }

    #[tokio::test]
    async fn empty_snapshot_completes_and_zero_batch_size_preserves_legacy_delivery() {
        let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
        send_initial::<u8>("test", &tx, vec![], 7, 2);
        let SpacetimeUpdateMessages::Initial { data, batches, .. } = rx.recv().await.unwrap()
        else {
            panic!("expected initial")
        };
        assert!(
            rows(data, batches)
                .try_collect::<Vec<_>>()
                .await
                .unwrap()
                .is_empty()
        );
        send_initial("test", &tx, vec![1, 2, 3], 7, 0);
        let SpacetimeUpdateMessages::Initial { data, batches, .. } = rx.recv().await.unwrap()
        else {
            panic!("expected initial")
        };
        assert!(batches.is_none());
        assert_eq!(data, [1, 2, 3]);
    }

    #[tokio::test]
    async fn missing_completion_is_an_error_not_an_empty_tail() {
        let (tx, rx) = unbounded_channel();
        tx.send(Part::Rows(vec![1])).ok().unwrap();
        drop(tx);
        let source = rows(vec![], Some(InitialBatches { rx, worker: "test" }));
        futures::pin_mut!(source);
        assert_eq!(source.try_next().await.unwrap(), Some(1));
        assert!(source.try_next().await.is_err());
    }

    #[tokio::test]
    async fn regional_snapshots_finish_before_the_next_envelope_or_live_update() {
        let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
        send_initial("test", &tx, vec![1, 2, 3], 1, 2);
        send_initial("test", &tx, vec![4, 5], 2, 2);
        tx.send(SpacetimeUpdateMessages::Insert {
            event: None,
            new: 6,
            database_name: 1,
        })
        .ok()
        .unwrap();
        for expected in [vec![1, 2, 3], vec![4, 5]] {
            let SpacetimeUpdateMessages::Initial { data, batches, .. } = rx.recv().await.unwrap()
            else {
                panic!("expected initial")
            };
            assert_eq!(
                rows(data, batches).try_collect::<Vec<_>>().await.unwrap(),
                expected
            );
        }
        assert!(matches!(
            rx.recv().await,
            Some(SpacetimeUpdateMessages::Insert { new: 6, .. })
        ));
    }

    #[tokio::test]
    async fn dropping_reconciliation_drops_remaining_chunks() {
        let (tx, rx) = unbounded_channel();
        let source = InitialBatches { rx, worker: "test" };
        drop(source);
        assert!(tx.send(Part::Rows(vec![1])).is_err());
    }

    fn collectible(id: i32) -> entity::vault_state_collectibles::Model {
        entity::vault_state_collectibles::Model {
            entity_id: 1,
            id,
            activated: true,
            count: 1,
            region: 7,
        }
    }

    fn key(id: i32) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [("0".into(), 1i64.into()), ("1".into(), id.into())].into()
    }

    fn conflict() -> migration::OnConflict {
        use entity::vault_state_collectibles::Column as C;
        migration::OnConflict::columns([C::EntityId, C::Id])
            .update_columns([C::Count])
            .to_owned()
    }

    fn filter(keys: &[(i64, i32)]) -> sea_orm::Condition {
        use entity::vault_state_collectibles::Column as C;
        super::super::initial_sync::composite_filter(C::EntityId, C::Id, keys)
    }

    #[tokio::test]
    async fn reconciliation_waits_for_all_chunks_before_deleting_stale_rows() {
        use entity::vault_state_collectibles::{Column as C, Entity as E};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryTrait};
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([vec![key(1), key(2), key(3)]])
            .append_query_results([vec![collectible(1)], vec![collectible(2)]])
            .append_exec_results([sea_orm::MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let (tx, rx) = unbounded_channel();
        let source = rows(vec![], Some(InitialBatches { rx, worker: "test" }));
        let (seen_tx, mut seen_rx) = unbounded_channel();
        let connection = db.clone();
        let task = tokio::spawn(async move {
            super::super::initial_sync::reconcile_stream::<E, _, _, _, _, _>(
                &connection,
                C::Region,
                7,
                &conflict(),
                1,
                source,
                |model| (model.entity_id, model.id),
                filter,
                |model, _| {
                    seen_tx.send(model.id).unwrap();
                },
            )
            .await
        });
        tx.send(Part::Rows(vec![collectible(1)])).ok().unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), seen_rx.recv())
                .await
                .unwrap(),
            Some(1)
        );
        assert!(!task.is_finished(), "first chunk is not the full region");
        tx.send(Part::Rows(vec![collectible(2)])).ok().unwrap();
        tx.send(Part::Complete).ok().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 4); // keys, two bounded comparisons, one final delete
        assert_eq!(
            log[3],
            sea_orm::Transaction::one(
                E::delete_many()
                    .filter(C::Region.eq(7i16))
                    .filter(filter(&[(1, 3)]))
                    .build(sea_orm::DbBackend::Postgres)
            )
        );
    }

    #[tokio::test]
    async fn interrupted_chunked_reconciliation_never_deletes_unseen_rows() {
        use entity::vault_state_collectibles::{Column as C, Entity as E};
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([vec![key(1), key(2)]])
            .append_query_results([vec![collectible(1)]])
            .into_connection();
        let (tx, rx) = unbounded_channel();
        tx.send(Part::Rows(vec![collectible(1)])).ok().unwrap();
        drop(tx); // no Complete marker
        let result = super::super::initial_sync::reconcile_stream::<E, _, _, _, _, _>(
            &db,
            C::Region,
            7,
            &conflict(),
            1,
            rows(vec![], Some(InitialBatches { rx, worker: "test" })),
            |model| (model.entity_id, model.id),
            filter,
            |_, _| {},
        )
        .await;
        assert!(result.is_err());
        assert_eq!(db.into_transaction_log().len(), 2); // no stale deletion
    }
}
