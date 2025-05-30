use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, Default,
)]
pub struct Location {
    x: i32,
    z: i32,
    dimension: u32,
}
