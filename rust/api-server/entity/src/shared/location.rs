use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{MapAccess, SeqAccess, Visitor};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, sea_orm::FromJsonQueryResult, Default)]
pub struct Location {
    x: i32,
    z: i32,
    dimension: u32,
}
