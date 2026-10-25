use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages, WorkerReceiveMetrics};
use game_module::module_bindings::TravelerTaskState;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_traveler_task_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("traveler_task_state");
        let on_conflict =
            sea_query::OnConflict::column(::entity::traveler_task_state::Column::EntityId)
                .update_columns([
                    ::entity::traveler_task_state::Column::PlayerEntityId,
                    ::entity::traveler_task_state::Column::TravelerId,
                    ::entity::traveler_task_state::Column::TaskId,
                    ::entity::traveler_task_state::Column::Completed,
                    ::entity::traveler_task_state::Column::Region,
                ])
                .to_owned();

        let mut messages = Vec::new();
        let mut messages_delete = Vec::new();
        loop {
            let mut timer = BatchTimer::new(time_limit);

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break; };
                        timer.start();
                        receive_metrics.record(1);
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages).await;
                                if !messages_delete.is_empty() {
                                    if let Err(error) = ::entity::traveler_task_state::Entity::delete_many()
                                        .filter(::entity::traveler_task_state::Column::EntityId.is_in(std::mem::take(&mut messages_delete)))
                                        .exec(&global_app_state.conn).await {
                                        tracing::error!(%error, "Could not flush traveler task deletes before initial sync");
                                    }
                                }
                                let incoming = data.into_iter().map(|value| ::entity::traveler_task_state::ModelBuilder::new(value).with_region(database_name).build());
                                if let Err(error) = crate::websocket::initial_sync::reconcile::<::entity::traveler_task_state::Entity, _, _, _, _, _>(
                                    &global_app_state.conn,
                                    ::entity::traveler_task_state::Column::Region,
                                    database_name,
                                    &on_conflict,
                                    batch_size,
                                    incoming,
                                    |model| model.entity_id,
                                    |ids| sea_query::Condition::all().add(::entity::traveler_task_state::Column::EntityId.is_in(ids.iter().copied())),
                                    |_, _| {},
                                ).await {
                                    tracing::error!(%error, region = database_name, "Could not reconcile initial traveler tasks");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(new).with_region(database_name).build();

                                let _ = global_app_state.tx.send(OutboundWebSocketMessages::TravelerTaskState(model.clone()));

                                queue_upsert(&mut messages, &mut messages_delete, model);
                                if messages.len() >= batch_size {
                                    insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                        .await;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(new).with_region(database_name).build();

                                let _ = global_app_state.tx.send(OutboundWebSocketMessages::TravelerTaskState(model.clone()));

                                queue_upsert(&mut messages, &mut messages_delete, model);
                                if messages.len() >= batch_size {
                                    insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                        .await;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(delete).build();
                                let id = model.entity_id;
                                let _ = global_app_state.tx.send(OutboundWebSocketMessages::TravelerTaskStateDelete(model.clone()));

                                queue_delete(&mut messages, &mut messages_delete, id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
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

            if !messages.is_empty() {
                insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                    .await;
            }

            if !messages_delete.is_empty() {
                tracing::debug!("TravelerTaskState::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::traveler_task_state::Entity::delete_many()
                        .filter(
                            ::entity::traveler_task_state::Column::EntityId
                                .is_in(chunk_ids.clone()),
                        )
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            TravelerTaskState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete TravelerTaskState"
                        );
                    }
                }
                messages_delete.clear();
            }

            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() && rx.is_empty()
            {
                break;
            }
        }
    });
}

// PostgreSQL rejects duplicate conflict keys within a single upsert statement.
// Both Insert and Update events must coalesce, including repeated regional or
// reconnect delivery. A later operation supersedes all earlier ones for that ID.
fn queue_upsert(
    messages: &mut Vec<::entity::traveler_task_state::Model>,
    deletes: &mut Vec<i64>,
    model: ::entity::traveler_task_state::Model,
) {
    messages.retain(|pending| pending.entity_id != model.entity_id);
    deletes.retain(|id| *id != model.entity_id);
    messages.push(model);
}

fn queue_delete(
    messages: &mut Vec<::entity::traveler_task_state::Model>,
    deletes: &mut Vec<i64>,
    id: i64,
) {
    messages.retain(|pending| pending.entity_id != id);
    if !deletes.contains(&id) {
        deletes.push(id);
    }
}

async fn insert_multiple_traveler_task_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::traveler_task_state::Model>,
) {
    if messages.is_empty() {
        return;
    }
    let insert = ::entity::traveler_task_state::Entity::insert_many(
        std::mem::take(messages)
            .into_iter()
            .map(|model| model.into_active_model()),
    )
    .on_conflict(on_conflict.clone())
    .exec(&global_app_state.conn)
    .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting TravelerTaskState: {}", err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use entity::traveler_task_state::{Column, Entity, Model};
    use sea_orm::{DbBackend, MockDatabase, QueryTrait, Transaction};

    fn task(entity_id: i64, completed: bool) -> Model {
        Model {
            entity_id,
            player_entity_id: 20,
            traveler_id: 30,
            task_id: 40,
            completed,
            region: 7,
        }
    }

    #[tokio::test]
    async fn repeated_inserts_and_updates_write_only_the_latest_row_per_entity() {
        let mut messages = Vec::new();
        let mut deletes = Vec::new();
        queue_upsert(&mut messages, &mut deletes, task(1, false));
        queue_upsert(&mut messages, &mut deletes, task(2, false));
        queue_upsert(&mut messages, &mut deletes, task(1, true));
        queue_upsert(&mut messages, &mut deletes, task(1, true));
        assert_eq!(messages, [task(2, false), task(1, true)]);
        assert!(deletes.is_empty());

        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![task(1, true)]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let on_conflict = sea_query::OnConflict::column(Column::EntityId)
            .update_column(Column::Completed)
            .to_owned();
        let mut expected = Entity::insert_many(
            [task(2, false), task(1, true)].map(IntoActiveModel::into_active_model),
        )
        .on_conflict(on_conflict.clone())
        .build(DbBackend::Postgres);
        expected.sql.push_str(" RETURNING \"entity_id\"");
        insert_multiple_traveler_task_state(&state, &on_conflict, &mut messages).await;
        assert!(messages.is_empty());
        assert_eq!(
            state.conn.into_transaction_log(),
            [Transaction::one(expected)]
        );
    }

    #[test]
    fn later_upserts_and_deletes_supersede_all_pending_operations_for_the_id() {
        let mut messages = vec![task(1, false), task(2, false), task(1, true)];
        let mut deletes = Vec::new();
        queue_delete(&mut messages, &mut deletes, 1);
        queue_delete(&mut messages, &mut deletes, 1);
        assert_eq!(messages, [task(2, false)]);
        assert_eq!(deletes, [1]);
        deletes.push(1); // also cancel every copy of a pre-existing delete
        queue_upsert(&mut messages, &mut deletes, task(1, true));
        assert_eq!(messages, [task(2, false), task(1, true)]);
        assert!(deletes.is_empty());
    }
}
