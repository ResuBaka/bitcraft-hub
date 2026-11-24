//! Query projections for the empire directory, empire profiles, and watchtowers.
use chrono::{DateTime, Utc};
use sea_orm::{FromJsonQueryResult, FromQueryResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireSummary {
    pub id: String,
    pub name: String,
    pub npc: bool,
    pub color: Option<String>,
    pub accent: Option<String>,
    pub capital_id: Option<String>,
    pub capital_name: Option<String>,
    pub members: i32,
    pub settlements: i32,
    pub chunks: i32,
    pub towers: i32,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireMember {
    pub id: String,
    pub username: Option<String>,
    pub rank: i16,
    pub title: Option<String>,
    #[ts(type = "number")]
    pub donated_shards: i64,
    #[ts(type = "number")]
    pub donated_currency: i64,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireSettlement {
    pub id: String,
    pub name: Option<String>,
    pub region: Option<i16>,
    pub x: i32,
    pub z: i32,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireResources {
    #[ts(type = "number")]
    pub shard_treasury: i64,
    #[ts(type = "number")]
    pub currency_treasury: i64,
    pub directive: Option<String>,
    pub active_towers: i32,
    pub foundries: i32,
    pub active_sieges: i32,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireNews {
    pub id: String,
    pub timestamp: i32,
    pub template: Option<String>,
    pub replacements: Vec<String>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmpireDetail {
    pub empire: EmpireSummary,
    pub resources: EmpireResources,
    pub members: Vec<EmpireMember>,
    pub settlements: Vec<EmpireSettlement>,
    pub news: Vec<EmpireNews>,
    pub towers: Vec<Watchtower>,
    pub former_towers: Vec<FormerWatchtower>,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Watchtower {
    pub id: String,
    pub nickname: Option<String>,
    pub empire_id: String,
    pub empire_name: Option<String>,
    pub x: i32,
    pub z: i32,
    pub dimension: i64,
    pub region: Option<i16>,
    pub active: bool,
    pub energy: i32,
    pub upkeep: i32,
    pub decay_tick_millis: Option<i32>,
    pub assigned_chunks: i32,
    pub controlled_chunks: i32,
    pub under_siege: bool,
    pub marked: bool,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FormerWatchtower {
    pub id: String,
    pub nickname: Option<String>,
    pub x: i32,
    pub z: i32,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WatchtowerSiege {
    pub id: String,
    pub empire_id: String,
    pub empire_name: Option<String>,
    pub active: bool,
    pub energy: i32,
    pub start_timestamp: Option<DateTime<Utc>>,
}

// Raw history JSON includes 64-bit identifiers; expose those as decimal strings.
#[derive(FromJsonQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WatchtowerReading {
    pub empire_id: String,
    pub empire_name: Option<String>,
    pub active: bool,
    pub energy: i32,
    pub upkeep: Option<i32>,
    pub x: Option<i32>,
    pub z: Option<i32>,
    pub dimension: Option<i64>,
    pub start_timestamp: Option<DateTime<Utc>>,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WatchtowerObservation {
    pub id: String,
    pub observed_at: DateTime<Utc>,
    pub source: String,
    pub event: String,
    pub previous: Option<WatchtowerReading>,
    pub current: Option<WatchtowerReading>,
}

#[derive(FromQueryResult, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WatchtowerReport {
    pub id: String,
    pub timestamp: i32,
    pub notification_type: i16,
    pub empire_id: String,
    pub empire_name: Option<String>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WatchtowerDetail {
    pub tower: Option<Watchtower>,
    pub sieges: Vec<WatchtowerSiege>,
    pub history: Vec<WatchtowerObservation>,
    pub reports: Vec<WatchtowerReport>,
    pub next_before: Option<String>,
}
