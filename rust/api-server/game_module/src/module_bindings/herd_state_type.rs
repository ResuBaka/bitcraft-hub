// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct HerdState {
    pub entity_id: u64,
    pub enemy_ai_params_desc_id: i32,
    pub current_population: i32,
    pub ignore_eagerness: bool,
    pub population_variance: Vec<f32>,
}

impl __sdk::InModule for HerdState {
    type Module = super::RemoteModule;
}
