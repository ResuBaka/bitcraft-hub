// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
#[derive(Copy, Eq, Hash)]
pub enum CollectibleType {
    Default,

    Hair,

    Mask,

    MaskPattern,

    HairColor,

    Nameplate,

    BodyColor,

    Emblem,

    ClothesHead,

    ClothesBelt,

    ClothesTorso,

    ClothesArms,

    ClothesLegs,

    ClothesFeet,

    Deployable,

    Title,

    Crown,

    Pet,

    ClothesCape,
}

impl __sdk::InModule for CollectibleType {
    type Module = super::RemoteModule;
}
