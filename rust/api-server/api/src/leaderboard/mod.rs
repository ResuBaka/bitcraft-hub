use std::collections::HashMap;
use std::fs::File;
use std::ops::AddAssign;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use futures::future::join_all;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use entity::experience_state;
use service::Query;
use crate::AppState;
use sea_orm::{ActiveModelTrait, DeriveColumn, EnumIter, IntoActiveModel, PaginatorTrait};

#[macro_export]
macro_rules! generate_mysql_sum_level_sql_statement {
    ($experience_per_level:expr) => {
        {
            let mut sql = String::new();
            sql.push_str("SUM(CASE ");
            for (level, xp) in $experience_per_level.iter().rev() {
              sql.push_str(format!("WHEN experience > {xp} THEN {level} ").as_str());
            }
            sql.push_str("ELSE 0 END)");
            sql
        }
    };
}

pub(crate) const EXPERIENCE_PER_LEVEL: [(i32, i64); 100] = [
    (1, 0),
    (2, 640),
    (3, 1330),
    (4, 2090),
    (5, 2920),
    (6, 3830),
    (7, 4820),
    (8, 5890),
    (9, 7070),
    (10, 8350),
    (11, 9740),
    (12, 11260),
    (13, 12920),
    (14, 14730),
    (15, 16710),
    (16, 18860),
    (17, 21210),
    (18, 23770),
    (19, 26560),
    (20, 29600),
    (21, 32920),
    (22, 36550),
    (23, 40490),
    (24, 44800),
    (25, 49490),
    (26, 54610),
    (27, 60200),
    (28, 66290),
    (29, 72930),
    (30, 80170),
    (31, 88060),
    (32, 96670),
    (33, 106060),
    (34, 116300),
    (35, 127470),
    (36, 139650),
    (37, 152930),
    (38, 167410),
    (39, 183200),
    (40, 200420),
    (41, 219200),
    (42, 239680),
    (43, 262020),
    (44, 286370),
    (45, 312930),
    (46, 341890),
    (47, 373480),
    (48, 407920),
    (49, 445480),
    (50, 486440),
    (51, 531110),
    (52, 579820),
    (53, 632940),
    (54, 690860),
    (55, 754030),
    (56, 822920),
    (57, 898040),
    (58, 979960),
    (59, 1069290),
    (60, 1166710),
    (61, 1272950),
    (62, 1388800),
    (63, 1515140),
    (64, 1652910),
    (65, 1803160),
    (66, 1967000),
    (67, 2145660),
    (68, 2340500),
    (69, 2552980),
    (70, 2784680),
    (71, 3037360),
    (72, 3312900),
    (73, 3613390),
    (74, 3941070),
    (75, 4298410),
    (76, 4688090),
    (77, 5113030),
    (78, 5576440),
    (79, 6081800),
    (80, 6632890),
    (81, 7233850),
    (82, 7889210),
    (83, 8603890),
    (84, 9383250),
    (85, 10233150),
    (86, 11159970),
    (87, 12170670),
    (88, 13272850),
    (89, 14474790),
    (90, 15785510),
    (91, 17214860),
    (92, 18773580),
    (93, 20473370),
    (94, 22327010),
    (95, 24348420),
    (96, 26552780),
    (97, 28956650),
    (98, 31578090),
    (99, 34436800),
    (100, 7554230),
];

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum RankType {
    Experience(LeaderboardExperience),
    Level(LeaderboardLevel),
    Skill(LeaderboardSkill),
    Time(LeaderboardTime),
}

#[derive(Debug, Serialize, Deserialize)]
struct LeaderboardSkill {
    player_id: u64,
    player_name: Option<String>,
    experience: i32,
    level: i32,
    rank: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LeaderboardLevel {
    player_id: u64,
    player_name: Option<String>,
    level: u32,
    rank: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LeaderboardExperience {
    player_id: u64,
    player_name: Option<String>,
    experience: i32,
    rank: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LeaderboardTime {
    player_id: u64,
    player_name: Option<String>,
    time_played: u64,
    rank: u64,
}

pub(crate) async fn get_top_100(
    state: State<AppState>,
) -> Result<Json<HashMap<String, Vec<RankType>>>, (StatusCode, &'static str)> {
    let skills = Query::skill_descriptions(&state.conn).await.map_err(|error| {
        println!("Error: {error}");

        (StatusCode::INTERNAL_SERVER_ERROR, "")
    })?;

    let mut leaderboard_result: HashMap<String, Vec<RankType>> = HashMap::new();

    let a = generate_mysql_sum_level_sql_statement!(EXPERIENCE_PER_LEVEL);

    let mut tasks: Vec<tokio::task::JoinHandle<Result<(String, Vec<RankType>), (StatusCode, &'static str)>>> = vec![];

    for skill in skills {
        if skill.name == "ANY" {
            continue;
        }

        let db = state.conn.clone();
        tasks.push(tokio::spawn(async move {
            let mut leaderboard: Vec<RankType> = Vec::new();
            let entries = Query::get_experience_state_top_100_by_skill_id(&db, skill.id).await.map_err(|error| {
                println!("Error: {error}");

                (StatusCode::INTERNAL_SERVER_ERROR, "")
            })?;

            for (i, entry) in entries.into_iter().enumerate() {
                let rank = i + 1;
                let mut player_name = None;

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
        let entries = Query::get_experience_state_top_100_total_experience(&db).await.map_err(|error| {
            println!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Experience(LeaderboardExperience {
                player_id: entry.entity_id,
                player_name: None,
                experience: entry.experience,
                rank: rank as u64,
            }));
        }

        Ok(("Experience".to_string(), leaderboard))
    }));


    let db = state.conn.clone();
    tasks.push(tokio::spawn(async move {
        let mut leaderboard: Vec<RankType> = Vec::new();
        let entries = Query::get_experience_state_top_100_total_level(&db, a).await.map_err(|error| {
            println!("Error: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "")
        })?;

        for (i, entry) in entries.into_iter().enumerate() {
            let rank = i + 1;
            leaderboard.push(RankType::Level(LeaderboardLevel {
                player_id: entry.0 as u64,
                player_name: None,
                level: entry.1 as u32,
                rank: rank as u64,
            }));
        }

        Ok(("Level".to_string(), leaderboard))
    }));

    let results = futures::future::join_all(tasks).await;
    let mut player_ids: Vec<u64> = vec![];

    for results_inner in results {
        for result in results_inner {
            if let Ok((name, mut leaderboard)) = result {
                player_ids.append(&mut leaderboard.iter().map(|x| {
                    match x {
                        RankType::Skill(x) => x.player_id,
                        RankType::Level(x) => x.player_id,
                        RankType::Experience(x) => x.player_id,
                        RankType::Time(x) => x.player_id,
                    }
                }).collect::<Vec<u64>>());

                leaderboard_result.entry(name).or_insert(Vec::new()).append(&mut leaderboard);
            } else {
                println!("Error: {result:?}");
            }
        }

    }

    player_ids.sort();
    player_ids.dedup();

    let players_name_by_id = Query::find_plyer_by_ids(&state.conn, player_ids).await.map_err(|error| {
        println!("Error: {error}");

        (StatusCode::INTERNAL_SERVER_ERROR, "")
    })?.into_iter().map(|x| (x.entity_id, x.username)).collect::<HashMap<u64, String>>();

    for (_, top) in leaderboard_result.iter_mut() {
        for x in top.iter_mut() {
            match x {
                RankType::Skill(x) => {
                    x.player_name = players_name_by_id.get(&x.player_id).or(None).map(|x| x.to_string());
                },
                RankType::Level(x) => {
                    x.player_name = players_name_by_id.get(&x.player_id).or(None).map(|x| x.to_string());
                },
                RankType::Experience(x) => {
                    x.player_name = players_name_by_id.get(&x.player_id).or(None).map(|x| x.to_string());
                },
                RankType::Time(x) => {
                    x.player_name = players_name_by_id.get(&x.player_id).or(None).map(|x| x.to_string());
                },
            };
        }
    }

    Ok(Json(leaderboard_result))
}

fn experience_to_level(experience: i64) -> i32 {
    if experience == 0 {
        return 1;
    }

    for (level, xp) in EXPERIENCE_PER_LEVEL.iter().rev() {
        if experience.gt(xp) {
            return *level;
        }
    }

    100 as i32
}

pub(crate) async fn import_experience_state(
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/State/ExperienceState.json").unwrap();
    let experience_state: Value = serde_json::from_reader(&item_file).unwrap();
    let experience_states: Vec<experience_state::ActiveModel> = serde_json::from_value::<Vec<serde_json::Value>>(experience_state.get(0).unwrap().get("rows").unwrap().clone()).unwrap().into_iter().map(|x| {
        let player_id = x.get(0).unwrap().as_u64().unwrap();
        x.get(1).unwrap().as_array().unwrap().iter().map(|x| {
            let skill_id = x.get(0).unwrap().as_u64().unwrap();
            let experience = x.get(1).unwrap().as_i64().unwrap();
            experience_state::Model { entity_id: player_id, skill_id: skill_id as i32, experience: experience as i32 }.into_active_model()
        }).collect::<Vec<experience_state::ActiveModel>>()
    }).flatten().collect();
    let count = experience_states.len();
    let db_count = experience_state::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        println!("ExperienceState already imported");
        return Ok(());
    }

    for experience_state in experience_states.chunks(5000) {
        let _ = experience_state::Entity::insert_many(experience_state.to_vec()).exec(conn).await;
    }

    Ok(())
}

