use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::LocationState;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, ModelTrait};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) async fn insert_many_location_state(
    global_app_state: &AppState,
    on_conflict: &sea_orm::sea_query::OnConflict,
    messages: &mut Vec<::entity::location_state::ActiveModel>,
) -> Result<(), sea_orm::DbErr> {
    if messages.is_empty() {
        return Ok(());
    }

    ::entity::location_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await?;

    messages.clear();
    Ok(())
}

pub(crate) fn start_worker_location_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<LocationState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
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

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                let mut local_messages = Vec::with_capacity(batch_size + 10);
                                for entry in data {
                                    let model = ::entity::location_state::ModelBuilder::new(entry)
                                        .with_region(database_name.to_string())
                                        .build();
                                    if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                        messages.remove(index);
                                    }
                                    local_messages.push(model.into_active_model());
                                    if local_messages.len() >= batch_size {
                                        let insert = insert_many_location_state(
                                            &global_app_state,
                                            &on_conflict,
                                            &mut local_messages,
                                        )
                                        .await;
                                        if let Err(e) = insert {
                                            tracing::error!("Error inserting InteriorNetworkDesc: {}", e);
                                        }
                                    }
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model = ::entity::location_state::ModelBuilder::new(new)
                                    .with_region(database_name.to_string())
                                    .build();
                                if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                  messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size { break; }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model = ::entity::location_state::ModelBuilder::new(new)
                                    .with_region(database_name.to_string())
                                    .build();
                                if let Some(index) = messages.iter().position(|value: &::entity::location_state::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                  messages.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size { break; }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model = ::entity::location_state::ModelBuilder::new(delete)
                                    .with_region(database_name.to_string())
                                    .build();
                                if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                    messages.remove(index);
                                }
                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(error = error.to_string(), "Could not delete LocationState");
                                }
                            }
                        }
                    }
                    _ = &mut timer => { break; }
                    else => { break; }
                }
            }

            if !messages.is_empty() {
                let insert =
                    insert_many_location_state(&global_app_state, &on_conflict, &mut messages)
                        .await;
                if let Err(e) = insert {
                    tracing::error!("Error inserting LocationState: {}", e);
                }
            }

            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
