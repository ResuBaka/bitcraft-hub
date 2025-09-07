use crate::AppState;
use crate::websocket::SpacetimeUpdateMessages;
use game_module::module_bindings::TradeOrderState;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use sea_orm::{EntityTrait, sea_query};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_trade_order_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<TradeOrderState>>,
    _batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        // let on_conflict = sea_query::OnConflict::column(::entity::trade_order::Column::EntityId)
        //     .update_columns([
        //         ::entity::trade_order::Column::TimePlayed,
        //         ::entity::trade_order::Column::SessionStartTimestamp,
        //         ::entity::trade_order::Column::TimeSignedIn,
        //         ::entity::trade_order::Column::SignInTimestamp,
        //         ::entity::trade_order::Column::SignedIn,
        //         ::entity::trade_order::Column::TeleportLocation,
        //         ::entity::trade_order::Column::TravelerTasksExpiration,
        //     ])
        //     .to_owned();

        loop {
            // let mut messages = Vec::new();
            // let mut ids = vec![];
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                data.into_par_iter().for_each(|value| {
                                    let model: ::entity::trade_order::Model = ::entity::trade_order::ModelBuilder::new(value).with_region(database_name.to_string()).build();
                                    global_app_state.trade_order_state.insert(model.entity_id, model);
                                });

                                // let mut local_messages = vec![];
                                // let mut trade_order = ::entity::trade_order::Entity::find()
                                //     .filter(::entity::trade_order::Column::Region.eq(database_name.to_string()))
                                //     .all(&global_app_state.conn)
                                //     .await
                                //     .map_or(vec![], |aa| aa)
                                //     .into_iter()
                                //     .map(|value| (value.entity_id, value))
                                //     .collect::<HashMap<_, _>>();
                                //
                                // for model in data.into_iter().map(|value| {
                                //     let model: ::entity::trade_order::Model = ::entity::trade_order::ModelBuilder::new(value).with_region(database_name.to_string()).build();
                                //
                                //     model
                                // }) {
                                //     use std::collections::hash_map::Entry;
                                //     match trade_order.entry(model.entity_id) {
                                //         Entry::Occupied(entry) => {
                                //             let existing_model = entry.get();
                                //             if &model != existing_model {
                                //                 local_messages.push(model.into_active_model());
                                //             }
                                //             entry.remove();
                                //         }
                                //         Entry::Vacant(_entry) => {
                                //             local_messages.push(model.into_active_model());
                                //         }
                                //     }
                                //     if local_messages.len() >= batch_size {
                                //        // trade_order(&global_app_state, &on_conflict, &mut local_messages).await;
                                //     }
                                // };
                                // if !local_messages.is_empty() {
                                //     // trade_order(&global_app_state, &on_conflict, &mut local_messages).await;
                                // }
                                //
                                // for chunk_ids in trade_order.into_keys().collect::<Vec<_>>().chunks(1000) {
                                //     let chunk_ids = chunk_ids.to_vec();
                                //     if let Err(error) = ::entity::trade_order::Entity::delete_many().filter(::entity::trade_order::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                //         let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                //         tracing::error!(TradeOrder = chunk_ids_str.join(","), error = error.to_string(), "Could not delete TradeOrder");
                                //     }
                                // }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let model: ::entity::trade_order::Model = ::entity::trade_order::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                global_app_state.trade_order_state.insert(model.entity_id, model);

                                // if ids.contains(&model.entity_id) {
                                //     if let Some(index) = messages.iter().position(|value: &::entity::trade_order::ActiveModel| value.entity_id.as_ref() == &model.entity_id) {
                                //         messages.remove(index);
                                //     }
                                // }
                                //
                                // ids.push(model.entity_id);
                                // messages.push(model.into_active_model());
                                // if messages.len() >= batch_size {
                                //     break;
                                // }
                            }
                            SpacetimeUpdateMessages::Update { new, database_name,  .. } => {
                                let model: ::entity::trade_order::Model = ::entity::trade_order::ModelBuilder::new(new).with_region(database_name.to_string()).build();

                                global_app_state.trade_order_state.insert(model.entity_id, model);
                                // if ids.contains(&model.entity_id) {
                                //     if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                //         messages.remove(index);
                                //     }
                                // }

                                // let _ = global_app_state.tx.send(WebSocketMessages::TradeOrder(model.clone()));

                                // ids.push(model.entity_id);
                                //
                                // messages.push(model.into_active_model());
                                // if messages.len() >= batch_size {
                                //     break;
                                // }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let model: ::entity::trade_order::Model = ::entity::trade_order::ModelBuilder::new(delete).with_region(database_name.to_string()).build();
                                global_app_state.trade_order_state.remove(&model.entity_id);

                                // let id = model.entity_id;
                                //
                                // if ids.contains(&id) {
                                //     if let Some(index) = messages.iter().position(|value| value.entity_id.as_ref() == &model.entity_id) {
                                //         messages.remove(index);
                                //     }
                                // }
                                //
                                // if let Err(error) = model.delete(&global_app_state.conn).await {
                                //     tracing::error!(TradeOrder = id, error = error.to_string(), "Could not delete TradeOrder");
                                // }
                                //
                                // tracing::debug!("TradeOrder::Remove");
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

            // if !messages.is_empty() {
            //tracing::info!("Processing {} messages in batch", messages.len());

            // trade_order(&global_app_state, &on_conflict, &mut messages).await;
            // Your batch processing logic here
            // }

            // If the channel is closed and we processed the last batch, exit the outer loop
            // if messages.is_empty() && rx.is_closed() {
            if rx.is_closed() {
                break;
            }
        }
    });
}

#[allow(dead_code)]
async fn insert_multiple_trade_order(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::trade_order::ActiveModel>,
) {
    let insert = ::entity::trade_order::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if insert.is_err() {
        tracing::error!("Error inserting TradeOrder: {}", insert.unwrap_err())
    }

    messages.clear();
}
