use crate::AppState;
use crate::websocket::batched_worker::BatchTimer;
use crate::websocket::initial_sync::{flush_before_snapshot, reconcile_global};
use crate::websocket::worker_channel::UnboundedReceiver;
use crate::websocket::{SpacetimeUpdateMessages, WorkerReceiveMetrics};
use entity::skill_desc;
use game_module::module_bindings::SkillDesc;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use std::time::Duration;

pub(crate) fn start_worker_skill_desc(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<SkillDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    global_app_state.shutdown.clone().spawn_worker(async move {
        let receive_metrics = WorkerReceiveMetrics::new("skill_desc");
        let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
            .update_columns([
                skill_desc::Column::Name,
                skill_desc::Column::Description,
                skill_desc::Column::IconAssetName,
                skill_desc::Column::Title,
                skill_desc::Column::SkillCategory,
                skill_desc::Column::Skill,
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
                            SpacetimeUpdateMessages::Initial { data, .. } => {
                                let filter = |keys: &[i64]| sea_orm::Condition::all().add(::entity::skill_desc::Column::Id.is_in(keys.iter().copied()));
                                if let Err(error) = flush_before_snapshot::<::entity::skill_desc::Entity, _>(
                                    &global_app_state.conn, &on_conflict, &mut messages, &mut messages_delete, filter,
                                ).await {
                                    tracing::error!(%error, "Could not flush skill_desc before snapshot");
                                    continue;
                                }
                                // Publish the complete table together, including removals. A
                                // failed reconciliation leaves the previous definitions visible.
                                let mut skills = super::SkillDefinitions::with_capacity(data.len());
                                match reconcile_global::<::entity::skill_desc::Entity, _, _, _, _, _>(
                                    &global_app_state.conn, &on_conflict, batch_size,
                                    data.into_iter().map(Into::into), |model| model.id, filter,
                                    |model, _| { skills.insert(model.id, model.clone()); },
                                ).await {
                                    Ok(()) => *global_app_state.skill_desc.write() = skills,
                                    Err(error) => tracing::error!(%error, "Could not reconcile skill_desc snapshot"),
                                }
                            }
                            SpacetimeUpdateMessages::Insert { new, .. } | SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::skill_desc::Model = new.into();
                                {
                                    let mut skills = global_app_state.skill_desc.write();
                                    if skills.get(&model.id) != Some(&model) {
                                        skills.insert(model.id, model.clone());
                                    }
                                }
                                if let Some(index) = messages_delete.iter().position(|value| *value == model.id) {
                                    messages_delete.remove(index);
                                }
                                messages.push(model.into_active_model());
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::skill_desc::Model = delete.into();
                                let id = model.id;

                                global_app_state.skill_desc.write().remove(&id);
                                if let Some(index) = messages.iter().position(|value| value.id.as_ref() == &model.id) {
                                    messages.remove(index);
                                }
                                messages_delete.push(id);
                                if messages_delete.len() >= batch_size {
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
                insert_multiple_skill_desc(&global_app_state, &on_conflict, &mut messages).await;

                // Your batch processing logic here
            }

            if !messages_delete.is_empty() {
                tracing::debug!("SkillDesc::Remove");
                for chunk_ids in messages_delete.chunks(1000) {
                    let chunk_ids = chunk_ids.to_vec();
                    if let Err(error) = ::entity::skill_desc::Entity::delete_many()
                        .filter(::entity::skill_desc::Column::Id.is_in(chunk_ids.clone()))
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            SkillDesc = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete SkillDesc"
                        );
                    }
                }
                messages_delete.clear();
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && messages_delete.is_empty() && rx.is_closed() && rx.is_empty()
            {
                break;
            }
        }
    });
}

async fn insert_multiple_skill_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::skill_desc::ActiveModel>,
) {
    let insert = ::entity::skill_desc::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting SkillDesc: {}", err)
    }

    messages.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::SkillCategory;
    use sea_orm::{DbBackend, DbErr, MockDatabase, MockExecResult};

    fn skill(id: i32) -> SkillDesc {
        SkillDesc {
            id,
            skill_type: id,
            name: format!("skill-{id}"),
            description: String::new(),
            icon_asset_name: String::new(),
            title: String::new(),
            skill_category: SkillCategory::Profession,
            max_level: 100,
        }
    }

    #[tokio::test]
    async fn xp_definitions_follow_snapshots_and_live_edits_but_not_failed_snapshots() {
        use SpacetimeUpdateMessages::*;
        let row = skill_desc::Model::from(skill(1));
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![], vec![row.clone()]]) // key scan, initial upsert
            .append_query_results([vec![row], vec![]]) // flush live upserts, next key scan
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .append_query_errors([DbErr::Custom("failed snapshot upsert".into())])
            .into_connection();
        let (out, _) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), out, Default::default());
        state.skill_desc.write().insert(9, skill(9).into()); // stale startup row
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        start_worker_skill_desc(state.clone(), rx, 100, Duration::from_secs(10));
        let edited = SkillDesc {
            name: "renamed".into(),
            skill_category: SkillCategory::Adventure,
            ..skill(1)
        };
        for msg in [
            Initial {
                database_name: 7,
                data: vec![skill(1), skill(2), skill(3)],
                batches: None,
            },
            Update {
                database_name: 7,
                old: skill(1),
                new: edited.clone(),
                event: None,
            },
            Remove {
                database_name: 7,
                delete: skill(2),
                event: None,
            },
            Insert {
                database_name: 7,
                new: skill(4),
                event: None,
            },
            Initial {
                database_name: 7,
                data: vec![skill(5)],
                batches: None,
            },
        ] {
            tx.send(msg).unwrap();
        }
        state.shutdown.drain().await;
        let skills = state.skill_desc.read();
        assert_eq!(
            **skills,
            [
                (1, edited.into()),
                (3, skill(3).into()),
                (4, skill(4).into())
            ]
            .into()
        );
        for (id, category) in [
            (1, Some(2)),
            (2, None),
            (3, Some(1)),
            (4, Some(1)),
            (5, None),
            (9, None),
        ] {
            assert_eq!(skills.category(id), category, "skill {id}");
        }
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 6);
        assert!(format!("{:?}", log[5]).contains("INSERT INTO"));
    }

    #[tokio::test]
    async fn empty_snapshot_clears_definitions_only_after_a_successful_flush() {
        for fail_flush in [false, true] {
            let db = MockDatabase::new(DbBackend::Postgres);
            let db = if fail_flush {
                db.append_query_errors([DbErr::Custom("failed pre-snapshot flush".into())])
            } else {
                db.append_query_results([vec![skill_desc::Model::from(skill(0))], vec![]])
            };
            let (out, _) = tokio::sync::mpsc::unbounded_channel();
            let state = AppState::new(db.into_connection(), out, Default::default());
            state.skill_desc.write().insert(-1, skill(-1).into());
            let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
            start_worker_skill_desc(state.clone(), rx, 100, Duration::from_secs(10));
            tx.send(SpacetimeUpdateMessages::Insert {
                database_name: 7,
                new: skill(0),
                event: None,
            })
            .unwrap();
            tx.send(SpacetimeUpdateMessages::Initial {
                database_name: 7,
                data: vec![],
                batches: None,
            })
            .unwrap();
            state.shutdown.drain().await;
            let skills = state.skill_desc.read();
            for id in [-1, 0] {
                assert_eq!(skills.category(id), fail_flush.then_some(1));
                assert_eq!(skills.contains_key(&i64::from(id)), fail_flush);
            }
        }
    }
}
