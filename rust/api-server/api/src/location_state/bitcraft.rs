use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::LocationState;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub(crate) fn start_worker_location_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<LocationState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        // let on_conflict = sea_query::OnConflict::columns([::entity::building_desc::Column::Id])
        //     .update_columns([
        //         ::entity::building_desc::Column::Functions,
        //         ::entity::building_desc::Column::Name,
        //         ::entity::building_desc::Column::Description,
        //         ::entity::building_desc::Column::RestedBuffDuration,
        //         ::entity::building_desc::Column::LightRadius,
        //         ::entity::building_desc::Column::ModelAssetName,
        //         ::entity::building_desc::Column::IconAssetName,
        //         ::entity::building_desc::Column::Unenterable,
        //         ::entity::building_desc::Column::Wilderness,
        //         ::entity::building_desc::Column::Footprint,
        //         ::entity::building_desc::Column::MaxHealth,
        //         ::entity::building_desc::Column::IgnoreDamage,
        //         ::entity::building_desc::Column::DefenseLevel,
        //         ::entity::building_desc::Column::Decay,
        //         ::entity::building_desc::Column::Maintenance,
        //         ::entity::building_desc::Column::BuildPermission,
        //         ::entity::building_desc::Column::InteractPermission,
        //         ::entity::building_desc::Column::HasAction,
        //         ::entity::building_desc::Column::ShowInCompendium,
        //         ::entity::building_desc::Column::IsRuins,
        //         ::entity::building_desc::Column::NotDeconstructible,
        //     ])
        //     .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                for entry in data {
                                    let model: ::entity::location::Model = ::entity::location::ModelBuilder::new(entry).with_region(database_name.to_string()).build();

                                    global_app_state.location_state.insert(model.entity_id, model);
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::location::Model = ::entity::location::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                messages.push(model.clone());
                                global_app_state.location_state.insert(model.entity_id, model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name, .. } => {
                                let model: ::entity::location::Model = ::entity::location::ModelBuilder::new(new).with_region(database_name.to_string()).build();
                                // messages.push(model.clone());
                               global_app_state.location_state.insert(model.entity_id, model);

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::location::Model = ::entity::location::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value.entity_id == model.entity_id) {
                                    messages.remove(index);
                                }

                                global_app_state.location_state.remove(&id);

                                // if let Err(error) = model.delete(&global_app_state.conn).await {
                                //     tracing::error!(LocationState = id, error = error.to_string(), "Could not delete LocationState");
                                // }

                                tracing::debug!("LocationState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!(
                    "LocationState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                // let insert = ::entity::building_desc::Entity::insert_many(
                //     messages
                //         .iter()
                //         .map(|value| value.clone().into_active_model())
                //         .collect::<Vec<_>>(),
                // )
                // .on_conflict(on_conflict.clone())
                // .exec(&global_app_state.conn)
                // .await;
                //
                // if insert.is_err() {
                //     tracing::error!("Error inserting BuildingDesc: {}", insert.unwrap_err())
                // }
                // Your batch processing logic here

                messages.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}
