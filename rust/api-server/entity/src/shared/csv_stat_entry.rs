use game_module::module_bindings;
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::shared::character_stat::CharacterStatType;

#[derive(Clone, Debug, PartialEq, FromJsonQueryResult, Deserialize, Serialize, TS)]

pub struct CsvStatEntry {
    pub id: CharacterStatType,
    pub value: f32,
    pub is_pct: bool,
}

impl From<module_bindings::CsvStatEntry> for CsvStatEntry {
    fn from(value: module_bindings::CsvStatEntry) -> Self {
        CsvStatEntry {
            id: value.id.into(),
            value: value.value,
            is_pct: value.is_pct,
        }
    }
}
