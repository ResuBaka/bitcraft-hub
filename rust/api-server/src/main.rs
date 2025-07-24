// Note that the standalone server is invoked through standaline/src/main.rs, so you will
// also want to set the allocator there.
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;


#[tokio::main]
async fn main() {
    api::main().await.expect("Failed to start server");
}

// use std::collections::HashMap;
// use std::fs::{File, read_to_string};
// use std::io::BufReader;
// use std::io::SeekFrom::End;
// use std::ops::{AddAssign, Deref};
// use std::time::{Duration, SystemTime};
// use sea_orm::ActiveValue::Set;
// use sea_orm::{DatabaseBackend, DbBackend, DeriveEntityModel, Schema, Statement, TransactionTrait};
// use sea_orm::entity::prelude::*;
// use sea_orm::sea_query::OnConflict;
// use serde::{Deserialize, Serialize};
// use indicatif::{ProgressIterator, ProgressStyle};
// use meilisearch_sdk::settings::Settings;
// use lazy_static::lazy_static;
// use meilisearch_sdk::client::Client;
// use sysinfo::ProcessRefreshKind;
// use tokio::task;
// use prometheus::{self, IntCounter, TextEncoder, Encoder, register_int_counter_vec, IntCounterVec};
// use base64::prelude::*;
// use reqwest::header::{HeaderMap, HeaderValue};
//
// lazy_static! {
//     static ref BITCRAFT_SKILL_EXPERIENCE: IntCounterVec =
//         register_int_counter_vec!("bitcraft_skill_experience", "Player skill experience", &["skill"]).unwrap();
// }
//
// struct BitcraftApi {
// }
//
// impl BitcraftApi {
//     async fn get_experience_state(client: &reqwest::Client, skill_desc: &Vec<SkillDesc>) -> Result<Vec<ExperienceState>, reqwest::Error> {
//         let ExperienceStateResponse = client.post("https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2")
//             .body(r#"SELECT * FROM ExperienceState;"#)
//             .send().await.unwrap();
//
//         Ok(ExperienceStateResponse.json::<serde_json::Value>().await.unwrap().get(0).unwrap().get("rows").unwrap().as_array().unwrap().iter().map(|x| ExperienceState::from_json(x, skill_desc)).collect::<Vec<_>>())
//     }
// }
//
// fn calc_exp_by_skills(ex_state: &Vec<ExperienceState>) -> HashMap<String, i64> {
//     let mut exp: HashMap<String, i64> = HashMap::new();
//     for exp_row in ex_state.iter() {
//         for (skill_name, skill_exp) in exp_row.skills_experience.iter() {
//             if exp.contains_key(skill_name) {
//                 exp.get_mut(skill_name).unwrap().add_assign(skill_exp);
//             } else {
//                 exp.insert(skill_name.clone(), *skill_exp);
//             }
//         }
//     }
//
//     exp
// }
//
// #[tokio::main]
// async fn main() {
//     let mut headers = HeaderMap::new();
//
//     headers.insert("Authorization", HeaderValue::from_str(&format!("Basic {}",
//         BASE64_STANDARD.encode(format!("token:{}",
//             std::env::var("BITCRAFT_API_KEY").unwrap()
//         ))
//     )).unwrap());
//
//     println!("Headers: {:?}", headers);
//
//     let client = reqwest::Client::builder().default_headers(headers).build().unwrap();
//
//     let schema = client.post("https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2")
//         .body(r#"SELECT * FROM EmpireState;"#)
//         .send().await.unwrap();
//
//     println!("Schema: {}", serde_json::to_string_pretty(&schema.json::<serde_json::Value>().await.unwrap()).unwrap());
//
//     let SkillDescResponse = client.post("https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2")
//         .body(r#"SELECT * FROM SkillDesc;"#)
//         .send().await.unwrap();
//
//     let SkillDescResponse = SkillDescResponse.json::<serde_json::Value>().await.unwrap().get(0).unwrap().get("rows").unwrap().as_array().unwrap().iter().map(|x| SkillDesc::from_json(x)).collect::<Vec<_>>();
//
//
//     let ExperienceStateResponse = BitcraftApi::get_experience_state(&client, &SkillDescResponse).await.unwrap();
//     // println!("ExperienceStateResponse:");
//     // ExperienceStateResponse.iter().for_each(
//     //     |x| println!("{:?}", x)
//     // );
//
//     let PlayerStateResponse = client.post("https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2")
//         .body(r#"SELECT * FROM PlayerState;"#)
//     .send().await.unwrap();
//
//     // println!("PlayerStateResponse: {:?}", PlayerStateResponse.json::<serde_json::Value>().await.unwrap().get(0).unwrap().get("rows").unwrap());
//
//
//     dbg!(calc_exp_by_skills(&ExperienceStateResponse));
//
//     // BITCRAFT_SKILL_EXPERIENCE.inc_by(1, Some("finishing".to_string())).unwrap();
//
//     return;
//     //
//     // // let db = sea_orm::Database::connect("sqlite:./sqlite_test.db").await.unwrap();
//     // // let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
//     // let db = sea_orm::Database::connect("mysql://root:root@localhost:3306/test").await.unwrap();
//     //
//     // let builder = db.get_database_backend();
//     // let schema = Schema::new(builder);
//     // if let Result::Err(err) = db.execute(builder.build(&schema.create_table_from_entity(Entity))).await {
//     //     println!("{}", err);
//     // }
//     //
//     // println!("Connected to database");
//     // let mut conn = db.clone();
//     //
//     // // db.run(sql`PRAGMA journal_mode = OFF;`)
//     // // db.run(sql`PRAGMA synchronous = 0;`)
//     // // db.run(sql`PRAGMA cache_size = 1000000;`)
//     // // db.run(sql`PRAGMA locking_mode = EXCLUSIVE;`)
//     // // db.run(sql`PRAGMA temp_store = MEMORY;`)
//     //
//     // if db.get_database_backend() == DatabaseBackend::Sqlite {
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA journal_mode = OFF;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA synchronous = OFF;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA cache_size = 10000000;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA locking_mode = EXCLUSIVE;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA temp_store = MEMORY;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA count_changes = OFF;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     //     conn.execute(Statement::from_string(
//     //         DatabaseBackend::Sqlite,
//     //         "PRAGMA auto_vacuum = NONE;".to_string(),
//     //     ))
//     //         .await
//     //         .unwrap();
//     // }
//     //
//     //
//     // let start = SystemTime::now();
//     //
//     // // build_index().await;
//     //
//     // write_memory_usage("Before load");
//     //
//     // let now = SystemTime::now();
//     // println!("Indexed data to Meilisearch {:?}", now.duration_since(start).unwrap());
//     //
//     // let start = SystemTime::now();
//     // println!("Start {:?}", start);
//     //
//     //
//     // let mut models = {
//     //     let state = File::open("../../storage/State/LocationState.json").unwrap();
//     //     let reader = BufReader::new(state);
//     //     println!("Loaded state");
//     //     let now = SystemTime::now();
//     //     println!("Read file {:?}", now.duration_since(start).unwrap());
//     //     write_memory_usage("Loaded data");
//     //
//     //     let lcoationState: Vec<LocationJson> = serde_json::from_reader(reader).unwrap();
//     //     write_memory_usage("Parsed data");
//     //     println!("Parsed state");
//     //     let now = SystemTime::now();
//     //     println!("Parsed state {:?}", now.duration_since(start).unwrap());
//     //
//     //     lcoationState[0].rows.iter().map(|row| {
//     //         let entity_id = row[0];
//     //         let chunk_index = row[1];
//     //         let x = row[2];
//     //         let z = row[3];
//     //         let dimension = row[4];
//     //
//     //         ActiveModel {
//     //             entity_id: Set(entity_id as u64),
//     //             chunk_index: Set(chunk_index as u64),
//     //             x: Set(x.try_into().unwrap()),
//     //             z: Set(z.try_into().unwrap()),
//     //             dimension: Set(dimension.try_into().unwrap()),
//     //         }
//     //
//     //         // Model {
//     //         //     entity_id: entity_id as u64,
//     //         //     chunk_index: chunk_index as u64,
//     //         //     x: x.try_into().unwrap(),
//     //         //     z: z.try_into().unwrap(),
//     //         //     dimension: dimension.try_into().unwrap(),
//     //         // }
//     //     }).collect::<Vec<_>>()
//     // };
//     // write_memory_usage("Generated models");
//     //
//     // // models.sort_by(|a, b| a.entity_id.clone().unwrap().cmp(&b.entity_id.clone().unwrap()));
//     // models.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
//     // write_memory_usage("Sorted models");
//     // let now = SystemTime::now();
//     // println!("Created Models {:?}", now.duration_since(start).unwrap());
//     //
//     //
//     // println!("Starting to insert: {}", models.len());
//     //
//     //
//     // println!("Started transaction");
//     //
//     //
//     // for rows in models.chunks(10000).progress() {
//     //
//     //     // let start_insert = SystemTime::now();
//     //     Entity::insert_many(rows.clone().to_owned()).exec(&db).await.unwrap();
//     //     // let now = SystemTime::now();
//     //     // println!("Inserted {:?}", now.duration_since(start_insert).unwrap());
//     // }
//     //
//     //
//     // let now = SystemTime::now();
//     // println!("Finished {:?}", now.duration_since(start).unwrap());
//     //
//     //
//     // println!("Hello, world!");
// }
//
// #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
// #[sea_orm(table_name = "location")]
// pub struct Model {
//     #[sea_orm(primary_key, auto_increment = false)]
//     pub entity_id: u64,
//     pub chunk_index: u64,
//     pub x: i32,
//     pub z: i32,
//     pub dimension: i32,
// }
//
// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
// pub enum Relation {}
//
// impl ActiveModelBehavior for ActiveModel {}
//
// #[derive(Serialize, Deserialize)]
// pub struct LocationJson {
//     rows: Vec<Vec<i64>>,
// }
//
// #[derive(Debug)]
// struct ExperienceState {
//     pub player_id: i64,
//     pub skills_experience: Vec<(String, i64)>,
// }
//
// impl ExperienceState {
//     fn from_json(json: &serde_json::Value, skill_desc: &Vec<SkillDesc>) -> ExperienceState {
//
//         ExperienceState {
//             player_id: json.get(0).unwrap().as_i64().unwrap(),
//             skills_experience: json.get(1).unwrap().as_array().unwrap().iter().map(|x| (skill_desc.iter().find(|y| y.id == x.get(0).unwrap().as_i64().unwrap()).unwrap().name.clone(), x.get(1).unwrap().as_i64().unwrap())).collect(),
//         }
//     }
// }
//
// #[derive(Debug)]
// struct SkillDesc {
//     pub id: i64,
//     pub name: String,
// }
//
// impl SkillDesc {
//     fn from_json(json: &serde_json::Value) -> SkillDesc {
//         SkillDesc {
//             id: json.get(0).unwrap().as_i64().unwrap(),
//             name: json.get(1).unwrap().as_str().unwrap().to_string(),
//         }
//     }
// }
//
// fn write_memory_usage(prefix: &str) {
//     let pid = sysinfo::Pid::from_u32(std::process::id());
//     let mut system = sysinfo::System::new_with_specifics(
//         sysinfo::RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
//     );
//     system.refresh_all();
//
//     println!(
//         "{prefix} current RAM usage: {} MB",
//         system.process(pid).unwrap().memory() / 1024 / 1024
//     );
// }
