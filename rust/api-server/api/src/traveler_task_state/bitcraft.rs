use crate::AppState;
use crate::websocket::{
    OutboundWebSocketMessages, SpacetimeUpdateMessages, record_worker_received,
};
use game_module::module_bindings::TravelerTaskState;
use migration::sea_query;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_traveler_task_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TravelerTaskState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let mut messages_delete = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        record_worker_received("traveler_task_state", 1);
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

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                        .await;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(new).with_region(database_name).build();

                                let _ = global_app_state.tx.send(OutboundWebSocketMessages::TravelerTaskState(model.clone()));

                                if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                    messages.remove(index);
                                }

                                if let Some(index) = messages_delete.iter().position(|value| *value == model.entity_id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    insert_multiple_traveler_task_state(&global_app_state, &on_conflict, &mut messages)
                                        .await;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, .. } => {
                                let model: ::entity::traveler_task_state::Model = ::entity::traveler_task_state::ModelBuilder::new(delete).build();
                                let id = model.entity_id;
                                let _ = global_app_state.tx.send(OutboundWebSocketMessages::TravelerTaskStateDelete(model.clone()));

                                if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                    messages.remove(index);
                                }

                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
                                    break;
                                }
                            }
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

            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
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
