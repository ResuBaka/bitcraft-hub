// THIS FILE IS AUTOMATICALLY GENERATED BY SPACETIMEDB. EDITS TO THIS FILE
// WILL NOT BE SAVED. MODIFY TABLES IN YOUR MODULE SOURCE CODE INSTEAD.

// This was generated using spacetimedb cli version 1.2.0 (commit fb41e50eb73573b70eea532aeb6158eaac06fae0).

#![allow(unused, clippy::all)]
use spacetimedb_sdk::__codegen::{self as __sdk, __lib, __sats, __ws};

use super::world_gen_animation_curve_type::WorldGenAnimationCurve;
use super::world_gen_biomes_map_definition_type::WorldGenBiomesMapDefinition;
use super::world_gen_buildings_map_definition_type::WorldGenBuildingsMapDefinition;
use super::world_gen_mountains_map_definition_type::WorldGenMountainsMapDefinition;
use super::world_gen_resources_map_definition_type::WorldGenResourcesMapDefinition;
use super::world_gen_vector_2_int_type::WorldGenVector2Int;
use super::world_gen_world_map_definition_type::WorldGenWorldMapDefinition;

#[derive(__lib::ser::Serialize, __lib::de::Deserialize, Clone, PartialEq, Debug)]
#[sats(crate = __lib)]
pub struct WorldGenWorldDefinition {
    pub size: WorldGenVector2Int,
    pub land_curve: WorldGenAnimationCurve,
    pub noise_influence: f32,
    pub sea_level: i32,
    pub world_map: WorldGenWorldMapDefinition,
    pub biomes_map: WorldGenBiomesMapDefinition,
    pub mountains_map: WorldGenMountainsMapDefinition,
    pub buildings_map: WorldGenBuildingsMapDefinition,
    pub resources_map: WorldGenResourcesMapDefinition,
}

impl __sdk::InModule for WorldGenWorldDefinition {
    type Module = super::RemoteModule;
}
