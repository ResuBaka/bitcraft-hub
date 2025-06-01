use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, Default,
)]
pub struct Location {
    pub x: i32,
    pub z: i32,
    pub dimension: u32,
}
