// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::offset_coordinates_small_message_type::OffsetCoordinatesSmallMessage;
use super::teleport_location_type_type::TeleportLocationType;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct TeleportLocation {
    pub location: OffsetCoordinatesSmallMessage,
    pub location_type: TeleportLocationType,
}

impl __sdk::InModule for TeleportLocation {
    type Module = super::RemoteModule;
}
