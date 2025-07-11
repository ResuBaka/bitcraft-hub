// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::chat_message_state_type::ChatMessageState;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct PlayerReportState {
    pub entity_id: u64,
    pub reporter_entity_id: u64,
    pub reported_player_entity_id: u64,
    pub reported_player_username: String,
    pub report_type: String,
    pub report_message: String,
    pub reported_chat_message: Option<ChatMessageState>,
    pub chat_channel_context: Option<Vec<ChatMessageState>>,
    pub chat_user_context: Option<Vec<ChatMessageState>>,
    pub actioned: bool,
}

impl __sdk::InModule for PlayerReportState {
    type Module = super::RemoteModule;
}
