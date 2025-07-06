use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, TS)]
pub struct Timestamp {
    #[ts(as = "String")]
    pub __timestamp_micros_since_unix_epoch__: DateTime<Utc>,
}

impl From<spacetimedb_sdk::Timestamp> for Timestamp {
    fn from(value: spacetimedb_sdk::Timestamp) -> Self {
        Self {
            __timestamp_micros_since_unix_epoch__: DateTime::from_timestamp_millis(
                value.to_micros_since_unix_epoch(),
            )
            .expect("invalid Unix timestamp"),
        }
    }
}
