use crate::{AppRouter, AppState, leaderboard};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use log::error;
use serde::{Deserialize, Serialize};
use service::Query;
use std::collections::{BTreeMap, HashMap};

#[macro_export]
macro_rules! generate_mysql_sum_level_sql_statement {
    ($experience_per_level:expr) => {{
        let mut sql = String::new();
        sql.push_str("SUM(CASE ");
        for (level, xp) in $experience_per_level.iter().rev() {
            sql.push_str(format!("WHEN experience >= {xp} THEN {level} ").as_str());
        }
        sql.push_str("ELSE 0 END)");
        sql
    }};
}

pub(crate) const EXCLUDED_USERS_FROM_LEADERBOARD: [i64; 1] = [18695126];
pub(crate) const EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY: [i64; 2] = [0, 2];

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

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/leaderboard",
            axum_codec::routing::get(leaderboard::get_top_100).into(),
        )
        .route(
            "/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/experience/{player_id}",
            axum_codec::routing::get(player_leaderboard).into(),
        )
        .route(
            "/api/bitcraft/leaderboard/claims/{claim_id}",
            axum_codec::routing::get(get_claim_leaderboard).into(),
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

type LeaderboardRankTypeTasks =
    Vec<tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>>;

pub(crate) async fn get_top_100(
    state: State<std::sync::Arc<AppState>>,
) -> Result<axum_codec::Codec<GetTop100Response>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: LeaderboardRankTypeTasks = vec![];

    for skill in skills {
        if skill.skill_category == 2 || skill.skill_category == 0 {
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
                    level: experience_to_level(entry.experience as i64),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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

    for result in results.into_iter().flatten() {
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
                .or_default()
                .append(&mut leaderboard);
        } else {
            error!("Error: {result:?}");
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

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GetTop100Response {
    pub player_map: HashMap<i64, i32>,
    pub leaderboard: BTreeMap<String, Vec<RankType>>,
}

pub(crate) fn experience_to_level(experience: i64) -> i32 {
    if experience == 0 {
        return 1;
    }

    for (level, xp) in EXPERIENCE_PER_LEVEL.iter().rev() {
        if experience.gt(xp) || experience.eq(xp) {
            return *level;
        }
    }

    100i32
}

// #[allow(dead_code)]
// pub(crate) async fn load_experience_state_from_file(
//     storage_path: &std::path::Path,
// ) -> anyhow::Result<Vec<experience_state::Model>> {
//     let item_file = File::open(storage_path.join("State/ExperienceState.json"))?;
//     let experience_state: Value = serde_json::from_reader(&item_file)?;
//     let experience_states: Vec<experience_state::Model> =
//         serde_json::from_value::<Vec<serde_json::Value>>(
//             experience_state
//                 .get(0)
//                 .unwrap()
//                 .get("rows")
//                 .unwrap()
//                 .clone(),
//         )?
//         .into_iter()
//         .flat_map(row_to_xp_values)
//         .collect();
//
//     Ok(experience_states)
// }

// #[derive(Debug, Deserialize, Clone)]
// struct PackedExperienceState {
//     entity_id: i64,
//     experience_stacks: Vec<(i32, i32)>,
// }
//
// async fn get_known_experience_states(
//     conn: &DatabaseConnection,
// ) -> anyhow::Result<HashSet<(i64, i32)>> {
//     let experience_state_to_delete: Vec<(i64, i32)> = experience_state::Entity::find()
//         .select_only()
//         .column(experience_state::Column::EntityId)
//         .column(experience_state::Column::SkillId)
//         .into_tuple()
//         .all(conn)
//         .await?;
//
//     let experience_state_to_delete = experience_state_to_delete
//         .into_iter()
//         .map(|x| (x.0, x.1))
//         .collect::<HashSet<_>>();
//     Ok(experience_state_to_delete)
// }
//
// async fn delete_experience_state(
//     conn: &DatabaseConnection,
//     experience_state_to_delete: &mut HashSet<(i64, i32)>,
// ) -> anyhow::Result<()> {
//     info!(
//         "experience_state's ({}) to delete: {:?}",
//         experience_state_to_delete.len(),
//         experience_state_to_delete
//     );
//
//     let filter = experience_state_to_delete
//         .iter()
//         .map(|x| {
//             experience_state::Column::EntityId
//                 .eq(x.0)
//                 .and(experience_state::Column::SkillId.eq(x.1))
//         })
//         .reduce(|accum, x| accum.or(x))
//         .unwrap();
//
//     experience_state::Entity::delete_many()
//         .filter(filter)
//         .exec(conn)
//         .await?;
//     Ok(())
// }
//
// async fn db_insert_experience_state(
//     conn: &DatabaseConnection,
//     buffer_before_insert: &mut Vec<PackedExperienceState>,
//     on_conflict: &OnConflict,
// ) -> anyhow::Result<()> {
//     let mut experience_state_user_to_search = HashSet::new();
//
//     let resolved_buffer_before_insert = buffer_before_insert
//         .iter()
//         .flat_map(|x| {
//             experience_state_user_to_search.insert(x.entity_id);
//
//             x.experience_stacks.iter().map(|y| experience_state::Model {
//                 entity_id: x.entity_id,
//                 skill_id: y.0,
//                 experience: y.1,
//             })
//         })
//         .collect::<Vec<experience_state::Model>>();
//
//     let experience_state_from_db = experience_state::Entity::find()
//         .filter(experience_state::Column::EntityId.is_in(experience_state_user_to_search))
//         .all(conn)
//         .await?;
//
//     let experience_state_from_db_map = experience_state_from_db
//         .into_iter()
//         .map(|experience_state| {
//             (
//                 (experience_state.entity_id, experience_state.skill_id),
//                 experience_state,
//             )
//         })
//         .collect::<HashMap<(i64, i32), experience_state::Model>>();
//
//     let things_to_insert = resolved_buffer_before_insert
//         .into_iter()
//         .filter_map(|experience_state| {
//             match experience_state_from_db_map
//                 .get(&(experience_state.entity_id, experience_state.skill_id))
//             {
//                 Some(experience_state_from_db) if experience_state_from_db != &experience_state => {
//                     Some(experience_state.into_active_model())
//                 }
//                 None => Some(experience_state.into_active_model()),
//                 _ => None,
//             }
//         })
//         .collect::<Vec<experience_state::ActiveModel>>();
//
//     if things_to_insert.is_empty() {
//         debug!("Nothing to insert");
//         buffer_before_insert.clear();
//         return Ok(());
//     } else {
//         debug!("Inserting {} experience_state", things_to_insert.len());
//     }
//
//     for things_to_insert_chunk in things_to_insert.chunks(5000) {
//         let _ = experience_state::Entity::insert_many(things_to_insert_chunk.to_vec())
//             .on_conflict(on_conflict.clone())
//             .exec(conn)
//             .await?;
//     }
//
//     buffer_before_insert.clear();
//
//     Ok(())
// }
//
// #[allow(dead_code)]
// fn row_to_xp_values(row: Value) -> Vec<experience_state::Model> {
//     let player_id = row.get(0).unwrap().as_i64().unwrap();
//     row.get(1)
//         .unwrap()
//         .as_array()
//         .unwrap()
//         .iter()
//         .map(|x| {
//             let skill_id = x.get(0).unwrap().as_i64().unwrap();
//             let experience = x.get(1).unwrap().as_i64().unwrap();
//             experience_state::Model {
//                 entity_id: player_id,
//                 skill_id: skill_id as i32,
//                 experience: experience as i32,
//             }
//         })
//         .collect::<Vec<experience_state::Model>>()
// }

type PlayerLeaderboardTasks =
    Vec<tokio::task::JoinHandle<Result<(String, RankType), (StatusCode, &'static str)>>>;

pub(crate) async fn player_leaderboard(
    state: State<std::sync::Arc<AppState>>,
    Path(player_id): Path<i64>,
) -> Result<axum_codec::Codec<BTreeMap<String, RankType>>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let mut leaderboard_result: BTreeMap<String, RankType> = BTreeMap::new();

    let mut tasks: PlayerLeaderboardTasks = vec![];

    for skill in skills {
        if skill.skill_category == 2 || skill.skill_category == 0 {
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
                    level: experience_to_level(entry.experience as i64),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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

    for result in results.into_iter().flatten() {
        if let Ok((name, leaderboard)) = result {
            leaderboard_result.entry(name).or_insert(leaderboard);
        } else {
            error!("Error: {result:?}");
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

    Ok(axum_codec::Codec(leaderboard_result))
}

pub(crate) async fn get_claim_leaderboard(
    state: State<std::sync::Arc<AppState>>,
    Path(claim_id): Path<i64>,
) -> Result<axum_codec::Codec<GetTop100Response>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    let claim_member = Query::find_claim_member_by_claim_id(&state.conn, claim_id)
        .await
        .map_err(|error| {
            error!("Error: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

    if claim_member.is_empty() {
        return Err((StatusCode::NOT_FOUND, ""));
    }

    let player_ids = claim_member
        .iter()
        .map(|member| member.player_entity_id)
        .collect::<Vec<i64>>();

    let mut leaderboard_result: BTreeMap<String, Vec<RankType>> = BTreeMap::new();

    let generated_level_sql = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: LeaderboardRankTypeTasks = vec![];

    for skill in skills {
        if skill.skill_category == 2 || skill.skill_category == 0 {
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
                    level: experience_to_level(entry.experience as i64),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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
            Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
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

    for result in results.into_iter().flatten() {
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
                .or_default()
                .append(&mut leaderboard);
        } else {
            error!("Error: {result:?}");
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

    Ok(axum_codec::Codec(GetTop100Response {
        player_map: players,
        leaderboard: leaderboard_result,
    }))
}

// pub(crate) async fn handle_initial_subscription(
//     p0: &DatabaseConnection,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::columns([
//         experience_state::Column::EntityId,
//         experience_state::Column::SkillId,
//     ])
//     .update_columns([experience_state::Column::Experience])
//     .to_owned();
//
//     let chunk_size = 500;
//     let mut buffer_before_insert: Vec<PackedExperienceState> = Vec::with_capacity(chunk_size);
//     let mut experience_state_to_delete = get_known_experience_states(p0).await?;
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<PackedExperienceState>(row.as_ref()) {
//                 Ok(experience_state) => {
//                     let resolved_buffer_before_insert = experience_state
//                         .experience_stacks
//                         .iter()
//                         .map(|y| experience_state::Model {
//                             entity_id: experience_state.entity_id,
//                             skill_id: y.0,
//                             experience: y.1,
//                         })
//                         .collect::<Vec<experience_state::Model>>();
//
//                     for exp_state in resolved_buffer_before_insert.iter() {
//                         if experience_state_to_delete
//                             .contains(&(exp_state.entity_id, exp_state.skill_id))
//                         {
//                             experience_state_to_delete
//                                 .remove(&(exp_state.entity_id, exp_state.skill_id));
//                         }
//                     }
//
//                     buffer_before_insert.push(experience_state);
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_experience_state(p0, &mut buffer_before_insert, &on_conflict)
//                             .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!("Error: {error} for row: {:?}", row);
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         db_insert_experience_state(p0, &mut buffer_before_insert, &on_conflict).await?;
//         info!("experience_state last batch imported");
//     }
//
//     if !experience_state_to_delete.is_empty() {
//         delete_experience_state(p0, &mut experience_state_to_delete).await?;
//     }
//
//     Ok(())
// }

// pub(crate) async fn handle_transaction_update(
//     p0: &DatabaseConnection,
//     tables: &[TableWithOriginalEventTransactionUpdate],
//     skill_id_to_skill_name: &HashMap<i64, String>,
//     tx: UnboundedSender<WebSocketMessages>,
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::columns([
//         experience_state::Column::EntityId,
//         experience_state::Column::SkillId,
//     ])
//     .update_columns([experience_state::Column::Experience])
//     .to_owned();
//
//     let mut user_ids = HashSet::new();
//     let mut skills_to_update_vec = vec![];
//
//     let mut potential_deletes = HashSet::new();
//
//     for p1 in tables.iter() {
//         let event_type = if !p1.inserts.is_empty() && !p1.deletes.is_empty() {
//             "update"
//         } else if !p1.inserts.is_empty() && p1.deletes.is_empty() {
//             "insert"
//         } else if !p1.deletes.is_empty() && p1.inserts.is_empty() {
//             "delete"
//         } else {
//             "unknown"
//         };
//
//         if event_type == "unknown" {
//             error!("Unknown event type {:?}", p1);
//             continue;
//         }
//
//         if event_type == "delete" {
//             for row in p1.deletes.iter() {
//                 match serde_json::from_str::<PackedExperienceState>(row.as_ref()) {
//                     Ok(experience_state) => {
//                         experience_state
//                             .experience_stacks
//                             .iter()
//                             .map(|y| experience_state::Model {
//                                 entity_id: experience_state.entity_id,
//                                 skill_id: y.0,
//                                 experience: y.1,
//                             })
//                             .for_each(|x| {
//                                 potential_deletes.insert((x.entity_id, x.skill_id));
//                             });
//                     }
//                     Err(error) => {
//                         error!("Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         } else if event_type == "update" {
//             let mut delete_parsed = HashMap::new();
//             for row in p1.deletes.iter() {
//                 let parsed = serde_json::from_str::<PackedExperienceState>(row.as_ref());
//
//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse delete experience_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row
//                     );
//                 } else {
//                     let parsed = parsed.unwrap();
//                     delete_parsed.insert(parsed.entity_id, parsed.clone());
//                     parsed.experience_stacks.iter().for_each(|x| {
//                         potential_deletes.insert((parsed.entity_id, x.0));
//                     });
//                 }
//             }
//
//             for row in p1.inserts.iter() {
//                 let parsed = serde_json::from_str::<PackedExperienceState>(row.as_ref());
//
//                 if parsed.is_err() {
//                     error!(
//                         "Could not parse insert experience_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row
//                     );
//                     continue;
//                 }
//
//                 let parsed = parsed.unwrap();
//                 let id = parsed.entity_id;
//
//                 match (parsed, delete_parsed.get(&id)) {
//                     (new_experience_state, Some(old_experience_state)) => {
//                         let new_experience_state_map = new_experience_state
//                             .experience_stacks
//                             .iter()
//                             .map(|y| {
//                                 (
//                                     (new_experience_state.entity_id, y.0),
//                                     experience_state::Model {
//                                         entity_id: new_experience_state.entity_id,
//                                         skill_id: y.0,
//                                         experience: y.1,
//                                     },
//                                 )
//                             })
//                             .collect::<HashMap<(i64, i32), experience_state::Model>>();
//
//                         let old_experience_state_map = old_experience_state
//                             .experience_stacks
//                             .iter()
//                             .map(|y| {
//                                 (
//                                     (old_experience_state.entity_id, y.0),
//                                     experience_state::Model {
//                                         entity_id: old_experience_state.entity_id,
//                                         skill_id: y.0,
//                                         experience: y.1,
//                                     },
//                                 )
//                             })
//                             .collect::<HashMap<(i64, i32), experience_state::Model>>();
//
//                         for (key, value) in new_experience_state_map.iter() {
//                             let skill_id = key.1 as i64;
//                             let skill_name = skill_id_to_skill_name.get(&skill_id);
//
//                             if old_experience_state_map.contains_key(key) {
//                                 let old_value = old_experience_state_map.get(key).unwrap();
//
//                                 if old_value != value {
//                                     user_ids.insert(new_experience_state.entity_id);
//                                     skills_to_update_vec.push(value.clone().into_active_model());
//
//                                     if let Some(skill_name) = skill_name {
//                                         tx.send(WebSocketMessages::Experience {
//                                             experience: value.experience as u64,
//                                             level: experience_to_level(value.experience as i64)
//                                                 as u64,
//                                             rank: 0,
//                                             skill_name: skill_name.clone().to_owned(),
//                                             user_id: new_experience_state.entity_id,
//                                         })
//                                         .expect("TODO: panic message");
//
//                                         let old_level =
//                                             experience_to_level(old_value.experience as i64);
//                                         let new_level =
//                                             experience_to_level(value.experience as i64);
//
//                                         if old_level != new_level {
//                                             tx.send(WebSocketMessages::Level {
//                                                 level: experience_to_level(value.experience as i64)
//                                                     as u64,
//                                                 skill_name: skill_name.clone().to_owned(),
//                                                 user_id: new_experience_state.entity_id,
//                                             })
//                                             .expect("TODO: panic message");
//                                         }
//
//                                         metrics::counter!(
//                                             "player_skill_experience_count",
//                                             &[("skill_name", skill_name.to_owned()),]
//                                         )
//                                         .increment(
//                                             (value.experience - old_value.experience) as u64,
//                                         );
//                                     }
//                                 }
//                             } else {
//                                 user_ids.insert(new_experience_state.entity_id);
//                                 skills_to_update_vec.push(value.clone().into_active_model());
//
//                                 if let Some(skill_name) = skill_name {
//                                     metrics::counter!(
//                                         "player_skill_experience_count",
//                                         &[("skill_name", skill_name.to_owned()),]
//                                     )
//                                     .increment(value.experience as u64);
//                                 }
//                             }
//
//                             potential_deletes.remove(key);
//                         }
//                     }
//                     (_new_experience_state, None) => {
//                         error!("Could not find delete state new experience state",);
//                     }
//                 }
//             }
//         } else if event_type == "insert" {
//             for row in p1.inserts.iter() {
//                 match serde_json::from_str::<PackedExperienceState>(row.as_ref()) {
//                     Ok(experience_state) => {
//                         experience_state
//                             .experience_stacks
//                             .iter()
//                             .map(|y| experience_state::Model {
//                                 entity_id: experience_state.entity_id,
//                                 skill_id: y.0,
//                                 experience: y.1,
//                             })
//                             .for_each(|x| {
//                                 let skill_id = x.skill_id as i64;
//                                 let skill_name = skill_id_to_skill_name.get(&skill_id);
//
//                                 let key = (x.entity_id, x.skill_id);
//                                 potential_deletes.remove(&key);
//                                 user_ids.insert(x.entity_id);
//                                 skills_to_update_vec.push(x.clone().into_active_model());
//
//                                 if let Some(skill_name) = skill_name {
//                                     tx.send(WebSocketMessages::Experience {
//                                         experience: x.experience as u64,
//                                         level: experience_to_level(x.experience as i64) as u64,
//                                         rank: 0,
//                                         skill_name: skill_name.clone().to_owned(),
//                                         user_id: x.entity_id,
//                                     })
//                                     .expect("TODO: panic message");
//
//                                     metrics::counter!(
//                                         "player_skill_experience_count",
//                                         &[("skill_name", skill_name.to_owned()),]
//                                     )
//                                     .increment(x.experience as u64);
//                                 }
//                             });
//                     }
//                     Err(error) => {
//                         error!("Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         }
//     }
//
//     for skill_entries_to_insert_chunk in skills_to_update_vec.chunks(5000) {
//         let result = experience_state::Entity::insert_many(skill_entries_to_insert_chunk.to_vec())
//             .on_conflict(on_conflict.clone())
//             .exec(p0)
//             .await;
//
//         if let Err(error) = result {
//             error!(
//                 "Error: {error} for chunk: {:?}",
//                 skill_entries_to_insert_chunk
//             );
//             return Err(error.into());
//         }
//     }
//
//     let total_experience =
//         Query::get_experience_state_player_ids_total_experience_with_experience_per_hour(
//             p0,
//             user_ids.clone().into_iter().collect(),
//             Some(EXCLUDED_SKILLS_FROM_GLOBAL_LEADERBOARD_SKILLS_CATEGORY),
//         )
//         .await?;
//
//     for experience in total_experience {
//         tx.send(WebSocketMessages::TotalExperience {
//             experience: experience.1 as u64,
//             user_id: experience.0,
//             experience_per_hour: experience.2 as u64,
//         })
//         .expect("Could not send total experience to websocket");
//     }
//
//     if !potential_deletes.is_empty() {
//         delete_experience_state(p0, &mut potential_deletes).await?;
//     }
//
//     Ok(())
// }
