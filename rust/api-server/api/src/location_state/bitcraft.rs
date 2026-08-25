use crate::AppState;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{SpacetimeUpdateMessages, record_worker_received};
use entity::shared::Region;
use game_module::module_bindings::LocationState;
use migration::{OnConflict, sea_query};
use sea_orm::{
    ColumnTrait, EntityOrSelect, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, QueryTrait,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::sleep;

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
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<LocationStateDbOperation>,
    messages: Vec<::entity::location_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl LocationStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_location_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
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
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_location_state = ::entity::location_state::Entity::find()
            .select_only()
            .column(::entity::location_state::Column::EntityId)
            .filter(::entity::location_state::Column::Region.eq(&database_name))
            .into_tuple()
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole location_state state"
                );
                vec![]
            })
            .into_iter()
            .collect::<HashSet<_>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::location_state::Model =
                ::entity::location_state::ModelBuilder::new(value)
                    .with_region(database_name.clone())
                    .build();

            model
        }) {
            match currently_known_location_state.contains(&model.entity_id) {
                true => {
                    currently_known_location_state.remove(&model.entity_id);
                    local_messages.push(model.into_active_model());
                }
                false => {}
            }
            if local_messages.len() >= self.batch_size {
                let messages = std::mem::replace(
                    &mut local_messages,
                    Vec::with_capacity(self.batch_size + 10),
                );
                self.queue_upserts(messages);
            }
        }
        if !local_messages.is_empty() {
            let messages = std::mem::replace(
                &mut local_messages,
                Vec::with_capacity(self.batch_size + 10),
            );
            self.queue_upserts(messages);
        }

        self.queue_deletes(
            currently_known_location_state
                .into_iter()
                .collect::<Vec<_>>(),
        );
    }

    async fn handle_insert(&mut self, new: LocationState, database_name: Region) {
        let model: ::entity::location_state::Model =
            ::entity::location_state::ModelBuilder::new(new)
                .with_region(database_name.clone())
                .build();

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(&mut self, new: LocationState, database_name: Region) {
        let model: ::entity::location_state::Model =
            ::entity::location_state::ModelBuilder::new(new)
                .with_region(database_name.clone())
                .build();
        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(&mut self, delete: LocationState, database_name: Region) {
        let model: ::entity::location_state::Model =
            ::entity::location_state::ModelBuilder::new(delete)
                .with_region(database_name.clone())
                .build();
        let id = model.entity_id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }
        self.messages_delete.push(id);
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("LocationState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
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
) -> UnboundedSender<LocationStateDbOperation> {
    let (tx, mut rx) = unbounded_channel();
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

    tokio::spawn(async move {
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
    let insert = ::entity::location_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting LocationState: {}", err)
    }

    messages.clear();
}
