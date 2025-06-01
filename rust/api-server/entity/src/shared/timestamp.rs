use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult)]
pub struct Timestamp {
    #[serde(with = "time::serde::timestamp::microseconds")]
    pub __timestamp_micros_since_unix_epoch__: OffsetDateTime,
}

impl From<spacetimedb_sdk::Timestamp> for Timestamp {
    fn from(value: spacetimedb_sdk::Timestamp) -> Self {
        Self {
            __timestamp_micros_since_unix_epoch__: OffsetDateTime::from_unix_timestamp_nanos(
                (value.to_micros_since_unix_epoch() * 1000) as i128,
            )
            .expect("invalid Unix timestamp"),
        }
    }
}
