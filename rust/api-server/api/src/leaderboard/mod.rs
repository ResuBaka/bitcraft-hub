pub(crate) mod bitcraft;

use crate::{AppRouter, AppState, leaderboard};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use log::error;
use serde::{Deserialize, Serialize};
use service::Query;
use std::collections::{BTreeMap, HashMap};
use ts_rs::TS;

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
    (3, 1_340),
    (4, 2_130),
    (5, 2_990),
    (6, 3_950),
    (7, 5_000),
    (8, 6_170),
    (9, 7_470),
    (10, 8_900),
    (11, 10_480),
    (12, 12_230),
    (13, 14_160),
    (14, 16_300),
    (15, 18_660),
    (16, 21_280),
    (17, 24_170),
    (18, 27_360),
    (19, 30_900),
    (20, 34_800),
    (21, 39_120),
    (22, 43_900),
    (23, 49_180),
    (24, 55_020),
    (25, 61_480),
    (26, 68_620),
    (27, 76_520),
    (28, 85_250),
    (29, 94_900),
    (30, 105_580),
    (31, 117_380),
    (32, 130_430),
    (33, 144_870),
    (34, 160_820),
    (35, 178_470),
    (36, 197_980),
    (37, 219_550),
    (38, 243_400),
    (39, 269_780),
    (40, 298_940),
    (41, 331_190),
    (42, 366_850),
    (43, 406_280),
    (44, 449_870),
    (45, 498_080),
    (46, 551_380),
    (47, 610_320),
    (48, 675_490),
    (49, 747_550),
    (50, 827_230),
    (51, 915_340),
    (52, 1_012_760),
    (53, 1_120_480),
    (54, 1_239_590),
    (55, 1_371_290),
    (56, 1_516_920),
    (57, 1_677_940),
    (58, 1_855_990),
    (59, 2_052_870),
    (60, 2_270_560),
    (61, 2_511_270),
    (62, 2_777_430),
    (63, 3_071_730),
    (64, 3_397_150),
    (65, 3_756_970),
    (66, 4_154_840),
    (67, 4_594_770),
    (68, 5_081_220),
    (69, 5_619_100),
    (70, 6_213_850),
    (71, 6_871_490),
    (72, 7_596_660),
    (73, 8_394_710),
    (74, 9_268_520),
    (75, 10_223_770),
    (76, 11_361_840),
    (77, 12_563_780),
    (78, 13_892_800),
    (79, 15_362_330),
    (80, 16_987_240),
    (81, 18_783_950),
    (82, 20_770_630),
    (83, 22_967_360),
    (84, 25_396_360),
    (85, 28_082_170),
    (86, 31_051_960),
    (87, 34_335_740),
    (88, 37_966_720),
    (89, 41_981_610),
    (90, 46_421_000),
    (91, 51_329_760),
    (92, 56_757_530),
    (93, 62_759_190),
    (94, 69_394_400),
    (95, 76_729_260),
    (96, 84_836_300),
    (97, 93_794_960),
    (98, 103_692_650),
    (99, 114_626_640),
    (100, 126_704_730),
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

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(untagged)]
pub(crate) enum RankType {
    Experience(LeaderboardExperience),
    ExperiencePerHour(LeaderboardExperiencePerHour),
    Level(LeaderboardLevel),
    Skill(LeaderboardSkill),
    Time(LeaderboardTime),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, TS)]
#[ts(export)]
pub(crate) struct LeaderboardSkill {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) level: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardLevel {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) level: u32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardExperiencePerHour {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardExperience {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) experience: i32,
    pub(crate) experience_per_hour: i32,
    pub(crate) rank: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
pub(crate) struct LeaderboardTime {
    pub(crate) player_id: i64,
    pub(crate) player_name: Option<String>,
    pub(crate) time_played: u64,
    pub(crate) rank: u64,
}

type LeaderboardRankTypeTasks =
    Vec<tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>>;

pub(crate) async fn get_top_100(
    state: State<AppState>,
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

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct PlayerLeaderboardResponse(BTreeMap<String, RankType>);

type PlayerLeaderboardTasks =
    Vec<tokio::task::JoinHandle<Result<(String, RankType), (StatusCode, &'static str)>>>;

pub(crate) async fn player_leaderboard(
    state: State<AppState>,
    Path(player_id): Path<i64>,
) -> Result<axum_codec::Codec<PlayerLeaderboardResponse>, (StatusCode, &'static str)> {
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

    Ok(axum_codec::Codec(PlayerLeaderboardResponse(
        leaderboard_result,
    )))
}

pub(crate) async fn get_claim_leaderboard(
    state: State<AppState>,
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
