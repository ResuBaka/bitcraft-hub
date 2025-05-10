use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult)]
pub struct Timestamp {
    #[serde(with = "time::serde::timestamp::microseconds")]
    pub __timestamp_micros_since_unix_epoch__: OffsetDateTime,
}