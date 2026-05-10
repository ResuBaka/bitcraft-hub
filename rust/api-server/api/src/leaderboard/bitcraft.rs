use crate::AppState;
use crate::leaderboard::{Leaderboard, experience_to_level};
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages, record_worker_received};
use chrono::DateTime;
use game_module::module_bindings::ExperienceState;
use migration::{OnConflict, sea_query};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use spacetimedb_sdk::Event;
use std::collections::HashMap;
use std::ops::AddAssign;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::sleep;

enum ExperienceStateDbOperation {
    Upsert(Vec<::entity::experience_state::ActiveModel>),
    Delete(Vec<(i64, i32)>),
    DeleteForRegion {
        ids: Vec<String>,
        region: entity::shared::Region,
    },
}

pub(crate) struct ExperienceStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<ExperienceStateDbOperation>,
    messages: Vec<::entity::experience_state::ActiveModel>,
    ids: Vec<i64>,
    messages_delete: Vec<(i64, i32)>,
}

fn start_experience_state_db_worker(
    global_app_state: AppState,
) -> UnboundedSender<ExperienceStateDbOperation> {
    let (tx, mut rx) = unbounded_channel();
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
                ExperienceStateDbOperation::DeleteForRegion { ids, region } => {
                    let mut query = sea_query::Condition::any();
                    for chunk_id in &ids {
                        let (entity_id, skill_id) = chunk_id.split_once(":").unwrap();
                        query = query.add(
                            ::entity::experience_state::Column::EntityId
                                .eq(entity_id.parse::<i64>().unwrap())
                                .and(
                                    ::entity::experience_state::Column::SkillId
                                        .eq(skill_id.parse::<i32>().unwrap()),
                                ),
                        );
                    }

                    let chunk_ids = ids.to_vec();
                    if let Err(error) = ::entity::experience_state::Entity::delete_many()
                        .filter(query)
                        .exec(&global_app_state.conn)
                        .await
                    {
                        let chunk_ids_str: Vec<String> =
                            chunk_ids.iter().map(|id| id.to_string()).collect();
                        tracing::error!(
                            ExperienceState = chunk_ids_str.join(","),
                            error = error.to_string(),
                            "Could not delete ExperienceState"
                        );
                    }
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

    fn queue_upserts(&self, messages: Vec<::entity::experience_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::Upsert(messages))
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<(i64, i32)>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ExperienceStateDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState deletes"
            );
        }
    }

    fn queue_region_deletes(&self, ids: Vec<String>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::DeleteForRegion { ids, region })
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState region deletes"
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
                reducer_name,
                ..
            } => {
                self.handle_remove(delete, database_name, reducer_name)
                    .await;
            }
        }
    }

    async fn handle_initial(
        &mut self,
        data: Vec<ExperienceState>,
        database_name: entity::shared::Region,
    ) {
        tracing::debug!("Processed Initial ExperienceState {}", data.len());
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_experience_state = ::entity::experience_state::Entity::find()
            .filter(::entity::experience_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole experience_state state"
                );
                vec![]
            })
            .into_par_iter()
            .map(|value| {
                let entity_id = value.entity_id;
                let skill_id = value.skill_id;
                (format!("{entity_id}:{skill_id}"), value)
            })
            .collect::<HashMap<_, _>>();

        for model in data
            .into_iter()
            .flat_map(|value| {
                let id = value.entity_id;
                let mut total_exp = 0;
                let model: Vec<::entity::experience_state::Model> = value
                    .experience_stacks
                    .iter()
                    .map(|exp_stack| {
                        total_exp.add_assign(exp_stack.quantity as i64);
                        ::entity::experience_state::Model {
                            entity_id: id as i64,
                            skill_id: exp_stack.skill_id,
                            experience: exp_stack.quantity,
                            region: database_name,
                        }
                    })
                    .collect();

                if let Some(xp) = self
                    .global_app_state
                    .ranking_system
                    .global_leaderboard
                    .get_value(&(value.entity_id as i64))
                {
                    if xp != total_exp {
                        self.global_app_state
                            .ranking_system
                            .global_leaderboard
                            .update(value.entity_id as i64, total_exp);

                        let mut xp_per_hour = 0;
                        if let Some(player_state) = self
                            .global_app_state
                            .player_state
                            .get(&(value.entity_id as i64))
                        {
                            if player_state.time_signed_in >= 3600 {
                                xp_per_hour =
                                    total_exp / (player_state.time_signed_in as i64 / 3600);
                            }
                        }
                        self.global_app_state
                            .ranking_system
                            .xp_per_hour
                            .update(value.entity_id as i64, xp_per_hour);
                    }
                }

                model
            })
            .collect::<Vec<_>>()
        {
            let key = format!("{}:{}", model.entity_id, model.skill_id);
            use std::collections::hash_map::Entry;
            match currently_known_experience_state.entry(key) {
                Entry::Occupied(entry) => {
                    let existing_model = entry.get();
                    if &model != existing_model {
                        if let Some(skill_leaderboard) = self
                            .global_app_state
                            .ranking_system
                            .skill_leaderboards
                            .get_mut(&(model.skill_id as i64))
                        {
                            skill_leaderboard.update(model.entity_id, model.experience as i64);
                        }

                        local_messages.push(model.into_active_model());
                    }
                    entry.remove();
                }
                Entry::Vacant(_entry) => {
                    local_messages.push(model.into_active_model());
                }
            }
            if local_messages.len() >= self.batch_size {
                self.queue_upserts(std::mem::take(&mut local_messages));
                local_messages = Vec::with_capacity(self.batch_size + 10);
            }
        }
        if !local_messages.is_empty() {
            self.queue_upserts(local_messages);
        }

        for chunk_ids in currently_known_experience_state
            .into_keys()
            .collect::<Vec<_>>()
            .chunks(100)
        {
            self.queue_region_deletes(chunk_ids.to_vec(), database_name);
        }
        tracing::debug!(
            "Processed Initial ExperienceState {}",
            database_name.to_string()
        );
    }

    async fn handle_insert(&mut self, new: ExperienceState, database_name: entity::shared::Region) {
        let id = new.entity_id as i64;
        let mut total_exp = 0;
        new.experience_stacks.iter().for_each(|es| {
            total_exp.add_assign(es.quantity as i64);
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
            .scores
            .get(&(new.entity_id as i64));
        let mut xp_per_hour = 0;
        if let Some(player_state) = self
            .global_app_state
            .player_state
            .get(&(new.entity_id as i64))
        {
            if player_state.time_signed_in >= 3600 {
                xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
            }
        }
        self.global_app_state
            .ranking_system
            .xp_per_hour
            .update(new.entity_id as i64, xp_per_hour);

        if let Some(current_score) = current_score {
            let current_known_xp = *current_score.value();

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
                {
                    if player_state.time_signed_in >= 3600 {
                        xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
                    }
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
            {
                if player_state.time_signed_in >= 3600 {
                    xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
                }
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
        new.experience_stacks.iter().for_each(|es| {
            new_total_exp.add_assign(es.quantity as i64);
            let new_level = experience_to_level(es.quantity as i64);
            new_total_level.add_assign(new_level);
        });

        let mut old_total_exp = 0;
        let mut old_total_level = 0;
        for (index, es) in old.experience_stacks.iter().enumerate() {
            old_total_exp.add_assign(es.quantity as i64);
            let old_level = experience_to_level(es.quantity as i64);
            old_total_level.add_assign(old_level);

            let new_skill = if let Some(new_skill) = new.experience_stacks.get(index) {
                if new_skill.skill_id == es.skill_id {
                    Some(new_skill)
                } else {
                    new.experience_stacks
                        .iter()
                        .find(|new_level| new_level.skill_id.eq(&es.skill_id))
                }
            } else {
                new.experience_stacks
                    .iter()
                    .find(|new_level| new_level.skill_id.eq(&es.skill_id))
            };
            if let Some(new_skill) = new_skill {
                let new_level = experience_to_level(new_skill.quantity as i64);
                if let Some(skill) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                    let skill_name = skill.to_owned().name;
                    if old_level != new_level {
                        let _ = self.global_app_state.tx.send(WebSocketMessages::Level {
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
                    {
                        if skill.skill_category != 0 {
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
                            let _ = self
                                .global_app_state
                                .tx
                                .send(WebSocketMessages::Experience {
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
            {
                if player_state.time_signed_in >= 3600 {
                    xp_per_hour = new_total_exp / (player_state.time_signed_in as i64 / 3600);
                }
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
                .send(WebSocketMessages::TotalExperience {
                    experience: new_total_exp as u64,
                    user_id: id,
                    experience_per_hour: xp_per_hour as u64,
                    rank: post_rank.unwrap_or(0) as u64,
                });
        }
    }

    async fn handle_remove(
        &mut self,
        delete: ExperienceState,
        database_name: entity::shared::Region,
        reducer_name: Option<&'static str>,
    ) {
        #[allow(clippy::single_match)]
        match reducer_name {
            Some("transfer_player_delayed") => {
                return;
            }
            _ => {}
        }

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

        tracing::debug!("ExperienceState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete);
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

    fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
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
