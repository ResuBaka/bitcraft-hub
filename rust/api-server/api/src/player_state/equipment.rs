use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use dashmap::DashMap;
use entity::inventory::ItemType;
use entity::shared::{JsonRarity, Region};
use game_module::module_bindings::{
    CsvStatEntry, DbConnection, EquipmentDescTableAccess, EquipmentPresetState,
    EquipmentPresetStateTableAccess, EquipmentSlot, EquipmentSlotType, EquipmentState,
    EquipmentStateTableAccess, ErrorContext, ItemType as GameItemType, SubscriptionEventContext,
    ToolDesc, ToolDescTableAccess,
};
use serde::Serialize;
use spacetimedb_sdk::{DbContext, Event, Table, TableWithPrimaryKey};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Default)]
struct RegionEquipment {
    ready: bool,
    players: HashMap<u64, Vec<EquipmentSlot>>,
    presets_ready: bool,
    presets: HashMap<u64, HashMap<u64, EquipmentPresetState>>,
    item_stats: HashMap<i32, Vec<CsvStatEntry>>,
    tools: HashMap<i32, ToolDesc>,
}

impl RegionEquipment {
    fn save_preset(&mut self, row: EquipmentPresetState) {
        self.presets
            .entry(row.player_entity_id)
            .or_default()
            .insert(row.entity_id, row);
    }

    fn remove_preset(&mut self, row: &EquipmentPresetState) {
        if let std::collections::hash_map::Entry::Occupied(mut player) =
            self.presets.entry(row.player_entity_id)
        {
            player.get_mut().remove(&row.entity_id);
            if player.get().is_empty() {
                player.remove();
            }
        }
    }
}

struct EquipmentSnapshot {
    slots: Vec<EquipmentSlot>,
    presets: Option<Vec<EquipmentPresetState>>,
}

#[derive(Default)]
pub(crate) struct PlayerEquipment(DashMap<Region, RegionEquipment>);

impl PlayerEquipment {
    pub(crate) fn ready(&self, region: Region) {
        if let Some(mut equipment) = self.0.get_mut(&region) {
            equipment.ready = true;
        }
    }

    pub(crate) fn invalidate(&self, region: Region) {
        self.0.remove(&region);
    }

    fn snapshot(&self, region: Region, id: u64) -> Option<EquipmentSnapshot> {
        let region = self.0.get(&region)?;
        region.ready.then(|| EquipmentSnapshot {
            slots: region.players.get(&id).cloned().unwrap_or_default(),
            presets: region.presets_ready.then(|| {
                let mut presets: Vec<_> = region
                    .presets
                    .get(&id)
                    .into_iter()
                    .flat_map(|rows| rows.values().cloned())
                    .collect();
                presets.sort_unstable_by_key(|row| (row.index, row.entity_id));
                presets
            }),
        })
    }

    fn upsert(&self, region: Region, row: &EquipmentState) {
        if let Some(mut equipment) = self.0.get_mut(&region) {
            equipment
                .players
                .insert(row.entity_id, row.equipment_slots.clone());
        }
    }

    fn remove(&self, region: Region, id: u64) {
        if let Some(mut equipment) = self.0.get_mut(&region) {
            equipment.players.remove(&id);
        }
    }
}

pub(crate) fn listen(ctx: &DbConnection, region: Region, state: &AppState) {
    state
        .player_equipment
        .0
        .insert(region, RegionEquipment::default());
    let table = ctx.db.equipment_state();
    let cache = state.player_equipment.clone();
    table.on_initial_owned(move |_, rows| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.players.extend(
                rows.into_iter()
                    .map(|row| (row.entity_id, row.equipment_slots)),
            );
        }
    });
    let cache = state.player_equipment.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            cache.upsert(region, row);
        }
    });
    let cache = state.player_equipment.clone();
    table.on_update(move |_, _, row| cache.upsert(region, row));
    let cache = state.player_equipment.clone();
    table.on_delete(move |_, row| cache.remove(region, row.entity_id));

    let table = ctx.db.equipment_desc();
    let cache = state.player_equipment.clone();
    table.on_initial_owned(move |_, rows| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment
                .item_stats
                .extend(rows.into_iter().map(|row| (row.item_id, row.stats)));
        }
    });
    let cache = state.player_equipment.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied)
            && let Some(mut equipment) = cache.0.get_mut(&region)
        {
            equipment.item_stats.insert(row.item_id, row.stats.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_update(move |_, _, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.item_stats.insert(row.item_id, row.stats.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_delete(move |_, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.item_stats.remove(&row.item_id);
        }
    });

    let table = ctx.db.tool_desc();
    let cache = state.player_equipment.clone();
    table.on_initial_owned(move |_, rows| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment
                .tools
                .extend(rows.into_iter().map(|row| (row.id, row)));
        }
    });
    let cache = state.player_equipment.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied)
            && let Some(mut equipment) = cache.0.get_mut(&region)
        {
            equipment.tools.insert(row.id, row.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_update(move |_, _, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.tools.insert(row.id, row.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_delete(move |_, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.tools.remove(&row.id);
        }
    });
}

pub(super) async fn tool_powers(
    State(state): State<AppState>,
) -> axum_codec::Codec<HashMap<i32, i32>> {
    let equipment = state
        .player_equipment
        .0
        .iter()
        .find(|region| !region.tools.is_empty());
    let mut powers = HashMap::new();
    if let Some(equipment) = equipment {
        for tool in equipment.tools.values() {
            powers
                .entry(tool.item_id)
                .and_modify(|power: &mut i32| *power = (*power).max(tool.power))
                .or_insert(tool.power);
        }
    }
    axum_codec::Codec(powers)
}

pub(crate) fn subscribe_presets(ctx: &DbConnection, region: Region, state: &AppState) {
    let table = ctx.db.equipment_preset_state();
    let cache = state.player_equipment.clone();
    table.on_initial_owned(move |_, rows| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            for row in rows {
                equipment.save_preset(row);
            }
        }
    });
    let cache = state.player_equipment.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied)
            && let Some(mut equipment) = cache.0.get_mut(&region)
        {
            equipment.save_preset(row.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_update(move |_, old, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            if old.player_entity_id != row.player_entity_id {
                equipment.remove_preset(old);
            }
            equipment.save_preset(row.clone());
        }
    });
    let cache = state.player_equipment.clone();
    table.on_delete(move |_, row| {
        if let Some(mut equipment) = cache.0.get_mut(&region) {
            equipment.remove_preset(row);
        }
    });
    let applied = state.player_equipment.clone();
    let failed = state.player_equipment.clone();
    ctx.subscription_builder()
        .on_applied(move |_: &SubscriptionEventContext| {
            if let Some(mut equipment) = applied.0.get_mut(&region) {
                equipment.presets_ready = true;
            }
        })
        .on_error(move |_: &ErrorContext, err| {
            if let Some(mut equipment) = failed.0.get_mut(&region) {
                equipment.presets_ready = false;
                equipment.presets.clear();
            }
            tracing::warn!(region, "Equipment presets subscription failed: {err}");
        })
        .subscribe(["SELECT * FROM equipment_preset_state"]);
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(super) struct PlayerEquipmentResponse {
    slots: Vec<PlayerEquipmentSlot>,
    presets: Option<Vec<PlayerEquipmentPreset>>,
}

#[derive(Serialize, TS)]
#[ts(export)]
struct PlayerEquipmentPreset {
    id: String,
    index: i32,
    active: bool,
    slots: Vec<PlayerEquipmentSlot>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct PlayerEquipmentSlot {
    slot_type: String,
    item: Option<PlayerEquipmentItem>,
}

#[derive(Serialize, TS)]
#[ts(export)]
struct PlayerEquipmentItem {
    item_id: i32,
    item_type: ItemType,
    name: Option<String>,
    icon_asset_name: Option<String>,
    tier: Option<i32>,
    rarity: Option<JsonRarity>,
    stats: Option<Vec<PlayerEquipmentStat>>,
}

#[derive(Serialize, TS)]
#[ts(export)]
struct PlayerEquipmentStat {
    stat: String,
    value: f32,
    is_pct: bool,
}

fn resolve_slots(
    state: &AppState,
    region: Region,
    slots: Vec<EquipmentSlot>,
) -> Vec<PlayerEquipmentSlot> {
    let equipment = state.player_equipment.0.get(&region);
    slots
        .into_iter()
        .filter(|slot| slot.primary != EquipmentSlotType::None)
        .map(|slot| PlayerEquipmentSlot {
            slot_type: format!("{:?}", slot.primary),
            item: slot.item.map(|item| {
                let id = item.item_id;
                let (item_type, details) = match item.item_type {
                    GameItemType::Item => (
                        ItemType::Item,
                        state.item_desc.get(&id).map(|desc| {
                            (
                                desc.name.clone(),
                                desc.icon_asset_name.clone(),
                                desc.tier,
                                desc.rarity.clone(),
                            )
                        }),
                    ),
                    GameItemType::Cargo => (
                        ItemType::Cargo,
                        state.cargo_desc.get(&id).map(|desc| {
                            (
                                desc.name.clone(),
                                desc.icon_asset_name.clone(),
                                desc.tier,
                                desc.rarity.clone(),
                            )
                        }),
                    ),
                };
                let (name, icon_asset_name, tier, rarity) = details
                    .map(|(name, icon, tier, rarity)| {
                        (Some(name), Some(icon), Some(tier), Some(rarity))
                    })
                    .unwrap_or_default();
                let stats = if item.item_type == GameItemType::Item {
                    equipment
                        .as_ref()
                        .and_then(|source| source.item_stats.get(&id))
                        .map(|entries| {
                            entries
                                .iter()
                                .filter(|entry| entry.value.is_finite())
                                .map(|entry| PlayerEquipmentStat {
                                    stat: format!("{:?}", entry.id),
                                    value: entry.value,
                                    is_pct: entry.is_pct,
                                })
                                .collect()
                        })
                } else {
                    None
                };
                PlayerEquipmentItem {
                    item_id: id,
                    item_type,
                    name,
                    icon_asset_name,
                    tier,
                    rarity,
                    stats,
                }
            }),
        })
        .collect()
}

pub(super) async fn player_equipment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<PlayerEquipmentResponse>, (StatusCode, &'static str)> {
    let region = state
        .player_state
        .get(&id)
        .map(|player| player.region)
        .ok_or((StatusCode::NOT_FOUND, "Player not found"))?;
    let snapshot = state.player_equipment.snapshot(region, id as u64).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Player equipment is not available",
    ))?;
    Ok(axum_codec::Codec(PlayerEquipmentResponse {
        slots: resolve_slots(&state, region, snapshot.slots),
        presets: snapshot.presets.map(|presets| {
            presets
                .into_iter()
                .map(|row| PlayerEquipmentPreset {
                    id: row.entity_id.to_string(),
                    index: row.index,
                    active: row.active,
                    slots: resolve_slots(&state, region, row.equipment_slots),
                })
                .collect()
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::{CharacterStatType, ItemStack};

    #[test]
    fn snapshots_distinguish_missing_rows_from_disconnected_regions() {
        let cache = PlayerEquipment::default();
        cache.0.insert(1, RegionEquipment::default());
        let mut row = EquipmentState {
            entity_id: 42,
            equipment_slots: vec![EquipmentSlot {
                primary: EquipmentSlotType::MainHand,
                item: None,
            }],
        };
        cache.upsert(1, &row);
        assert!(cache.snapshot(1, 42).is_none());
        cache.ready(1);
        assert_eq!(cache.snapshot(1, 42).unwrap().slots, row.equipment_slots);
        assert!(cache.snapshot(2, 42).is_none());

        row.equipment_slots[0].primary = EquipmentSlotType::OffHand;
        cache.upsert(1, &row);
        assert_eq!(cache.snapshot(1, 42).unwrap().slots, row.equipment_slots);
        cache.remove(1, 42);
        assert!(cache.snapshot(1, 42).unwrap().slots.is_empty());
        cache.invalidate(1);
        assert!(cache.snapshot(1, 42).is_none());
    }

    #[test]
    fn presets_keep_their_active_flag_and_are_sorted_and_scoped_to_the_player() {
        let cache = PlayerEquipment::default();
        cache.0.insert(1, RegionEquipment::default());
        cache.ready(1);
        assert!(cache.snapshot(1, 42).unwrap().presets.is_none());

        let mut first = EquipmentPresetState {
            entity_id: 123,
            player_entity_id: 42,
            index: 0,
            active: false,
            equipment_slots: vec![],
        };
        let mut second = EquipmentPresetState {
            entity_id: 124,
            index: 1,
            active: true,
            ..first.clone()
        };
        {
            let mut region = cache.0.get_mut(&1).unwrap();
            region.save_preset(second.clone());
            region.save_preset(first.clone());
            region.presets_ready = true;
        }
        assert_eq!(
            cache.snapshot(1, 42).unwrap().presets.unwrap(),
            vec![first.clone(), second.clone()]
        );
        assert_eq!(cache.snapshot(1, 43).unwrap().presets, Some(vec![]));

        first.active = true;
        second.active = false;
        {
            let mut region = cache.0.get_mut(&1).unwrap();
            region.save_preset(first.clone());
            region.save_preset(second.clone());
            region.remove_preset(&second);
        }
        assert_eq!(cache.snapshot(1, 42).unwrap().presets, Some(vec![first]));
    }

    #[test]
    fn keeps_empty_slots_and_equipped_ids_without_descriptions() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(Default::default(), tx, Default::default());
        let mut equipment = RegionEquipment::default();
        equipment.item_stats.insert(
            123,
            vec![CsvStatEntry {
                id: CharacterStatType::GatheringSpeed,
                value: 0.05,
                is_pct: true,
            }],
        );
        state.player_equipment.0.insert(1, equipment);
        let slots = resolve_slots(
            &state,
            1,
            vec![
                EquipmentSlot {
                    primary: EquipmentSlotType::MainHand,
                    item: None,
                },
                EquipmentSlot {
                    primary: EquipmentSlotType::ForestryCharm,
                    item: Some(ItemStack {
                        item_id: 123,
                        quantity: 1,
                        item_type: GameItemType::Item,
                        durability: None,
                    }),
                },
                EquipmentSlot {
                    primary: EquipmentSlotType::None,
                    item: None,
                },
            ],
        );
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot_type, "MainHand");
        assert!(slots[0].item.is_none());
        assert_eq!(slots[1].slot_type, "ForestryCharm");
        let item = slots[1].item.as_ref().unwrap();
        assert_eq!(item.item_id, 123);
        assert!(item.name.is_none());
        let stats = item.stats.as_deref().unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].stat, "GatheringSpeed");
        assert_eq!(stats[0].value, 0.05);
        assert!(stats[0].is_pct);
    }
}
