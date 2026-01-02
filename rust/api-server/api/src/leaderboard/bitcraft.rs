use crate::AppState;
use crate::leaderboard::{Leaderboard, experience_to_level};
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use game_module::module_bindings::ExperienceState;
use migration::{OnConflict, sea_query};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter};
use std::collections::HashMap;
use std::ops::AddAssign;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

pub(crate) fn start_worker_experience_state(
    global_app_state: AppState,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([::entity::experience_state::Column::Experience])
        .to_owned();

        loop {
            let mut messages = Vec::with_capacity(batch_size + 10);
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                let mut buffer = vec![];
                let fill_buffer_with = batch_size
                    .saturating_sub(buffer.len())
                    .saturating_sub(messages.len());

                tokio::select! {
                    _count = rx.recv_many(&mut buffer, fill_buffer_with) => {
                        for msg in buffer {
                            match msg {
                                SpacetimeUpdateMessages::Initial { data, database_name, .. } => {
                                    tracing::debug!("Processed Initial ExperienceState {}", data.len());
                                    let mut local_messages = Vec::with_capacity(batch_size + 10);
                                    let mut currently_known_experience_state = ::entity::experience_state::Entity::find()
                                        .filter(::entity::building_state::Column::Region.eq(database_name.to_string()))
                                        .all(&global_app_state.conn)
                                        .await
                                        .map_or(vec![], |aa| aa)
                                        .into_par_iter()
                                        .map(|value| {
                                            let entity_id = value.entity_id;
                                            let skill_id = value.skill_id;
                                            (format!("{entity_id}:{skill_id}"), value)
                                        })
                                        .collect::<HashMap<_, _>>();

                                    for model in data.into_iter().flat_map(|value| {
                                        let id = value.entity_id;
                                        let mut total_exp = 0;
                                        let model: Vec<::entity::experience_state::Model> = value.experience_stacks.iter().map(|exp_stack| {
                                                total_exp.add_assign(exp_stack.quantity as i64);
                                                ::entity::experience_state::Model {
                                                    entity_id: id as i64,
                                                    skill_id: exp_stack.skill_id,
                                                    experience: exp_stack.quantity,
                                                    region: database_name.to_string()
                                                }
                                            }).collect();

                                        global_app_state.ranking_system.global_leaderboard.update(value.entity_id as i64, total_exp);

                                        let mut xp_per_hour = 0;
                                        if let Some(player_state) = global_app_state.player_state.get(&(value.entity_id as i64)) {
                                            if player_state.time_signed_in >= 3600 {
                                                xp_per_hour = total_exp / (player_state.time_signed_in as i64/ 3600);
                                            }
                                        }
                                        global_app_state.ranking_system.xp_per_hour.update(value.entity_id as i64, xp_per_hour);

                                        model
                                    }).collect::<Vec<_>>() {
                                        let key = format!("{}:{}", model.entity_id, model.skill_id);
                                        use std::collections::hash_map::Entry;
                                        match currently_known_experience_state.entry(key) {
                                            Entry::Occupied(entry) => {
                                                let existing_model = entry.get();
                                                if &model != existing_model {
                                                    local_messages.push(model.into_active_model());
                                                }
                                                entry.remove();
                                            }
                                            Entry::Vacant(_entry) => {
                                                local_messages.push(model.into_active_model());
                                            }
                                        }
                                        if local_messages.len() >= batch_size {
                                           tracing::debug!("Initial Processing {} messages in batch", local_messages.len());
                                           insert_multiple_experience_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                        }
                                    };
                                    if !local_messages.is_empty() {
                                        tracing::debug!("Last Initial Processing {} messages in batch", local_messages.len());
                                        insert_multiple_experience_state(&global_app_state, &on_conflict, &mut local_messages).await;
                                    }

                                    for chunk_ids in currently_known_experience_state.into_keys().collect::<Vec<_>>().chunks(1000) {
                                        let chunk_ids = chunk_ids.to_vec();
                                        if let Err(error) = ::entity::experience_state::Entity::delete_many().filter(::entity::experience_state::Column::EntityId.is_in(chunk_ids.clone())).exec(&global_app_state.conn).await {
                                            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
                                            tracing::error!(ExperienceState = chunk_ids_str.join(","), error = error.to_string(), "Could not delete ExperienceState");
                                        }
                                    }
                                    tracing::debug!("Processed Initial ExperienceState {}", database_name.to_string());
                                }
                                SpacetimeUpdateMessages::Insert { new, database_name, .. } => {
                                    let id = new.entity_id as i64;
                                    let mut total_exp = 0;
                                    new.experience_stacks.iter().for_each(|es| {
                                        total_exp.add_assign(es.quantity as i64);
                                        let model = ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        };

                                        if let Some(index) = messages.iter().position(|value: &::entity::experience_state::ActiveModel| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }
                                        messages.push(model.into_active_model());
                                    });

                                    let current_score = global_app_state.ranking_system.global_leaderboard.scores.get(&(new.entity_id as i64));
                                    let mut xp_per_hour = 0;
                                    if let Some(player_state) = global_app_state.player_state.get(&(new.entity_id as i64)) {
                                        if player_state.time_signed_in >= 3600 {
                                            xp_per_hour = total_exp / (player_state.time_signed_in as i64/ 3600);
                                        }
                                    }
                                    global_app_state.ranking_system.xp_per_hour.update(new.entity_id as i64, xp_per_hour);

                                    if let Some(current_score) = current_score {
                                        let current_known_xp = current_score.value().clone();

                                        if current_known_xp < total_exp {
                                            global_app_state.ranking_system.global_leaderboard.update(new.entity_id as i64, total_exp);
                                            let mut xp_per_hour = 0;
                                            if let Some(player_state) = global_app_state.player_state.get(&(new.entity_id as i64)) {
                                                if player_state.time_signed_in >= 3600 {
                                                    xp_per_hour = total_exp / (player_state.time_signed_in as i64/ 3600);
                                                }
                                            }
                                            global_app_state.ranking_system.xp_per_hour.update(new.entity_id as i64, xp_per_hour);
                                        }
                                    } else {
                                        global_app_state.ranking_system.global_leaderboard.update(new.entity_id as i64, total_exp);
                                        let mut xp_per_hour = 0;
                                        if let Some(player_state) = global_app_state.player_state.get(&(new.entity_id as i64)) {
                                            if player_state.time_signed_in >= 3600 {
                                                xp_per_hour = total_exp / (player_state.time_signed_in as i64/ 3600);
                                            }
                                        }
                                        global_app_state.ranking_system.xp_per_hour.update(new.entity_id as i64, xp_per_hour);
                                    }

                                    if messages.len() >= batch_size {
                                        insert_multiple_experience_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Update { new, old, database_name, .. } => {
                                    let id = new.entity_id as i64;

                                    let mut new_level_vec = vec![];

                                    let mut new_total_exp = 0;
                                    let mut new_total_level = 0;
                                    new.experience_stacks.iter().for_each(|es| {
                                        new_total_exp.add_assign(es.quantity as i64);
                                        let new_level = experience_to_level(es.quantity as i64);
                                        new_total_level.add_assign(new_level);
                                        new_level_vec.push((
                                            es.clone(),
                                            new_level,
                                        ));

                                        let model = ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        };

                                        if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }
                                        messages.push(model.into_active_model());
                                    });

                                    let mut old_total_exp = 0;
                                    let mut old_total_level = 0;
                                    for es in old.experience_stacks.iter() {
                                        old_total_exp.add_assign(es.quantity as i64);
                                        let old_level =
                                            experience_to_level(es.quantity as i64);
                                        old_total_level.add_assign(old_level);

                                        let new_level = new_level_vec.iter().find(|new_level| new_level.0.skill_id.eq(&es.skill_id));
                                        let skill_name = global_app_state.skill_desc.get(&(es.skill_id as i64));

                                        if let Some(skill) = skill_name {
                                            if let Some(new_level) = new_level {
                                                let skill_name = skill.to_owned().name;
                                                if old_level != new_level.1 {

                                                    let _ = global_app_state.tx.send(WebSocketMessages::Level {
                                                        level: new_level.1 as u64,
                                                        skill_name: skill_name.clone(),
                                                        user_id: id,
                                                    });
                                                }

                                                if new_level.0.quantity > es.quantity {
                                                    if skill.skill_category != 0 {
                                                        let mut prev_rank = None;
                                                        let mut post_rank = None;
                                                        if let Some(skill_leaderboard) = global_app_state.ranking_system.skill_leaderboards.get_mut(&(es.skill_id as i64)) {
                                                            prev_rank = skill_leaderboard.get_rank(new.entity_id as i64);
                                                            skill_leaderboard.update(new.entity_id as i64, new_level.0.quantity as i64);
                                                            post_rank = skill_leaderboard.get_rank(new.entity_id as i64);
                                                        } else {
                                                            global_app_state.ranking_system.skill_leaderboards.insert(es.skill_id as i64, Leaderboard::default());
                                                            global_app_state.ranking_system.skill_leaderboards.get_mut(&(es.skill_id as i64)).unwrap().update(new.entity_id as i64, new_level.0.quantity as i64);
                                                            post_rank = global_app_state.ranking_system.skill_leaderboards.get_mut(&(es.skill_id as i64)).unwrap().get_rank(new.entity_id as i64);
                                                        }

                                                        match (prev_rank, post_rank) {
                                                            (Some(prev_rank), Some(post_rank)) => {
                                                                if prev_rank != post_rank {
                                                                    tracing::debug!("Skill EXP rank {skill_name} changed {prev_rank} to {post_rank}");
                                                                }
                                                            }
                                                            (Some(prev_rank), None) => {
                                                                tracing::debug!("Skill EXP rank {skill_name} changed {prev_rank} to no post_rank");
                                                            }
                                                            (None, Some(post_rank)) => {
                                                                tracing::debug!("Skill EXP rank {skill_name} changed no prev_rank to {post_rank}");
                                                            }
                                                            (None, None) => {
                                                                tracing::error!("Skill EXP {skill_name} no rank?");
                                                            }
                                                        }

                                                        let _ = global_app_state.tx.send(WebSocketMessages::Experience {
                                                            level: new_level.1 as u64,
                                                            experience: new_level.0.quantity as u64,
                                                            rank: post_rank.unwrap_or_else(|| 0) as u64,
                                                            skill_name,
                                                            user_id: id,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    };

                                    if old_total_level != new_total_level {
                                        let prev_rank = global_app_state.ranking_system.level_leaderboard.get_rank(new.entity_id as i64);
                                        global_app_state.ranking_system.level_leaderboard.update(new.entity_id as i64, new_total_level as i64);
                                        let post_rank = global_app_state.ranking_system.level_leaderboard.get_rank(new.entity_id as i64);

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
                                        let prev_rank = global_app_state.ranking_system.global_leaderboard.get_rank(new.entity_id as i64);
                                        global_app_state.ranking_system.global_leaderboard.update(new.entity_id as i64, new_total_exp);
                                        let mut xp_per_hour = 0;
                                        if let Some(player_state) = global_app_state.player_state.get(&(new.entity_id as i64)) {
                                            if player_state.time_signed_in >= 3600 {
                                                xp_per_hour = new_total_exp / (player_state.time_signed_in as i64/ 3600);
                                            }
                                        }
                                        global_app_state.ranking_system.xp_per_hour.update(new.entity_id as i64, xp_per_hour);

                                        let post_rank = global_app_state.ranking_system.global_leaderboard.get_rank(new.entity_id as i64);

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

                                        let _ = global_app_state.tx.send(WebSocketMessages::TotalExperience {
                                            experience: new_total_exp as u64,
                                            user_id: id,
                                            experience_per_hour: xp_per_hour as u64,
                                            rank: post_rank.unwrap_or_else(|| 0) as u64
                                        });
                                    }

                                    if messages.len() >= batch_size {
                                        insert_multiple_experience_state(&global_app_state, &on_conflict, &mut messages)
                                            .await;
                                    }
                                }
                                SpacetimeUpdateMessages::Remove { delete, database_name, .. } => {
                                    let id = delete.entity_id as i64;
                                    let mut total_exp = 0;
                                    let vec_es = delete.experience_stacks.iter().map(|es| {
                                        if let Some(index) = messages.iter().position(|value| value.skill_id.as_ref() == &es.skill_id && value.entity_id.as_ref() == &id) {
                                            messages.remove(index);
                                        }
                                        total_exp.add_assign(es.quantity);

                                        ::entity::experience_state::Model {
                                            entity_id: id,
                                            skill_id: es.skill_id,
                                            experience: es.quantity,
                                            region: database_name.to_string()
                                        }
                                    }).collect::<Vec<_>>();

                                    for es in vec_es {
                                        global_app_state.ranking_system.global_leaderboard.remove(es.entity_id);

                                        if let Err(error) = es.delete(&global_app_state.conn).await {
                                            tracing::error!(ExperienceState = id, error = error.to_string(), "Could not delete ExperienceState");
                                        }
                                    }
                                    tracing::debug!("ExperienceState::Remove");
                                }
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
                tracing::debug!("Processing {} messages in batch", messages.len());
                insert_multiple_experience_state(&global_app_state, &on_conflict, &mut messages)
                    .await;

                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                tracing::warn!(
                    "Shutting down ExperienceState worker as there no messages and rx is closed"
                );
                break;
            }
        }
    });
}

async fn insert_multiple_experience_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::experience_state::ActiveModel>,
) {
    let result = ::entity::experience_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving experience_state"
        );
    }

    messages.clear();
}
