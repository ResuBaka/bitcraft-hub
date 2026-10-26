use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::worker_channel::{UnboundedReceiver, UnboundedSender};
use entity::shared::Region;
use game_module::module_bindings::LocationState;
use migration::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender as TokioUnboundedSender;

// pub(crate) async fn insert_many_location_state(
//     global_app_state: &AppState,
//     on_conflict: &sea_orm::sea_query::OnConflict,
//     messages: &mut Vec<::entity::location_state::ActiveModel>,
// ) -> Result<(), sea_orm::DbErr> {
//     if messages.is_empty() {
//         return Ok(());
//     }
//
//     ::entity::location_state::Entity::insert_many(messages.clone())
//         .on_conflict(on_conflict.clone())
//         .exec(&global_app_state.conn)
//         .await?;
//
//     messages.clear();
//     Ok(())
// }

// pub(crate) fn start_worker_location_state(
//     global_app_state: AppState,
//     mut rx: UnboundedReceiver<SpacetimeUpdateMessages<LocationState>>,
//     batch_size: usize,
//     time_limit: Duration,
// ) {
//     tokio::spawn(async move {
//         let on_conflict =
//             sea_orm::sea_query::OnConflict::columns([::entity::location_state::Column::EntityId])
//                 .update_columns([
//                     ::entity::location_state::Column::ChunkIndex,
//                     ::entity::location_state::Column::X,
//                     ::entity::location_state::Column::Z,
//                     ::entity::location_state::Column::Dimension,
//                     ::entity::location_state::Column::Region,
//                 ])
//                 .to_owned();
//
//         loop {
//             let mut messages = Vec::with_capacity(batch_size + 10);
//             let mut messages_delete = Vec::with_capacity(batch_size + 10);
//             let timer = sleep(time_limit);
//             tokio::pin!(timer);
//
//             loop {
//                 tokio::select! {
//                     Some(msg) = rx.recv() => {
//                         record_worker_received("location_state", 1);
//                         match msg {
//                             SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
//                                 let mut local_messages = Vec::with_capacity(batch_size + 10);
//                                 for entry in data {
//                                     let model = ::entity::location_state::ModelBuilder::new(entry)
//                                         .with_region(database_name)
//                                         .build();
//                                     if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
//                                         messages.remove(index);
//                                     }
//                                     local_messages.push(model.into_active_model());
//                                     if local_messages.len() >= batch_size {
//                                         let insert = insert_many_location_state(
//                                             &global_app_state,
//                                             &on_conflict,
//                                             &mut local_messages,
//                                         )
//                                         .await;
//                                         if let Err(e) = insert {
//                                             tracing::error!("Error inserting LocationState: {}", e);
//                                         }
//                                     }
//                                 }
//                             }
//                             SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
//                                 let model = ::entity::location_state::ModelBuilder::new(new)
//                                     .with_region(database_name)
//                                     .build();
//                                 if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
//                                     messages_delete.remove(index);
//                                 }
//                                 if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
//                                   messages.remove(index);
//                                 }
//                                 messages.push(model.into_active_model());
//                                 if messages.len() >= batch_size { break; }
//                             }
//                             SpacetimeUpdateMessages::Update { new, database_name, .. } => {
//                                 let model = ::entity::location_state::ModelBuilder::new(new)
//                                     .with_region(database_name)
//                                     .build();
//                                 if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
//                                     messages_delete.remove(index);
//                                 }
//                                 if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
//                                   messages.remove(index);
//                                 }
//                                 messages.push(model.into_active_model());
//                                 if messages.len() >= batch_size { break; }
//                             }
//                             SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
//                                 let model = ::entity::location_state::ModelBuilder::new(delete)
//                                     .with_region(database_name)
//                                     .build();
//                                 if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
//                                     messages.remove(index);
//                                 }
//                                 messages_delete.push(model.entity_id);
//                                 if messages_delete.len() >= batch_size { break; }
//                             }
//                         }
//                     }
//                     _ = &mut timer => { break; }
//                     else => { break; }
//                 }
//             }
//
//             if !messages.is_empty() {
//                 let insert =
//                     insert_many_location_state(&global_app_state, &on_conflict, &mut messages)
//                         .await;
//                 if let Err(e) = insert {
//                     tracing::error!("Error inserting LocationState: {}", e);
//                 }
//             }
//
//             if !messages_delete.is_empty() {
//                 for chunk_ids in messages_delete.chunks(1000) {
//                     let chunk_ids = chunk_ids.to_vec();
//                     if let Err(error) = ::entity::location_state::Entity::delete_many()
//                         .filter(::entity::location_state::Column::EntityId.is_in(chunk_ids.clone()))
//                         .exec(&global_app_state.conn)
//                         .await
//                     {
//                         let chunk_ids_str: Vec<String> =
//                             chunk_ids.iter().map(|id| id.to_string()).collect();
//                         tracing::error!(
//                             LocationState = chunk_ids_str.join(","),
//                             error = error.to_string(),
//                             "Could not delete LocationState"
//                         );
//                     }
//                 }
//                 messages_delete.clear();
//             }
//
//             if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
//                 break;
//             }
//         }
//     });
// }

enum LocationStateDbOperation {
    Upsert(Vec<::entity::location_state::ActiveModel>),
    Delete(Vec<i64>),
}

pub(crate) struct LocationStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<LocationState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<LocationState>>,
    batch_size: usize,
    time_limit: Duration,
    db_tx: TokioUnboundedSender<LocationStateDbOperation>,
    messages: Vec<::entity::location_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl LocationStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_location_state_db_worker(global_app_state.clone());
        let (tx, rx) =
            crate::websocket::worker_channel::shutdown_channel(&global_app_state.shutdown);

        Self {
            rx,
            tx,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::new(),
            messages_delete: Vec::new(),
        }
    }

    fn queue_upserts(&self, messages: Vec<::entity::location_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(LocationStateDbOperation::Upsert(messages)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue LocationState upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(LocationStateDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue LocationState deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<LocationState>) {
        match msg {
            SpacetimeUpdateMessages::Initial {
                data,
                database_name,
                ..
            } => {
                self.handle_initial(data, database_name).await;
            }
            SpacetimeUpdateMessages::Insert {
                new, database_name, ..
            } => {
                self.handle_insert(new, database_name).await;
            }
            SpacetimeUpdateMessages::Update {
                new, database_name, ..
            } => {
                self.handle_update(new, database_name).await;
            }
            SpacetimeUpdateMessages::Remove {
                delete,
                database_name,
                ..
            } => {
                self.handle_remove(delete, database_name).await;
            }
        }
    }

    async fn handle_initial(&mut self, data: Vec<LocationState>, database_name: Region) {
        // Each initial subscription covers only one chunk, not the whole region.
        // Upsert all received rows (including new entities), but never interpret
        // absent entities as deletions from other chunks. Live deletes still apply.
        for location in data {
            self.handle_insert(location, database_name).await;
            if self.messages.len() >= self.batch_size {
                self.flush_messages();
            }
        }
        self.flush_messages();
    }

    async fn handle_insert(&mut self, new: LocationState, database_name: Region) {
        let model: ::entity::location_state::Model =
            ::entity::location_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

        // The conflict key is entity_id alone, including across regions. A later
        // operation supersedes every pending write/delete for this entity.
        self.messages
            .retain(|value| value.entity_id.as_ref() != &model.entity_id);
        self.messages_delete.retain(|id| *id != model.entity_id);
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(&mut self, new: LocationState, database_name: Region) {
        // PostgreSQL rejects duplicate conflict keys within one INSERT statement.
        // Updates must coalesce exactly like inserts and initial chunk rows.
        self.handle_insert(new, database_name).await;
    }

    async fn handle_remove(&mut self, delete: LocationState, database_name: Region) {
        let model: ::entity::location_state::Model =
            ::entity::location_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        self.messages
            .retain(|value| value.entity_id.as_ref() != &id);
        if !self.messages_delete.contains(&id) {
            self.messages_delete.push(id);
        }
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages);
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("LocationState::Remove");
        let messages_delete = std::mem::take(&mut self.messages_delete);
        self.queue_deletes(messages_delete);
    }
}

impl BatchedWorker for LocationStateWorker {
    type Entity = LocationState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "location_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
    }
}

fn start_location_state_db_worker(
    global_app_state: AppState,
) -> TokioUnboundedSender<LocationStateDbOperation> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let on_conflict =
        sea_orm::sea_query::OnConflict::columns([::entity::location_state::Column::EntityId])
            .update_columns([
                ::entity::location_state::Column::ChunkIndex,
                ::entity::location_state::Column::X,
                ::entity::location_state::Column::Z,
                ::entity::location_state::Column::Dimension,
                ::entity::location_state::Column::Region,
            ])
            .to_owned();

    global_app_state.shutdown.clone().spawn_worker(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                LocationStateDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_location_state(&global_app_state, &on_conflict, &mut messages)
                        .await;
                }
                LocationStateDbOperation::Delete(ids) => {
                    delete_multiple_location_state(&global_app_state, ids).await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_location_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::location_state::Entity::delete_many()
            .filter(::entity::location_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                LocationState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete LocationState"
            );
        }
    }
}

async fn insert_multiple_location_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::location_state::ActiveModel>,
) {
    let insert = ::entity::location_state::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting LocationState: {}", err)
    }

    messages.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(entity_id: u64, x: i32) -> LocationState {
        LocationState {
            entity_id,
            chunk_index: 3003,
            x,
            z: 10,
            dimension: 1,
        }
    }

    fn model(entity_id: u64, x: i32, region: Region) -> ::entity::location_state::ActiveModel {
        ::entity::location_state::ModelBuilder::new(location(entity_id, x))
            .with_region(region)
            .build()
            .into_active_model()
    }

    fn worker() -> (
        LocationStateWorker,
        tokio::sync::mpsc::UnboundedReceiver<LocationStateDbOperation>,
    ) {
        let (tx, rx) = crate::websocket::worker_channel::unbounded_channel();
        let (db_tx, db_rx) = tokio::sync::mpsc::unbounded_channel();
        (
            LocationStateWorker {
                tx,
                rx,
                db_tx,
                batch_size: 10,
                time_limit: Duration::from_millis(200),
                messages: Vec::new(),
                messages_delete: Vec::new(),
            },
            db_rx,
        )
    }

    #[tokio::test]
    async fn repeated_updates_write_only_latest_row_per_conflict_key() {
        use entity::location_state::{Column, Entity};
        use sea_orm::{DbBackend, MockDatabase, QueryTrait, Transaction};
        let (mut worker, mut operations) = worker();
        worker.handle_insert(location(1, 10), 1).await;
        worker.handle_insert(location(2, 20), 1).await;
        worker.handle_update(location(1, 30), 1).await;
        worker.handle_update(location(1, 40), 2).await;
        worker.handle_update(location(1, 50), 3).await;
        assert_eq!(worker.messages, [model(2, 20, 1), model(1, 50, 3)]);
        worker.flush().await;
        let LocationStateDbOperation::Upsert(mut rows) = operations.try_recv().unwrap() else {
            panic!("expected upsert");
        };
        assert!(operations.try_recv().is_err());
        assert_eq!(rows, [model(2, 20, 1), model(1, 50, 3)]);

        let returned = ::entity::location_state::ModelBuilder::new(location(1, 50))
            .with_region(3)
            .build();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![returned]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let conflict = sea_orm::sea_query::OnConflict::column(Column::EntityId)
            .update_columns([
                Column::ChunkIndex,
                Column::X,
                Column::Z,
                Column::Dimension,
                Column::Region,
            ])
            .to_owned();
        let mut expected = Entity::insert_many([model(2, 20, 1), model(1, 50, 3)])
            .on_conflict(conflict.clone())
            .build(DbBackend::Postgres);
        expected.sql.push_str(" RETURNING \"entity_id\"");
        insert_multiple_location_state(&state, &conflict, &mut rows).await;
        assert!(rows.is_empty());
        assert_eq!(
            state.conn.into_transaction_log(),
            [Transaction::one(expected)]
        );
    }

    #[tokio::test]
    async fn later_deletes_and_upserts_cancel_all_prior_operations_for_entity() {
        let (mut worker, mut operations) = worker();
        worker
            .messages
            .extend([model(1, 10, 1), model(2, 20, 1), model(1, 30, 2)]);
        worker.handle_remove(location(1, 30), 2).await;
        worker.handle_remove(location(1, 30), 2).await;
        assert_eq!(worker.messages, [model(2, 20, 1)]);
        assert_eq!(worker.messages_delete, [1]);
        worker.messages_delete.push(1); // Also repair duplicates already pending.
        worker.handle_update(location(1, 40), 3).await;
        assert!(worker.messages_delete.is_empty());
        assert_eq!(worker.messages, [model(2, 20, 1), model(1, 40, 3)]);
        worker.handle_remove(location(1, 40), 3).await;
        worker.flush().await;
        assert!(
            matches!(operations.try_recv().unwrap(), LocationStateDbOperation::Upsert(rows) if rows == [model(2, 20, 1)])
        );
        assert!(
            matches!(operations.try_recv().unwrap(), LocationStateDbOperation::Delete(ids) if ids == [1])
        );
        assert!(worker.is_idle());
    }

    #[tokio::test]
    async fn initial_chunk_duplicates_and_live_updates_remain_ordered_across_flushes() {
        let (mut worker, mut operations) = worker();
        worker.handle_update(location(1, 10), 1).await;
        worker
            .handle_initial(vec![location(1, 20), location(1, 30)], 2)
            .await;
        assert!(
            matches!(operations.try_recv().unwrap(), LocationStateDbOperation::Upsert(rows) if rows == [model(1, 30, 2)])
        );
        worker.handle_update(location(1, 40), 2).await;
        worker.handle_update(location(1, 50), 2).await;
        worker.flush().await;
        assert!(
            matches!(operations.try_recv().unwrap(), LocationStateDbOperation::Upsert(rows) if rows == [model(1, 50, 2)])
        );
        worker.handle_initial(vec![], 2).await;
        assert!(
            operations.try_recv().is_err(),
            "empty chunk snapshots never imply stale deletion"
        );
    }

    #[tokio::test]
    async fn chunk_snapshots_upsert_new_entities_without_deleting_other_chunks() {
        let (tx, rx) = crate::websocket::worker_channel::unbounded_channel();
        let (db_tx, mut db_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut worker = LocationStateWorker {
            tx,
            rx,
            db_tx,
            batch_size: 10,
            time_limit: Duration::from_millis(200),
            messages: Vec::new(),
            messages_delete: Vec::new(),
        };
        for (entity_id, chunk_index) in [(42, 3003), (43, 4003)] {
            worker
                .handle_initial(
                    vec![LocationState {
                        entity_id,
                        chunk_index,
                        x: 0,
                        z: 0,
                        dimension: 1,
                    }],
                    1,
                )
                .await;
            let Some(LocationStateDbOperation::Upsert(rows)) = db_rx.recv().await else {
                panic!("chunk snapshot must enqueue an upsert, not a delete");
            };
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].entity_id.clone().unwrap(), entity_id as i64);
        }
        worker.handle_initial(Vec::new(), 1).await;
        assert!(db_rx.try_recv().is_err());
        assert!(worker.messages_delete.is_empty());
    }
}
