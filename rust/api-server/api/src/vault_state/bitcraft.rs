use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{composite_filter, reconcile_stream};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::vault_state_collectibles;
use futures::TryStreamExt;
use game_module::module_bindings::VaultState;
use migration::sea_query;
use sea_orm::{ColumnTrait, Condition, EntityTrait, IntoActiveModel, QueryFilter};
use std::collections::HashSet;
use std::time::Duration;

// Two array parameters keep PostgreSQL's statement shape independent of batch
// size and entity distribution. Multi-argument unnest pairs corresponding keys.
fn collectible_keys_filter(keys: &[(i64, i32)]) -> Condition {
    let (entities, ids): (Vec<i64>, Vec<i32>) = keys.iter().copied().unzip();
    Condition::all().add(sea_query::Expr::cust_with_values(
        r#"("entity_id", "id") IN (SELECT * FROM unnest($1::bigint[], $2::integer[]))"#,
        [sea_orm::Value::from(entities), sea_orm::Value::from(ids)],
    ))
}

pub(crate) fn start_worker_vault_state_collectibles(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<VaultState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("vault_state");
        let on_conflict = sea_query::OnConflict::columns([
            vault_state_collectibles::Column::EntityId,
            vault_state_collectibles::Column::Id,
        ])
        .update_columns([
            vault_state_collectibles::Column::Activated,
            vault_state_collectibles::Column::Count,
            vault_state_collectibles::Column::Region,
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
                            SpacetimeUpdateMessages::Initial { data, database_name, batches } => {
                                // Older live events must not be written after the newer snapshot.
                                insert_multiple_vault_state_collectibles(&global_app_state, &on_conflict, &mut messages).await;
                                delete_collectibles(&global_app_state, &mut messages_delete).await;
                                let incoming = crate::websocket::initial_batches::rows(data, batches).map_ok(|vault| {
                                    let entity_id = vault.entity_id as i64;
                                    futures::stream::iter(vault.collectibles.into_iter().map(move |collectible| Ok::<_, sea_orm::DbErr>(vault_state_collectibles::Model {
                                        entity_id,
                                        id: collectible.id,
                                        activated: collectible.activated,
                                        count: collectible.count,
                                        region: database_name,
                                    })))
                                }).try_flatten();
                                if let Err(error) = reconcile_stream::<vault_state_collectibles::Entity, _, _, _, _, _>(
                                    &global_app_state.conn,
                                    vault_state_collectibles::Column::Region,
                                    database_name,
                                    &on_conflict,
                                    batch_size,
                                    incoming,
                                    |model| (model.entity_id, model.id),
                                    collectible_keys_filter,
                                    |_, _| {},
                                ).await {
                                    tracing::error!(%error, region = database_name, "Could not reconcile initial vault collectibles");
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles(database_name);

                                for model in models {
                                    queue_collectible(&mut messages, &mut messages_delete, model);
                                }

                                if messages.len() + messages_delete.len() >= batch_size.max(1) {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { old, new, database_name, .. } => {
                                let removed = removed_collectibles(&old, &new);
                                for id in removed {
                                    queue_collectible_delete(&mut messages, &mut messages_delete, database_name, new.entity_id as i64, id);
                                }
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles(database_name);
                                for model in models {
                                    queue_collectible(&mut messages, &mut messages_delete, model);
                                }
                                if messages.len() + messages_delete.len() >= batch_size.max(1) {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = delete.into();
                                let models= raw_model.to_model_collectibles(database_name);
                                for model in models {

                                    queue_collectible_delete(&mut messages, &mut messages_delete, database_name, model.entity_id, model.id);
                                }

                                tracing::debug!("VaultState::Remove");
                                if messages.len() + messages_delete.len() >= batch_size.max(1) {
                                    break;
                                }
                            }
                        }
                    }
                    _ = timer.elapsed() => {
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
                //tracing::info!("Processing {} messages in batch", messages.len());
                insert_multiple_vault_state_collectibles(
                    &global_app_state,
                    &on_conflict,
                    &mut messages,
                )
                .await;
                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                delete_collectibles(&global_app_state, &mut messages_delete).await;
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() && rx.is_empty()
            {
                break;
            }
        }
    });
}

async fn insert_multiple_vault_state_collectibles(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: &mut Vec<::entity::vault_state_collectibles::ActiveModel>,
) {
    if messages.is_empty() {
        return;
    }
    let insert = ::entity::vault_state_collectibles::Entity::insert_many(std::mem::take(messages))
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting VaultState: {}", err)
    }
}

type CollectibleDelete = (entity::shared::Region, i64, i32);

fn queue_collectible(
    messages: &mut Vec<vault_state_collectibles::ActiveModel>,
    deletes: &mut Vec<CollectibleDelete>,
    model: vault_state_collectibles::Model,
) {
    deletes.retain(|&(region, entity_id, id)| {
        (region, entity_id, id) != (model.region, model.entity_id, model.id)
    });
    messages.retain(|value| {
        (*value.entity_id.as_ref(), *value.id.as_ref()) != (model.entity_id, model.id)
    });
    messages.push(model.into_active_model());
}

fn queue_collectible_delete(
    messages: &mut Vec<vault_state_collectibles::ActiveModel>,
    deletes: &mut Vec<CollectibleDelete>,
    region: entity::shared::Region,
    entity_id: i64,
    id: i32,
) {
    messages.retain(|value| (*value.entity_id.as_ref(), *value.id.as_ref()) != (entity_id, id));
    if !deletes.contains(&(region, entity_id, id)) {
        deletes.push((region, entity_id, id));
    }
}

fn removed_collectibles(old: &VaultState, new: &VaultState) -> Vec<i32> {
    let new_ids = new
        .collectibles
        .iter()
        .map(|value| value.id)
        .collect::<HashSet<_>>();
    old.collectibles
        .iter()
        .filter(|value| !new_ids.contains(&value.id))
        .map(|value| value.id)
        .collect()
}

async fn delete_collectibles(state: &AppState, deletes: &mut Vec<CollectibleDelete>) {
    for chunk in std::mem::take(deletes).chunks(1_000) {
        // Group by region so no deletion can affect a replicated/moved row elsewhere.
        let regions = chunk.iter().map(|value| value.0).collect::<HashSet<_>>();
        for region in regions {
            let keys = chunk
                .iter()
                .filter(|value| value.0 == region)
                .map(|value| (value.1, value.2))
                .collect::<Vec<_>>();
            if let Err(error) = vault_state_collectibles::Entity::delete_many()
                .filter(vault_state_collectibles::Column::Region.eq(region))
                .filter(composite_filter(
                    vault_state_collectibles::Column::EntityId,
                    vault_state_collectibles::Column::Id,
                    &keys,
                ))
                .exec(&state.conn)
                .await
            {
                tracing::error!(%error, region, "Could not delete vault collectibles");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::VaultCollectible;

    #[test]
    fn collectible_filter_has_fixed_sql_and_paired_array_parameters() {
        use sea_orm::{DbBackend, QueryTrait};
        let query = |keys: &[(i64, i32)]| {
            vault_state_collectibles::Entity::find()
                .filter(vault_state_collectibles::Column::Region.eq(7_i16))
                .filter(collectible_keys_filter(keys))
                .build(DbBackend::Postgres)
        };
        let one = query(&[(1, 10)]);
        let many = query(&[(1, 10), (2, 20), (1, 11)]);
        assert_eq!(one.sql, many.sql);
        assert!(many.sql.contains("unnest($2::bigint[], $3::integer[])"));
        let values = many.values.unwrap().0;
        assert_eq!(values.len(), 3);
        assert_eq!(values[1], sea_orm::Value::from(vec![1_i64, 2, 1]));
        assert_eq!(values[2], sea_orm::Value::from(vec![10_i32, 20, 11]));

        let keys: Vec<_> = (0..3000).map(|id| (i64::from(id), id)).collect();
        let old = vault_state_collectibles::Entity::find()
            .filter(vault_state_collectibles::Column::Region.eq(7_i16))
            .filter(composite_filter(
                vault_state_collectibles::Column::EntityId,
                vault_state_collectibles::Column::Id,
                &keys,
            ))
            .build(DbBackend::Postgres);
        let fixed = query(&keys);
        assert_eq!(old.values.as_ref().unwrap().0.len(), 6001);
        assert_eq!(fixed.values.as_ref().unwrap().0.len(), 3);
        println!(
            "3000 distinct-entity keys: old {} parameters/{} SQL bytes; fixed {} parameters/{} SQL bytes",
            old.values.unwrap().0.len(),
            old.sql.len(),
            fixed.values.unwrap().0.len(),
            fixed.sql.len()
        );
    }

    fn model(entity_id: i64, id: i32, count: i32) -> vault_state_collectibles::Model {
        vault_state_collectibles::Model {
            entity_id,
            id,
            activated: true,
            count,
            region: 7,
        }
    }

    #[test]
    fn live_upserts_keep_multiple_collectibles_for_the_same_vault() {
        let mut messages = Vec::new();
        let mut deletes = Vec::new();
        queue_collectible(&mut messages, &mut deletes, model(1, 10, 1));
        queue_collectible(&mut messages, &mut deletes, model(1, 11, 1));
        queue_collectible(&mut messages, &mut deletes, model(1, 10, 5));
        assert_eq!(messages.len(), 2);
        assert!(messages.contains(&model(1, 10, 5).into_active_model()));
        assert!(messages.contains(&model(1, 11, 1).into_active_model()));
    }

    #[test]
    fn deleting_one_collectible_keeps_other_collectibles_and_vaults() {
        let mut messages = Vec::new();
        let mut deletes = Vec::new();
        for (entity, id) in [(1, 10), (1, 11), (2, 10)] {
            queue_collectible(&mut messages, &mut deletes, model(entity, id, 1));
        }
        queue_collectible_delete(&mut messages, &mut deletes, 7, 1, 10);
        assert_eq!(messages.len(), 2);
        assert!(messages.contains(&model(1, 11, 1).into_active_model()));
        assert!(messages.contains(&model(2, 10, 1).into_active_model()));
        assert_eq!(deletes, [(7, 1, 10)]);
        queue_collectible_delete(&mut messages, &mut deletes, 7, 1, 10);
        assert_eq!(deletes.len(), 1);
        queue_collectible(&mut messages, &mut deletes, model(1, 10, 2));
        assert!(deletes.is_empty());
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn upserts_do_not_cancel_deletes_for_another_region() {
        let mut messages = Vec::new();
        let mut deletes = vec![(8, 1, 10)];
        queue_collectible(&mut messages, &mut deletes, model(1, 10, 1));
        assert_eq!(deletes, [(8, 1, 10)]);
    }

    #[test]
    fn updates_detect_collectibles_removed_from_a_vault_including_empty_vaults() {
        let old = VaultState {
            entity_id: 1,
            collectibles: vec![
                VaultCollectible {
                    id: 10,
                    activated: true,
                    count: 1,
                },
                VaultCollectible {
                    id: 11,
                    activated: true,
                    count: 2,
                },
            ],
        };
        let mut new = old.clone();
        new.collectibles.remove(0);
        assert_eq!(removed_collectibles(&old, &new), [10]);
        new.collectibles[0].count = 3;
        assert_eq!(removed_collectibles(&old, &new), [10]);
        new.collectibles.clear();
        assert_eq!(removed_collectibles(&old, &new), [10, 11]);
    }
}
