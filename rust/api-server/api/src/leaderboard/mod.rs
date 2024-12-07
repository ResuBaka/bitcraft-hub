use crate::claims::ClaimDescriptionState;
use crate::websocket::Table;
use crate::{leaderboard, AppState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::experience_state;
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::IntoActiveModel;
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::Query;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;

#[macro_export]
macro_rules! generate_mysql_sum_level_sql_statement {
    ($experience_per_level:expr) => {{
        let mut sql = String::new();
        sql.push_str("SUM(CASE ");
        for (level, xp) in $experience_per_level.iter().rev() {
            sql.push_str(format!("WHEN experience > {xp} THEN {level} ").as_str());
        }
        sql.push_str("ELSE 0 END)");
        sql
    }};
}

pub(crate) const EXCLUDED_USERS_FROM_LEADERBOARD: [i64; 1] = [18695126];

pub(crate) const EXPERIENCE_PER_LEVEL: [(i32, i64); 100] = [
    (1, 0),
    (2, 640),
    (3, 1_330),
    (4, 2_090),
    (5, 2_920),
    (6, 3_830),
    (7, 4_820),
    (8, 5_890),
    (9, 7_070),
    (10, 8_350),
    (11, 9_740),
    (12, 11_260),
    (13, 12_920),
    (14, 14_730),
    (15, 16_710),
    (16, 18_860),
    (17, 21_210),
    (18, 23_770),
    (19, 26_560),
    (20, 29_600),
    (21, 32_920),
    (22, 36_550),
    (23, 40_490),
    (24, 44_800),
    (25, 49_490),
    (26, 54_610),
    (27, 60_200),
    (28, 66_290),
    (29, 72_930),
    (30, 80_170),
    (31, 88_060),
    (32, 96_670),
    (33, 106_060),
    (34, 116_300),
    (35, 127_470),
    (36, 139_650),
    (37, 152_930),
    (38, 167_410),
    (39, 183_200),
    (40, 200_420),
    (41, 219_200),
    (42, 239_680),
    (43, 262_020),
    (44, 286_370),
    (45, 312_930),
    (46, 341_890),
    (47, 373_480),
    (48, 407_920),
    (49, 445_480),
    (50, 486_440),
    (51, 531_110),
    (52, 579_820),
    (53, 632_940),
    (54, 690_860),
    (55, 754_030),
    (56, 822_920),
    (57, 898_040),
    (58, 979_960),
    (59, 1_069_290),
    (60, 1_166_710),
    (61, 1_272_950),
    (62, 1_388_800),
    (63, 1_515_140),
    (64, 1_652_910),
    (65, 1_803_160),
    (66, 1_967_000),
    (67, 2_145_660),
    (68, 2_340_500),
    (69, 2_552_980),
    (70, 2_784_680),
    (71, 3_037_360),
    (72, 3_312_900),
    (73, 3_613_390),
    (74, 3_941_070),
    (75, 4_298_410),
    (76, 4_688_090),
    (77, 5_113_030),
    (78, 5_576_440),
    (79, 6_081_800),
    (80, 6_632_890),
    (81, 7_233_850),
    (82, 7_889_210),
    (83, 8_603_890),
    (84, 9_383_250),
    (85, 10_233_150),
    (86, 11_159_970),
    (87, 12_170_670),
    (88, 13_272_850),
    (89, 14_474_790),
    (90, 15_785_510),
    (91, 17_214_860),
    (92, 18_773_580),
    (93, 20_473_370),
    (94, 22_327_010),
    (95, 24_348_420),
    (96, 26_552_780),
    (97, 28_956_650),
    (98, 31_578_090),
    (99, 34_436_800),
    (100, 37_554_230),
];

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/leaderboard", get(leaderboard::get_top_100))
        .route("/experience/:player_id", get(player_leaderboard))
        .route(
            "/api/bitcraft/experience/:player_id",
            get(player_leaderboard),
        )
        .route(
            "/api/bitcraft/leaderboard/claims/:claim_id",
            get(get_claim_leaderboard),
        )
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub(crate) enum RankType {
    Experience(LeaderboardExperience),
    ExperiencePerHour(LeaderboardExperiencePerHour),
    Level(LeaderboardLevel),
    Skill(LeaderboardSkill),
    Time(LeaderboardTime),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct LeaderboardSkill {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) level: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct LeaderboardLevel {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) level: u32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct LeaderboardExperiencePerHour {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct LeaderboardExperience {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) experience_per_hour: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct LeaderboardTime {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) time_played: u64,
    pub(crate) rank: u64,
}

pub(crate) async fn get_top_100(
    state: State<AppState>,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: Vec<
        tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>,
    > = vec![];

    for skill in skills {
        if skill.name == "ANY" {
            continue;
        }

        if skill.name == "Lore Keeper" {
            continue;
        }

        if skill.name == "Trading" {
            continue;
        }

        if skill.name == "Taming" {
            continue;
        }

        if skill.name == "Exploration" {
            continue;
        }

        let db = state.conn.clone();
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<RankType> = Vec::new();
            let entries = Query::get_experience_state_top_100_by_skill_id(
                &db,
                skill.id,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            for (i, entry) in entries.into_iter().enumerate() {
                let rank = i + 1;
                let player_name = None;

                leaderboard.push(RankType::Skill(LeaderboardSkill {
                    player_id: entry.entity_id,
                    player_name,
                    experience: entry.experience,
                    level: experience_to_level(entry.experience.clone() as i64),
                    rank: rank as u64,
                }));
            }

            Ok((skill.name.clone(), leaderboard))
        }));
    }

    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_top_100_total_experience(
            &db,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Experience(LeaderboardExperience {
                player_id: entry.0,
                player_name: None,
                experience: entry.1 as i32,
                experience_per_hour: entry.2 as i32,
                rank: rank as u64,
            }));
        }

        Ok(("Experience".to_string(), leaderboard))
    }));

    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_top_100_experience_per_hour(
            &db,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::ExperiencePerHour(LeaderboardExperiencePerHour {
                player_id: entry.entity_id,
                player_name: None,
                experience: entry.experience,
                rank: rank as u64,
            }));
        }

        Ok(("Experience Per Hour".to_string(), leaderboard))
    }));

    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_top_100_total_level(
            &db,
            generated_level_sql,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Level(LeaderboardLevel {
                player_id: entry.0 as i64,
                player_name: None,
                level: entry.1 as u32,
                rank: rank as u64,
            }));
        }

        Ok(("Level".to_string(), leaderboard))
    }));

    let results = futures::future::join_all(tasks).await;
    let mut player_ids: Vec<i64> = vec![];

    for results_inner in results {
        if let Ok(result) = results_inner {
            if let Ok((name, mut leaderboard)) = result {
                player_ids.append(
                    &mut leaderboard
                        .iter()
                        .map(|x| match x {
                            RankType::Skill(x) => x.player_id,
                            RankType::Level(x) => x.player_id,
                            RankType::Experience(x) => x.player_id,
                            RankType::Time(x) => x.player_id,
                            RankType::ExperiencePerHour(x) => x.player_id,
                        })
                        .collect::<Vec<i64>>(),
                );

                leaderboard_result
                    .entry(name)
                    .or_insert(Vec::new())
                    .append(&mut leaderboard);
            } else {
                error!("Error: {result:?}");
            }
        }
    }

    player_ids.sort();
    player_ids.dedup();

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, player_ids.clone())
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for (_, top) in leaderboard_result.iter_mut() {
        for x in top.iter_mut() {
            match x {
                RankType::Skill(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Level(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Experience(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Time(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::ExperiencePerHour(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
            };
        }
    }

    let players = Query::find_player_by_ids(&state.conn, player_ids.clone())
        .await
        .unwrap_or_else(|error| {
            error!("Error loading players: {error}");

            vec![]
        })
        .iter()
        .map(|player| (player.entity_id, player.time_signed_in))
        .collect::<HashMap<i64, i32>>();

    Ok(Json(serde_json::json!({
        "player_map": players,
        "leaderboard": leaderboard_result
    })))
}

pub(crate) fn experience_to_level(experience: i64) -> i32 {
    if experience == 0 {
        return 1;
    }

    for (level, xp) in EXPERIENCE_PER_LEVEL.iter().rev() {
        if experience.gt(xp) {
            return *level;
        }
    }

    100i32
}

#[allow(dead_code)]
pub(crate) async fn load_experience_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<experience_state::Model>> {
    let item_file = File::open(storage_path.join("State/ExperienceState.json"))?;
    let experience_state: Value = serde_json::from_reader(&item_file)?;
    let experience_states: Vec<experience_state::Model> =
        serde_json::from_value::<Vec<serde_json::Value>>(
            experience_state
                .get(0)
                .unwrap()
                .get("rows")
                .unwrap()
                .clone(),
        )?
        .into_iter()
        .map(|x| row_to_xp_values(x))
        .flatten()
        .collect();

    Ok(experience_states)
}

#[derive(Debug, Deserialize, Clone)]
struct PackedExperienceState {
    entity_id: i64,
    experience_stacks: Vec<(i32, i32)>,
}

async fn get_known_experience_states(
    conn: &DatabaseConnection,
) -> anyhow::Result<HashSet<(i64, i32)>> {
    let experience_state_to_delete: Vec<(i64, i32)> = experience_state::Entity::find()
        .select_only()
        .column(experience_state::Column::EntityId)
        .column(experience_state::Column::SkillId)
        .into_tuple()
        .all(conn)
        .await?;

    let experience_state_to_delete = experience_state_to_delete
        .into_iter()
        .map(|x| (x.0, x.1))
        .collect::<HashSet<_>>();
    Ok(experience_state_to_delete)
}

async fn delete_experience_state(
    conn: &DatabaseConnection,
    experience_state_to_delete: &mut HashSet<(i64, i32)>,
) -> anyhow::Result<()> {
    info!(
        "experience_state's ({}) to delete: {:?}",
        experience_state_to_delete.len(),
        experience_state_to_delete
    );

    let filter = experience_state_to_delete
        .iter()
        .map(|x| {
            experience_state::Column::EntityId
                .eq(x.0)
                .and(experience_state::Column::SkillId.eq(x.1))
        })
        .reduce(|accum, x| accum.or(x))
        .unwrap();

    experience_state::Entity::delete_many()
        .filter(filter)
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_experience_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<PackedExperienceState>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let resolved_buffer_before_insert = buffer_before_insert
        .iter()
        .map(|x| {
            x.experience_stacks.iter().map(|y| experience_state::Model {
                entity_id: x.entity_id,
                skill_id: y.0,
                experience: y.1,
            })
        })
        .flatten()
        .collect::<Vec<experience_state::Model>>();

    let experience_state_from_db = experience_state::Entity::find()
        .filter(
            experience_state::Column::EntityId.is_in(
                resolved_buffer_before_insert
                    .iter()
                    .map(|experience_state| experience_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let experience_state_from_db_map = experience_state_from_db
        .into_iter()
        .map(|experience_state| {
            (
                (experience_state.entity_id, experience_state.skill_id),
                experience_state,
            )
        })
        .collect::<HashMap<(i64, i32), experience_state::Model>>();

    let things_to_insert = resolved_buffer_before_insert
        .iter()
        .filter(|experience_state| {
            match experience_state_from_db_map
                .get(&(experience_state.entity_id, experience_state.skill_id))
            {
                Some(experience_state_from_db) => {
                    if experience_state_from_db != *experience_state {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|experience_state| experience_state.clone().into_active_model())
        .collect::<Vec<experience_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} experience_state", things_to_insert.len());
    }

    for things_to_insert_chunk in things_to_insert.chunks(5000) {
        let _ = experience_state::Entity::insert_many(things_to_insert_chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(conn)
            .await?;
    }

    buffer_before_insert.clear();

    Ok(())
}

#[allow(dead_code)]
fn row_to_xp_values(row: Value) -> Vec<experience_state::Model> {
    let player_id = row.get(0).unwrap().as_i64().unwrap();
    row.get(1)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let skill_id = x.get(0).unwrap().as_i64().unwrap();
            let experience = x.get(1).unwrap().as_i64().unwrap();
            experience_state::Model {
                entity_id: player_id,
                skill_id: skill_id as i32,
                experience: experience as i32,
            }
        })
        .collect::<Vec<experience_state::Model>>()
}

pub(crate) async fn player_leaderboard(
    state: State<AppState>,
    Path(player_id): Path<i64>,
) -> Result<Json<BTreeMap<String, RankType>>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let mut leaderboard_result: BTreeMap<String, RankType> = BTreeMap::new();

    let mut tasks: Vec<
        tokio::task::JoinHandle<Result<(String, RankType), (StatusCode, &'static str)>>,
    > = vec![];

    for skill in skills {
        if skill.name == "ANY" {
            continue;
        }

        if skill.name == "Lore Keeper" {
            continue;
        }

        if skill.name == "Trading" {
            continue;
        }

        if skill.name == "Taming" {
            continue;
        }

        if skill.name == "Exploration" {
            continue;
        }

        let db = state.conn.clone();
        tasks.push(tokio::spawn(async move {
            let (entrie, rank) = Query::get_experience_state_player_by_skill_id(
                &db,
                skill.id,
                player_id,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            if entrie.is_none() {
                return Err((StatusCode::NOT_FOUND, ""));
            }

            let player_name = None;

            let entry = entrie.unwrap();

            Ok((
                skill.name.clone(),
                RankType::Skill(LeaderboardSkill {
                    player_id: entry.entity_id,
                    player_name,
                    experience: entry.experience,
                    level: experience_to_level(entry.experience.clone() as i64),
                    rank: rank.unwrap(),
                }),
            ))
        }));
    }

    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let (total_experience, rank) = Query::get_experience_state_player_rank_total_experience(
            &db,
            player_id,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        Ok((
            "Experience".to_string(),
            RankType::Experience(LeaderboardExperience {
                player_id,
                player_name: None,
                experience: total_experience.unwrap() as i32,
                experience_per_hour: 0,
                rank: rank.unwrap(),
            }),
        ))
    }));

    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

        let (level, rank) = Query::get_experience_state_player_level(
            &db,
            generated_level_sql,
            player_id,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        Ok((
            "Level".to_string(),
            RankType::Level(LeaderboardLevel {
                player_id,
                player_name: None,
                level: level.unwrap_or_default() as u32,
                rank: rank.unwrap_or_default(),
            }),
        ))
    }));

    let results = futures::future::join_all(tasks).await;

    for results_inner in results {
        if let Ok(result) = results_inner {
            if let Ok((name, leaderboard)) = result {
                leaderboard_result.entry(name).or_insert(leaderboard);
            } else {
                error!("Error: {result:?}");
            }
        }
    }

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, vec![player_id])
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for (_, top) in leaderboard_result.iter_mut() {
        match top {
            RankType::Skill(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Level(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Experience(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::Time(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
            RankType::ExperiencePerHour(x) => {
                x.player_name = players_name_by_id
                    .get(&x.player_id)
                    .or(None)
                    .map(|x| x.to_string());
            }
        };
    }

    Ok(Json(leaderboard_result))
}

pub(crate) async fn get_claim_leaderboard(
    state: State<AppState>,
    Path(claim_id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let claim = Query::find_claim_description_by_id(&state.conn, claim_id)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    if claim.is_none() {
        return Err((StatusCode::NOT_FOUND, ""));
    }

    let claim: ClaimDescriptionState = claim.unwrap().into();

    let player_ids = claim
        .members
        .iter()
        .map(|member| member.entity_id)
        .collect::<Vec<i64>>();

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: Vec<
        tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>,
    > = vec![];

    for skill in skills {
        if skill.name == "ANY" {
            continue;
        }

        if skill.name == "Lore Keeper" {
            continue;
        }

        if skill.name == "Trading" {
            continue;
        }

        if skill.name == "Taming" {
            continue;
        }

        if skill.name == "Exploration" {
            continue;
        }

        let db = state.conn.clone();
        let player_ids = player_ids.clone();
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<RankType> = Vec::new();
            let entries = Query::get_experience_state_player_ids_by_skill_id(
                &db,
                skill.id,
                player_ids,
                Some(EXCLUDED_USERS_FROM_LEADERBOARD),
            )
            .await
            .map_err(|error| {
                error!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            for (i, entry) in entries.into_iter().enumerate() {
                let rank = i + 1;
                let player_name = None;

                leaderboard.push(RankType::Skill(LeaderboardSkill {
                    player_id: entry.entity_id,
                    player_name,
                    experience: entry.experience,
                    level: experience_to_level(entry.experience.clone() as i64),
                    rank: rank as u64,
                }));
            }

            Ok((skill.name.clone(), leaderboard))
        }));
    }

    let db = state.conn.clone();
    let tmp_player_ids = player_ids.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_total_experience(
            &db,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Experience(LeaderboardExperience {
                player_id: entry.entity_id,
                player_name: None,
                experience: entry.experience,
                experience_per_hour: 0,
                rank: rank as u64,
            }));
        }

        Ok(("Experience".to_string(), leaderboard))
    }));

    let db = state.conn.clone();
    let tmp_player_ids = player_ids.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_player_ids_total_level(
            &db,
            generated_level_sql,
            tmp_player_ids,
            Some(EXCLUDED_USERS_FROM_LEADERBOARD),
        )
        .await
        .map_err(|error| {
            error!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Level(LeaderboardLevel {
                player_id: entry.0 as i64,
                player_name: None,
                level: entry.1 as u32,
                rank: rank as u64,
            }));
        }

        Ok(("Level".to_string(), leaderboard))
    }));

    let results = futures::future::join_all(tasks).await;
    let mut player_ids: Vec<i64> = vec![];

    for results_inner in results {
        if let Ok(result) = results_inner {
            if let Ok((name, mut leaderboard)) = result {
                player_ids.append(
                    &mut leaderboard
                        .iter()
                        .map(|x| match x {
                            RankType::Skill(x) => x.player_id,
                            RankType::Level(x) => x.player_id,
                            RankType::Experience(x) => x.player_id,
                            RankType::Time(x) => x.player_id,
                            RankType::ExperiencePerHour(x) => x.player_id,
                        })
                        .collect::<Vec<i64>>(),
                );

                leaderboard_result
                    .entry(name)
                    .or_insert(Vec::new())
                    .append(&mut leaderboard);
            } else {
                error!("Error: {result:?}");
            }
        }
    }

    player_ids.sort();
    player_ids.dedup();

    let players_name_by_id = Query::find_player_username_by_ids(&state.conn, player_ids.clone())
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?
        .into_iter()
        .map(|x| (x.entity_id, x.username))
        .collect::<HashMap<i64, String>>();

    for (_, top) in leaderboard_result.iter_mut() {
        for x in top.iter_mut() {
            match x {
                RankType::Skill(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Level(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Experience(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::Time(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
                RankType::ExperiencePerHour(x) => {
                    x.player_name = players_name_by_id
                        .get(&x.player_id)
                        .or(None)
                        .map(|x| x.to_string());
                }
            };
        }
    }

    let players = Query::find_player_by_ids(&state.conn, player_ids.clone())
        .await
        .unwrap_or_else(|error| {
            error!("Error loading players: {error}");

            vec![]
        })
        .iter()
        .map(|player| (player.entity_id, player.time_signed_in))
        .collect::<HashMap<i64, i32>>();

    Ok(Json(serde_json::json!({
        "player_map": players,
        "leaderboard": leaderboard_result
    })))
}

pub(crate) async fn handle_initial_subscription(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::columns([
        experience_state::Column::EntityId,
        experience_state::Column::SkillId,
    ])
    .update_columns([experience_state::Column::Experience])
    .to_owned();

    let chunk_size = Some(500);
    let mut buffer_before_insert: Vec<PackedExperienceState> =
        Vec::with_capacity(chunk_size.unwrap_or(500));
    let mut experience_state_to_delete = get_known_experience_states(p0).await?;
    for row in p1.inserts.iter() {
        match serde_json::from_str::<PackedExperienceState>(row.text.as_ref()) {
            Ok(experience_state) => {
                let resolved_buffer_before_insert = experience_state
                    .experience_stacks
                    .iter()
                    .map(|y| experience_state::Model {
                        entity_id: experience_state.entity_id,
                        skill_id: y.0,
                        experience: y.1,
                    })
                    .collect::<Vec<experience_state::Model>>();

                for exp_state in resolved_buffer_before_insert.iter() {
                    if experience_state_to_delete
                        .contains(&(exp_state.entity_id, exp_state.skill_id))
                    {
                        experience_state_to_delete
                            .remove(&(exp_state.entity_id, exp_state.skill_id));
                    }
                }

                buffer_before_insert.push(experience_state);
                if buffer_before_insert.len() == chunk_size.unwrap_or(500) {
                    db_insert_experience_state(p0, &mut buffer_before_insert, &on_conflict).await?;
                }
            }
            Err(error) => {
                error!("Error: {error} for row: {:?}", row.text);
            }
        }
    }

    if buffer_before_insert.len() > 0 {
        db_insert_experience_state(p0, &mut buffer_before_insert, &on_conflict).await?;
        info!("experience_state last batch imported");
    }

    if experience_state_to_delete.len() > 0 {
        delete_experience_state(p0, &mut experience_state_to_delete).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    p0: &DatabaseConnection,
    tables: &Vec<Table>,
    skill_id_to_skill_name: &HashMap<i64, String>,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::columns([
        experience_state::Column::EntityId,
        experience_state::Column::SkillId,
    ])
    .update_columns([experience_state::Column::Experience])
    .to_owned();

    // let mut found_in_inserts = HashSet::new();
    let mut skills_to_update = HashMap::new();

    let mut potential_deletes = HashSet::new();

    for p1 in tables.iter() {
        let event_type = if p1.inserts.len() > 0 && p1.deletes.len() > 0 {
            "update"
        } else if p1.inserts.len() > 0 && p1.deletes.len() == 0 {
            "insert"
        } else if p1.deletes.len() > 0 && p1.inserts.len() == 0 {
            "delete"
        } else {
            "unknown"
        };

        if event_type == "unknown" {
            error!("Unknown event type {:?}", p1);
            continue;
        }

        if event_type == "delete" {
            for row in p1.deletes.iter() {
                match serde_json::from_str::<PackedExperienceState>(row.text.as_ref()) {
                    Ok(experience_state) => {
                        experience_state
                            .experience_stacks
                            .iter()
                            .map(|y| experience_state::Model {
                                entity_id: experience_state.entity_id,
                                skill_id: y.0,
                                experience: y.1,
                            })
                            .for_each(|x| {
                                potential_deletes.insert((x.entity_id, x.skill_id));
                            });
                    }
                    Err(error) => {
                        error!("Error: {error} for row: {:?}", row.text);
                    }
                }
            }
        } else if event_type == "update" {
            for row in p1.inserts.iter().enumerate() {
                match (
                    serde_json::from_str::<PackedExperienceState>(row.1.text.as_ref()),
                    serde_json::from_str::<PackedExperienceState>(
                        p1.deletes.get(row.0).unwrap().text.as_ref(),
                    ),
                ) {
                    (Ok(new_experience_state), Ok(old_experience_state)) => {
                        let new_experience_state_map = new_experience_state
                            .experience_stacks
                            .iter()
                            .map(|y| {
                                (
                                    (new_experience_state.entity_id, y.0),
                                    experience_state::Model {
                                        entity_id: new_experience_state.entity_id,
                                        skill_id: y.0,
                                        experience: y.1,
                                    },
                                )
                            })
                            .collect::<HashMap<(i64, i32), experience_state::Model>>();

                        let old_experience_state_map = old_experience_state
                            .experience_stacks
                            .iter()
                            .map(|y| {
                                (
                                    (old_experience_state.entity_id, y.0),
                                    experience_state::Model {
                                        entity_id: old_experience_state.entity_id,
                                        skill_id: y.0,
                                        experience: y.1,
                                    },
                                )
                            })
                            .collect::<HashMap<(i64, i32), experience_state::Model>>();

                        for (key, value) in new_experience_state_map.iter() {
                            let skill_id = key.1 as i64;
                            let skill_name =
                                skill_id_to_skill_name.get(&skill_id).unwrap().to_owned();

                            if old_experience_state_map.contains_key(key) {
                                let old_value = old_experience_state_map.get(key).unwrap();

                                if old_value != value {
                                    skills_to_update
                                        .insert(*key, value.clone().into_active_model());

                                    metrics::counter!(
                                        "player_skill_experience_count",
                                        &[("skill_name", skill_name),]
                                    )
                                    .increment((value.experience - old_value.experience) as u64);
                                }
                            } else {
                                skills_to_update.insert(*key, value.clone().into_active_model());

                                metrics::counter!(
                                    "player_skill_experience_count",
                                    &[("skill_name", skill_name),]
                                )
                                .increment(value.experience as u64);
                            }

                            potential_deletes.remove(key);
                        }
                    }
                    (Err(error), _) => {
                        error!(
                            "Could not parse new experience state: {error}, row: {}",
                            row.1.text
                        );
                    }
                    (_, Err(error)) => {
                        error!(
                            "Could not parse old experience state: {error}, row: {}",
                            p1.deletes.get(row.0).unwrap().text
                        );
                    }
                }
            }
        } else if event_type == "insert" {
            for row in p1.inserts.iter() {
                match serde_json::from_str::<PackedExperienceState>(row.text.as_ref()) {
                    Ok(experience_state) => {
                        experience_state
                            .experience_stacks
                            .iter()
                            .map(|y| experience_state::Model {
                                entity_id: experience_state.entity_id,
                                skill_id: y.0,
                                experience: y.1,
                            })
                            .for_each(|x| {
                                let skill_id = x.skill_id as i64;
                                let skill_name =
                                    skill_id_to_skill_name.get(&skill_id).unwrap().to_owned();

                                let key = (x.entity_id, x.skill_id);
                                potential_deletes.remove(&key);
                                skills_to_update.insert(key, x.clone().into_active_model());

                                metrics::counter!(
                                    "player_skill_experience_count",
                                    &[("skill_name", skill_name),]
                                )
                                .increment(x.experience as u64);
                            });
                    }
                    Err(error) => {
                        error!("Error: {error} for row: {:?}", row.text);
                    }
                }
            }
        }
    }

    let skills_to_update_vec = skills_to_update
        .into_iter()
        .map(|x| x.1)
        .collect::<Vec<experience_state::ActiveModel>>();

    for skill_entries_to_insert_chunk in skills_to_update_vec.chunks(5000) {
        let result = experience_state::Entity::insert_many(skill_entries_to_insert_chunk.to_vec())
            .on_conflict(on_conflict.clone())
            .exec(p0)
            .await;

        if let Err(error) = result {
            error!(
                "Error: {error} for chunk: {:?}",
                skill_entries_to_insert_chunk
            );
            return Err(error.into());
        }
    }

    if potential_deletes.len() > 0 {
        delete_experience_state(p0, &mut potential_deletes).await?;
    }

    Ok(())
}
