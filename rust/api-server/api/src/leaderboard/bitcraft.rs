use crate::AppState;
use crate::leaderboard::{Leaderboard, experience_to_level};
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages};
use entity::shared::Region;
use game_module::module_bindings::ExperienceState;
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, IntoActiveModel, QueryFilter};
use std::ops::AddAssign;
use std::time::Duration;
use tokio::sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel};

enum ExperienceStateDbOperation {
    Upsert(Vec<::entity::experience_state::ActiveModel>),
    Delete(Vec<(i64, i32)>),
    Barrier(tokio::sync::oneshot::Sender<()>),
}

pub(crate) struct ExperienceStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<ExperienceStateDbOperation>,
    messages: Vec<::entity::experience_state::ActiveModel>,
    ids: Vec<i64>,
    messages_delete: Vec<(i64, i32)>,
}

/// Flatten lazily: do not allocate a second region-wide experience row vector.
fn initial_experience_rows(
    state: &AppState,
    data: Vec<ExperienceState>,
    region: Region,
) -> impl Iterator<Item = ::entity::experience_state::Model> + Send + '_ {
    data.into_iter().flat_map(move |value| {
        let id = value.entity_id as i64;
        let mut total_exp = 0;
        let mut profession_exp = 0;
        let mut adventure_exp = 0;
        for stack in &value.experience_stacks {
            total_exp += i64::from(stack.quantity);
            if let Some(skill) = state.skill_desc.get(&i64::from(stack.skill_id)) {
                match skill.skill_category {
                    1 => profession_exp += i64::from(stack.quantity),
                    2 => adventure_exp += i64::from(stack.quantity),
                    _ => {}
                }
            }
        }
        if let Some(xp) = state.ranking_system.global_leaderboard.get_value(&id)
            && xp != total_exp
        {
            state
                .ranking_system
                .global_leaderboard
                .update(id, total_exp);
            let xp_per_hour = state
                .player_state
                .get(&id)
                .filter(|player| player.time_signed_in >= 3_600)
                .map_or(0, |player| {
                    total_exp / (i64::from(player.time_signed_in) / 3_600)
                });
            state.ranking_system.xp_per_hour.update(id, xp_per_hour);
        }
        if let Some(xp) = state.ranking_system.adventure_leaderboard.get_value(&id)
            && xp != adventure_exp
        {
            state
                .ranking_system
                .adventure_leaderboard
                .update(id, adventure_exp);
        }
        if let Some(xp) = state.ranking_system.profession_leaderboard.get_value(&id)
            && xp != profession_exp
        {
            state
                .ranking_system
                .profession_leaderboard
                .update(id, profession_exp);
        }
        value
            .experience_stacks
            .into_iter()
            .map(move |stack| ::entity::experience_state::Model {
                entity_id: id,
                skill_id: stack.skill_id,
                experience: stack.quantity,
                region,
            })
    })
}

#[cfg(test)]
mod initial_tests {
    use super::*;
    use game_module::module_bindings::ExperienceStack;

    #[test]
    fn experience_rows_are_flattened_lazily_without_losing_totals() {
        let (tx, _rx) = unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        state.ranking_system.global_leaderboard.update(1, 1);
        state.ranking_system.global_leaderboard.update(2, 1);
        let data = vec![
            ExperienceState {
                entity_id: 1,
                experience_stacks: vec![
                    ExperienceStack {
                        skill_id: 10,
                        quantity: 100,
                    },
                    ExperienceStack {
                        skill_id: 11,
                        quantity: 200,
                    },
                ],
            },
            ExperienceState {
                entity_id: 2,
                experience_stacks: vec![ExperienceStack {
                    skill_id: 10,
                    quantity: 400,
                }],
            },
        ];
        let mut rows = initial_experience_rows(&state, data, 7);
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&1),
            Some(1)
        );
        let first = rows.next().unwrap();
        assert_eq!(
            (
                first.entity_id,
                first.skill_id,
                first.experience,
                first.region
            ),
            (1, 10, 100, 7)
        );
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&1),
            Some(300)
        );
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&2),
            Some(1)
        );
        assert_eq!(rows.next().unwrap().skill_id, 11);
        assert_eq!(rows.next().unwrap().experience, 400);
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&2),
            Some(400)
        );
        assert!(rows.next().is_none());
    }

    #[tokio::test]
    async fn initial_sync_barrier_waits_for_preceding_live_writes() {
        let model = ::entity::experience_state::Model {
            entity_id: 1,
            skill_id: 10,
            experience: 100,
            region: 7,
        };
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([vec![model.clone()]])
            .into_connection();
        let (tx, _rx) = unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let worker = start_experience_state_db_worker(state);
        worker
            .send(ExperienceStateDbOperation::Upsert(vec![
                model.into_active_model(),
            ]))
            .await
            .unwrap();
        let (done, drained) = tokio::sync::oneshot::channel();
        worker
            .send(ExperienceStateDbOperation::Barrier(done))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), drained)
            .await
            .unwrap()
            .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
    }
}

fn start_experience_state_db_worker(
    global_app_state: AppState,
) -> Sender<ExperienceStateDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict = sea_query::OnConflict::columns([
        ::entity::experience_state::Column::EntityId,
        ::entity::experience_state::Column::SkillId,
    ])
    .update_columns([
        ::entity::experience_state::Column::Experience,
        ::entity::experience_state::Column::Region,
    ])
    .to_owned();

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ExperienceStateDbOperation::Upsert(messages) => {
                    insert_multiple_experience_state(&global_app_state, &on_conflict, messages)
                        .await;
                }
                ExperienceStateDbOperation::Delete(ids) => {
                    tracing::debug!("ExperienceState::Remove");
                    for chunk_ids in ids.chunks(100) {
                        let mut query = sea_query::Condition::any();
                        for (entity_id, skill_id) in chunk_ids {
                            query = query.add(
                                ::entity::experience_state::Column::EntityId
                                    .eq(*entity_id)
                                    .and(::entity::experience_state::Column::SkillId.eq(*skill_id)),
                            );
                        }

                        let chunk_ids = chunk_ids.to_vec();
                        if let Err(error) = ::entity::experience_state::Entity::delete_many()
                            .filter(query)
                            .exec(&global_app_state.conn)
                            .await
                        {
                            let chunk_ids_str: Vec<String> = chunk_ids
                                .iter()
                                .map(|(entity_id, skill_id)| format!("{}:{}", entity_id, skill_id))
                                .collect();
                            tracing::error!(
                                ExperienceState = chunk_ids_str.join(","),
                                error = error.to_string(),
                                "Could not delete ExperienceState"
                            );
                        }
                    }
                }
                ExperienceStateDbOperation::Barrier(done) => {
                    let _ = done.send(());
                }
            }
        }
    });

    tx
}

impl ExperienceStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_experience_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            ids: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::experience_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState upserts"
            );
        }
    }

    async fn queue_deletes(&self, ids: Vec<(i64, i32)>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::Delete(ids))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ExperienceState>) {
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
                new,
                database_name,
                old,
                ..
            } => {
                self.handle_update(new, database_name, old).await;
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

    async fn handle_initial(
        &mut self,
        data: Vec<ExperienceState>,
        database_name: entity::shared::Region,
    ) {
        tracing::debug!("Processed Initial ExperienceState {}", data.len());
        // Drain previously queued live writes before comparing a newer snapshot.
        self.flush_messages().await;
        self.flush_deletes().await;
        let (done, drained) = tokio::sync::oneshot::channel();
        if self
            .db_tx
            .send(ExperienceStateDbOperation::Barrier(done))
            .await
            .is_err()
            || drained.await.is_err()
        {
            tracing::error!(
                region = database_name,
                "Experience persistence worker stopped before initial sync"
            );
            return;
        }
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([
            ::entity::experience_state::Column::Experience,
            ::entity::experience_state::Column::Region,
        ])
        .to_owned();
        let state = &self.global_app_state;
        if let Err(error) = crate::websocket::initial_sync::reconcile::<
            ::entity::experience_state::Entity,
            _,
            _,
            _,
            _,
            _,
        >(
            &state.conn,
            ::entity::experience_state::Column::Region,
            database_name,
            &on_conflict,
            self.batch_size,
            initial_experience_rows(state, data, database_name),
            |model| (model.entity_id, model.skill_id),
            |keys| {
                crate::websocket::initial_sync::composite_filter(
                    ::entity::experience_state::Column::EntityId,
                    ::entity::experience_state::Column::SkillId,
                    keys,
                )
            },
            |model, changed| {
                if changed {
                    if let Some(board) = state
                        .ranking_system
                        .skill_leaderboards
                        .get(&(model.skill_id as i64))
                    {
                        board.update(model.entity_id, model.experience as i64);
                    }
                }
            },
        )
        .await
        {
            tracing::error!(%error, region = database_name, "Could not reconcile initial experience");
        }
        tracing::debug!(
            "Processed Initial ExperienceState {}",
            database_name.to_string()
        );
    }

    async fn handle_insert(&mut self, new: ExperienceState, database_name: entity::shared::Region) {
        let id = new.entity_id as i64;
        let mut total_exp = 0;
        let mut profession_exp = 0;
        let mut adventure_exp = 0;
        new.experience_stacks.iter().for_each(|es| {
            total_exp.add_assign(es.quantity as i64);
            if let Some(skill_desc) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                match skill_desc.skill_category {
                    1 => {
                        profession_exp.add_assign(es.quantity as i64);
                    }
                    2 => {
                        adventure_exp.add_assign(es.quantity as i64);
                    }
                    _ => {}
                }
            };

            let model = ::entity::experience_state::Model {
                entity_id: id,
                skill_id: es.skill_id,
                experience: es.quantity,
                region: database_name,
            };

            if let Some(index) = self
                .messages_delete
                .iter()
                .position(|value| value.0 == id && value.1 == es.skill_id)
            {
                self.messages_delete.remove(index);
            }
            if let Some(index) =
                self.messages
                    .iter()
                    .position(|value: &::entity::experience_state::ActiveModel| {
                        value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id
                    })
            {
                self.messages.remove(index);
            }
            self.messages.push(model.into_active_model());
        });

        let current_score = self
            .global_app_state
            .ranking_system
            .global_leaderboard
            .get_value(&(new.entity_id as i64));
        let mut xp_per_hour = 0;
        if let Some(player_state) = self
            .global_app_state
            .player_state
            .get(&(new.entity_id as i64))
            && player_state.time_signed_in >= 3600
        {
            xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
        }
        self.global_app_state
            .ranking_system
            .xp_per_hour
            .update(new.entity_id as i64, xp_per_hour);
        self.global_app_state
            .ranking_system
            .profession_leaderboard
            .update(new.entity_id as i64, profession_exp);
        self.global_app_state
            .ranking_system
            .adventure_leaderboard
            .update(new.entity_id as i64, adventure_exp);

        if let Some(current_score) = current_score {
            let current_known_xp = current_score;

            if current_known_xp < total_exp {
                self.global_app_state
                    .ranking_system
                    .global_leaderboard
                    .update(new.entity_id as i64, total_exp);
                let mut xp_per_hour = 0;
                if let Some(player_state) = self
                    .global_app_state
                    .player_state
                    .get(&(new.entity_id as i64))
                    && player_state.time_signed_in >= 3600
                {
                    xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
                }
                self.global_app_state
                    .ranking_system
                    .xp_per_hour
                    .update(new.entity_id as i64, xp_per_hour);
            }
        } else {
            self.global_app_state
                .ranking_system
                .global_leaderboard
                .update(new.entity_id as i64, total_exp);
            let mut xp_per_hour = 0;
            if let Some(player_state) = self
                .global_app_state
                .player_state
                .get(&(new.entity_id as i64))
                && player_state.time_signed_in >= 3600
            {
                xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
            }
            self.global_app_state
                .ranking_system
                .xp_per_hour
                .update(new.entity_id as i64, xp_per_hour);
        }
    }

    async fn handle_update(
        &mut self,
        new: ExperienceState,
        database_name: entity::shared::Region,
        old: ExperienceState,
    ) {
        // if new.entity_id == 1224979098660016778 {
        //     if let Some(Event::Reducer(event)) = &event {
        //         match last_time {
        //             Some(time) => {
        //                 let current_time = DateTime::from_timestamp_micros(event.timestamp.to_micros_since_unix_epoch()).unwrap();
        //
        //                 let diff = current_time.timestamp_millis() - time.timestamp_millis();
        //
        //                 tracing::error!("Diff {:?} Off {} Off % {} ", diff, diff - 1170, (diff as f64 / 1170f64 * 100f64) - 100f64);
        //
        //                 last_time = Some(current_time);
        //             }
        //             None => {
        //                 tracing::error!("AA {:?}", event.timestamp);
        //                 last_time = Some(DateTime::from_timestamp_micros(event.timestamp.to_micros_since_unix_epoch()).unwrap());
        //             }
        //         }
        //
        //     }
        // }

        let id = new.entity_id as i64;
        let mut new_total_exp = 0;
        let mut new_total_level = 0;
        let mut new_profession_exp = 0;
        let mut new_adventure_exp = 0;
        new.experience_stacks.iter().for_each(|es| {
            new_total_exp.add_assign(es.quantity as i64);
            let new_level = experience_to_level(es.quantity as i64);
            new_total_level.add_assign(new_level);
            if let Some(skill_desc) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                match skill_desc.skill_category {
                    1 => {
                        new_profession_exp.add_assign(es.quantity as i64);
                    }
                    2 => {
                        new_adventure_exp.add_assign(es.quantity as i64);
                    }
                    _ => {}
                }
            };
        });

        let mut old_total_exp = 0;
        let mut old_total_level = 0;
        let mut old_profession_exp = 0;
        let mut old_adventure_exp = 0;
        for (index, es) in old.experience_stacks.iter().enumerate() {
            old_total_exp.add_assign(es.quantity as i64);
            let old_level = experience_to_level(es.quantity as i64);
            old_total_level.add_assign(old_level);
            if let Some(skill_desc) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                match skill_desc.skill_category {
                    1 => {
                        old_profession_exp.add_assign(es.quantity as i64);
                    }
                    2 => {
                        old_adventure_exp.add_assign(es.quantity as i64);
                    }
                    _ => {}
                }
            };

            let new_skill = if let Some(new_skill) = new.experience_stacks.get(index) {
                if new_skill.skill_id == es.skill_id {
                    Some(new_skill)
                } else {
                    new.experience_stacks
                        .iter()
                        .find(|new_level| new_level.skill_id == es.skill_id)
                }
            } else {
                new.experience_stacks
                    .iter()
                    .find(|new_level| new_level.skill_id == es.skill_id)
            };
            if let Some(new_skill) = new_skill {
                let new_level = experience_to_level(new_skill.quantity as i64);
                if let Some(skill) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                    let skill_name = skill.to_owned().name;
                    if old_level != new_level {
                        let _ = self
                            .global_app_state
                            .tx
                            .send(OutboundWebSocketMessages::Level {
                                level: new_level as u64,
                                skill_name: skill_name.clone(),
                                user_id: id,
                            });
                    }
                }

                if new_skill.quantity > es.quantity {
                    let model = ::entity::experience_state::Model {
                        entity_id: id,
                        skill_id: es.skill_id,
                        experience: es.quantity,
                        region: database_name,
                    };

                    if let Some(index) = self
                        .messages_delete
                        .iter()
                        .position(|value| value.0 == id && value.1 == es.skill_id)
                    {
                        self.messages_delete.remove(index);
                    }
                    if let Some(index) = self.messages.iter().position(|value| {
                        value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id
                    }) {
                        self.messages.remove(index);
                    }
                    self.messages.push(model.into_active_model());

                    if let Some(skill) = self.global_app_state.skill_desc.get(&(es.skill_id as i64))
                        && skill.skill_category != 0
                    {
                        let skill_name = skill.to_owned().name;

                        let mut prev_rank = None;
                        let post_rank;
                        if let Some(skill_leaderboard) = self
                            .global_app_state
                            .ranking_system
                            .skill_leaderboards
                            .get_mut(&(es.skill_id as i64))
                        {
                            prev_rank = skill_leaderboard.get_rank(new.entity_id as i64);
                            skill_leaderboard
                                .update(new.entity_id as i64, new_skill.quantity as i64);
                            post_rank = skill_leaderboard.get_rank(new.entity_id as i64);
                        } else {
                            self.global_app_state
                                .ranking_system
                                .skill_leaderboards
                                .insert(es.skill_id as i64, Leaderboard::default());
                            self.global_app_state
                                .ranking_system
                                .skill_leaderboards
                                .get_mut(&(es.skill_id as i64))
                                .unwrap()
                                .update(new.entity_id as i64, new_skill.quantity as i64);
                            post_rank = self
                                .global_app_state
                                .ranking_system
                                .skill_leaderboards
                                .get_mut(&(es.skill_id as i64))
                                .unwrap()
                                .get_rank(new.entity_id as i64);
                        }

                        match (prev_rank, post_rank) {
                            (Some(prev_rank), Some(post_rank)) => {
                                if prev_rank != post_rank {
                                    tracing::debug!(
                                        "Skill EXP rank {skill_name} changed {prev_rank} to {post_rank}"
                                    );
                                }
                            }
                            (Some(prev_rank), None) => {
                                tracing::debug!(
                                    "Skill EXP rank {skill_name} changed {prev_rank} to no post_rank"
                                );
                            }
                            (None, Some(post_rank)) => {
                                tracing::debug!(
                                    "Skill EXP rank {skill_name} changed no prev_rank to {post_rank}"
                                );
                            }
                            (None, None) => {
                                tracing::error!("Skill EXP {skill_name} no rank?");
                            }
                        }
                        let _ =
                            self.global_app_state
                                .tx
                                .send(OutboundWebSocketMessages::Experience {
                                    level: new_level as u64,
                                    experience: new_skill.quantity as u64,
                                    rank: post_rank.unwrap_or(0) as u64,
                                    skill_name,
                                    user_id: id,
                                });
                    }
                }
            }
        }

        if old_total_level != new_total_level {
            let prev_rank = self
                .global_app_state
                .ranking_system
                .level_leaderboard
                .get_rank(new.entity_id as i64);
            self.global_app_state
                .ranking_system
                .level_leaderboard
                .update(new.entity_id as i64, new_total_level as i64);
            let post_rank = self
                .global_app_state
                .ranking_system
                .level_leaderboard
                .get_rank(new.entity_id as i64);

            match (prev_rank, post_rank) {
                (Some(prev_rank), Some(post_rank)) => {
                    if prev_rank != post_rank {
                        tracing::debug!("Level rank changed {prev_rank} to {post_rank}")
                    }
                }
                (Some(prev_rank), None) => {
                    tracing::debug!("Level rank changed {prev_rank} to no post_rank")
                }
                (None, Some(post_rank)) => {
                    tracing::debug!("Level rank changed no prev_rank to {post_rank}")
                }
                (None, None) => {
                    tracing::error!("Level no rank?")
                }
            }
        }

        if old_total_exp != new_total_exp {
            let prev_rank = self
                .global_app_state
                .ranking_system
                .global_leaderboard
                .get_rank(new.entity_id as i64);
            self.global_app_state
                .ranking_system
                .global_leaderboard
                .update(new.entity_id as i64, new_total_exp);
            let mut xp_per_hour = 0;
            if let Some(player_state) = self
                .global_app_state
                .player_state
                .get(&(new.entity_id as i64))
                && player_state.time_signed_in >= 3600
            {
                xp_per_hour = new_total_exp / (player_state.time_signed_in as i64 / 3600);
            }
            self.global_app_state
                .ranking_system
                .xp_per_hour
                .update(new.entity_id as i64, xp_per_hour);

            let post_rank = self
                .global_app_state
                .ranking_system
                .global_leaderboard
                .get_rank(new.entity_id as i64);

            match (prev_rank, post_rank) {
                (Some(prev_rank), Some(post_rank)) => {
                    if prev_rank != post_rank {
                        tracing::debug!("Total EXP rank changed {prev_rank} to {post_rank}")
                    }
                }
                (Some(prev_rank), None) => {
                    tracing::debug!("Total EXP rank changed {prev_rank} to no post_rank")
                }
                (None, Some(post_rank)) => {
                    tracing::debug!("Total EXP rank changed no prev_rank to {post_rank}")
                }
                (None, None) => {
                    tracing::error!("Total EXP no rank?")
                }
            }

            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::TotalExperience {
                    experience: new_total_exp as u64,
                    user_id: id,
                    experience_per_hour: xp_per_hour as u64,
                    rank: post_rank.unwrap_or(0) as u64,
                });
        }

        if old_adventure_exp != new_adventure_exp {
            self.global_app_state
                .ranking_system
                .adventure_leaderboard
                .update(new.entity_id as i64, new_adventure_exp);

            let post_rank = self
                .global_app_state
                .ranking_system
                .adventure_leaderboard
                .get_rank(new.entity_id as i64);

            let _ = self.global_app_state.tx.send(
                OutboundWebSocketMessages::TotalAdventureExperience {
                    experience: new_adventure_exp as u64,
                    user_id: id,
                    rank: post_rank.unwrap_or(0) as u64,
                },
            );
        }

        if old_profession_exp != new_profession_exp {
            self.global_app_state
                .ranking_system
                .profession_leaderboard
                .update(new.entity_id as i64, new_profession_exp);

            let post_rank = self
                .global_app_state
                .ranking_system
                .profession_leaderboard
                .get_rank(new.entity_id as i64);

            let _ = self.global_app_state.tx.send(
                OutboundWebSocketMessages::TotalProfessionExperience {
                    experience: new_profession_exp as u64,
                    user_id: id,
                    rank: post_rank.unwrap_or(0) as u64,
                },
            );
        }
    }

    async fn handle_remove(
        &mut self,
        delete: ExperienceState,
        database_name: entity::shared::Region,
    ) {
        let id = delete.entity_id as i64;
        let mut total_exp = 0;
        let vec_es = delete
            .experience_stacks
            .iter()
            .map(|es| {
                if let Some(index) = self.messages.iter().position(|value| {
                    value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id
                }) {
                    self.messages.remove(index);
                }
                total_exp.add_assign(es.quantity);

                ::entity::experience_state::Model {
                    entity_id: id,
                    skill_id: es.skill_id,
                    experience: es.quantity,
                    region: database_name,
                }
            })
            .collect::<Vec<_>>();

        for es in vec_es {
            self.messages_delete.push((es.entity_id, es.skill_id));
        }
    }

    async fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages).await;
    }

    async fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ExperienceState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for ExperienceStateWorker {
    type Entity = ExperienceState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "player_state"
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

    fn reset_batch(&mut self) {
        self.ids.clear();
    }

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<ExperienceState>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages().await;
        self.flush_deletes().await;
    }
}

async fn insert_multiple_experience_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: Vec<::entity::experience_state::ActiveModel>,
) {
    let result = ::entity::experience_state::Entity::insert_many(messages)
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving experience_state"
        );
    }
}
