//! Read models from BitCraft's authoritative global database.
pub mod views;

use game_global_module::module_bindings as wire;
use sea_orm::prelude::DateTimeWithTimeZone;

fn timestamp(value: spacetimedb_sdk::Timestamp) -> DateTimeWithTimeZone {
    chrono::DateTime::from_timestamp_micros(value.to_micros_since_unix_epoch())
        .expect("invalid empire timestamp")
        .into()
}

macro_rules! table {
    ($module:ident, $name:literal, $wire:ident |$r:ident| {
        $pk:ident: $key:ty, $($field:ident: $ty:ty = $value:expr),+ $(,)?
    }) => {
        pub mod $module {
            use super::*;
            use sea_orm::entity::prelude::*;
            #[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
            #[sea_orm(table_name = $name)]
            pub struct Model {
                #[sea_orm(primary_key, auto_increment = false)]
                pub $pk: $key,
                $(pub $field: $ty),+
            }
            #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
            pub enum Relation {}
            impl ActiveModelBehavior for ActiveModel {}

            impl From<&wire::$wire> for Model {
                fn from($r: &wire::$wire) -> Self {
                    Self { $pk: $r.$pk as $key, $($field: $value),+ }
                }
            }
        }
    }
}

table!(parameters_desc, "empire_decay_parameters", ParametersDesc |r| {
    version: i32, empire_decay_tick_millis: i32 = r.empire_decay_tick_millis,
});
table!(empire_state, "empire_state", EmpireState |r| {
    entity_id: i64,
    capital_building_entity_id: i64 = r.capital_building_entity_id as i64,
    name: String = r.name.clone(),
    shard_treasury: i64 = r.shard_treasury.into(),
    nobility_threshold: i32 = r.nobility_threshold,
    num_claims: i32 = r.num_claims,
    x: i32 = r.location.x, z: i32 = r.location.z, dimension: i64 = r.location.dimension.into(),
    empire_currency_treasury: i64 = r.empire_currency_treasury.into(),
    owner_type: i16 = r.owner_type as i16,
});
table!(empire_player_data_state, "empire_player_data_state", EmpirePlayerDataState |r| {
    entity_id: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    rank: i16 = r.rank.into(), donated_shards: i64 = r.donated_shards.into(),
    noble: Option<DateTimeWithTimeZone> = r.noble.map(timestamp),
    donated_empire_currency: i64 = r.donated_empire_currency.into(),
});
table!(empire_rank_state, "empire_rank_state", EmpireRankState |r| {
    entity_id: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    rank: i16 = r.rank.into(), title: String = r.title.clone(), permissions: Vec<bool> = r.permissions.clone(),
});
table!(empire_settlement_state, "empire_settlement_state", EmpireSettlementState |r| {
    building_entity_id: i64, claim_entity_id: i64 = r.claim_entity_id as i64,
    empire_entity_id: i64 = r.empire_entity_id as i64, chunk_index: i64 = r.chunk_index as i64,
    can_house_empire_storehouse: bool = r.can_house_empire_storehouse, members_donations: i64 = r.members_donations.into(),
    x: i32 = r.location.x, z: i32 = r.location.z, dimension: i64 = r.location.dimension.into(),
});
table!(empire_node_state, "empire_node_state", EmpireNodeState |r| {
    entity_id: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    chunk_index: i64 = r.chunk_index as i64, energy: i32 = r.energy, active: bool = r.active, upkeep: i32 = r.upkeep,
    x: i32 = r.location.x, z: i32 = r.location.z, dimension: i64 = r.location.dimension.into(),
});
table!(empire_chunk_state, "empire_chunk_state", EmpireChunkState |r| {
    chunk_index: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    watchtower_entity_id: i64 = r.watchtower_entity_id as i64,
});
table!(empire_node_siege_state, "empire_node_siege_state", EmpireNodeSiegeState |r| {
    entity_id: i64, building_entity_id: i64 = r.building_entity_id as i64,
    empire_entity_id: i64 = r.empire_entity_id as i64, energy: i32 = r.energy, active: bool = r.active,
    start_timestamp: Option<DateTimeWithTimeZone> = r.start_timestamp.map(timestamp),
});
table!(empire_emblem_state, "empire_emblem_state", EmpireEmblemState |r| {
    entity_id: i64, icon_id: i32 = r.icon_id, shape_id: i32 = r.shape_id,
    color_1_id: i32 = r.color_1_id, color_2_id: i32 = r.color_2_id,
});
table!(empire_directive_state, "empire_directive_state", EmpireDirectiveState |r| {
    entity_id: i64, directive_message: String = r.directive_message.clone(),
    directive_message_timestamp: Option<DateTimeWithTimeZone> = r.directive_message_timestamp.map(timestamp),
});
table!(empire_foundry_state, "empire_foundry_state", EmpireFoundryState |r| {
    entity_id: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    hexite_capsules: i32 = r.hexite_capsules, queued: i32 = r.queued, started: DateTimeWithTimeZone = timestamp(r.started),
});
table!(empire_siege_engine_state, "empire_siege_engine_state", EmpireSiegeEngineState |r| {
    entity_id: i64, building_entity_id: i64 = r.building_entity_id as i64,
});
table!(empire_notification_state, "empire_notification_state", EmpireNotificationState |r| {
    entity_id: i64, empire_entity_id: i64 = r.empire_entity_id as i64,
    notification_type: i16 = r.notification_type as i16, text_replacement: Vec<String> = r.text_replacement.clone(),
    timestamp: i32 = r.timestamp,
});
table!(empire_rank_desc, "empire_rank_desc", EmpireRankDesc |r| {
    rank: i32, title: String = r.title.clone(), max_count: Option<i32> = r.max_count,
    permissions: Vec<bool> = r.permissions.clone(),
});
table!(empire_color_desc, "empire_color_desc", EmpireColorDesc |r| {
    id: i32, color_argb: i64 = r.color_argb as i64, color_2_argb: Option<i64> = r.color_2_argb.map(|c| c as i64),
});
table!(empire_icon_desc, "empire_icon_desc", EmpireIconDesc |r| {
    id: i32, icon_unicode: String = r.icon_unicode.clone(), is_shape: bool = r.is_shape,
});
table!(empire_supplies_desc, "empire_supplies_desc", EmpireSuppliesDesc |r| {
    cargo_id: i32, energy: i32 = r.energy,
});
table!(empire_territory_desc, "empire_territory_desc", EmpireTerritoryDesc |r| {
    id: i16, chunks: i32 = r.chunks.into(), ranks: Vec<i16> = r.ranks.iter().copied().map(i16::from).collect(),
    crown_collectible_id: i32 = r.crown_collectible_id,
});
table!(empire_notification_desc, "empire_notification_desc", EmpireNotificationDesc |r| {
    id: i32, notification_type: i16 = r.notification_type as i16, priority: i32 = r.priority,
    show_on_login: bool = r.show_on_login, text: String = r.text.clone(),
});
