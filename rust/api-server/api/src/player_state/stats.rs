//! Volatile player stat snapshots populated from regional game subscriptions.
use entity::shared::Region;
use game_module::module_bindings::{
    ActiveBuff, ActiveBuffState, BuffDesc, BuffTypeDesc, CharacterStatType, CharacterStatsState,
    FoodDesc, HealthState, SkillType, StaminaState, ToolDesc, ToolTypeDesc,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use ts_rs::TS;

#[derive(Default)]
struct RegionStats {
    ready: bool,
    health: HashMap<u64, HealthState>,
    stamina: HashMap<u64, StaminaState>,
    buffs: HashMap<u64, Box<[ActiveBuff]>>,
    // Keep known-empty rows distinct from rows we haven't received.
    empty_buffs: HashSet<u64>,
    character_stats: HashMap<u64, (u32, Vec<f32>)>,
}

#[derive(Default)]
struct FoodBuffIndex {
    region: Option<Region>,
    ready: bool,
    foods: HashMap<i32, Vec<i32>>,
    buff_ids: HashSet<i32>,
}

impl FoodBuffIndex {
    fn rebuild(&mut self) {
        self.buff_ids = self.foods.values().flatten().copied().collect();
    }
}

#[derive(Default)]
pub(crate) struct PlayerStats {
    regions: parking_lot::RwLock<HashMap<Region, RegionStats>>,
    buff_desc: parking_lot::RwLock<HashMap<i32, BuffDesc>>,
    buff_types: parking_lot::RwLock<HashMap<i32, BuffTypeDesc>>,
    food_buffs: parking_lot::RwLock<FoodBuffIndex>,
    tools: parking_lot::RwLock<ToolIndex>,
    character_revision: AtomicU32,
}

#[derive(Serialize)]
pub(crate) struct CharacterStatsSnapshot {
    pub player_id: String,
    pub revision: Option<u32>,
    pub values: Option<Vec<Option<f32>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub(crate) struct CharacterStatsDelta {
    pub base_revision: u32,
    pub revision: u32,
    pub changes: Vec<(u32, Option<f32>)>,
}

pub(crate) enum CharacterStatsChange {
    Reset,
    Delta(CharacterStatsDelta),
}

#[derive(Default)]
pub(crate) struct ToolIndex {
    pub region: Option<Region>,
    pub ready: bool,
    pub descriptions: BTreeMap<i32, ToolDesc>,
    pub types: BTreeMap<i32, ToolTypeDesc>,
}

#[derive(Serialize)]
pub(crate) struct ToolPowers {
    pub slots_by_skill: BTreeMap<i32, i32>,
    pub power_by_item: BTreeMap<i32, i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub(crate) struct BuffSnapshot {
    pub buff_id: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_asset_name: Option<String>,
    pub duration: i32,
    pub start_timestamp: i32,
    pub values: Vec<f32>,
    pub remaining_seconds: Option<i64>,
    pub paused: bool,
    pub stats: Option<Vec<BuffStat>>,
    pub is_food_associated: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub(crate) struct BuffStat {
    pub name: String,
    pub value: f32,
    pub is_percent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub(crate) struct StatsSnapshot {
    pub player_id: String,
    pub available: bool,
    pub health: Option<f32>,
    pub stamina: Option<f32>,
    pub max_health: Option<f32>,
    pub max_stamina: Option<f32>,
    pub buffs: Option<Vec<BuffSnapshot>>,
    pub buffs_complete: bool,
    pub food_metadata_available: bool,
}

// This check doesn't require the current time, sign-in state, or buff metadata.
fn is_deactivated(start: i32, duration: i32) -> bool {
    duration >= 0 && (duration == 0 || start == 0)
}

// Negative durations are persistent. Online-only timestamps are stored as
// elapsed seconds while signed out, with -1 representing zero elapsed.
fn buff_time(
    start: i32,
    duration: i32,
    online_only: Option<bool>,
    signed_in: bool,
    now: i64,
) -> Result<Option<(Option<i64>, bool)>, ()> {
    if duration < 0 {
        return Ok(Some((None, false)));
    }
    if is_deactivated(start, duration) {
        return Ok(None);
    }
    if !signed_in && online_only.is_none() && i64::from(start) + i64::from(duration) <= now {
        return Err(());
    }
    let paused = !signed_in && online_only == Some(true);
    let remaining = if paused {
        i64::from(duration) - if start == -1 { 0 } else { i64::from(start) }
    } else {
        i64::from(start) + i64::from(duration) - now
    };
    Ok((remaining > 0).then_some((Some(remaining), paused)))
}

impl PlayerStats {
    pub fn reset(&self, region: Region) {
        self.regions.write().insert(region, RegionStats::default());
    }
    pub fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }
    pub fn ready(&self, region: Region) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            state.ready = true;
        }
    }
    pub fn health(&self, region: Region, id: u64, row: Option<HealthState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(row) = row {
                state.health.insert(id, row);
            } else {
                state.health.remove(&id);
            }
        }
    }
    pub fn stamina(&self, region: Region, id: u64, row: Option<StaminaState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(row) = row {
                state.stamina.insert(id, row);
            } else {
                state.stamina.remove(&id);
            }
        }
    }
    pub fn buffs(&self, region: Region, id: u64, row: Option<&ActiveBuffState>) {
        let mut regions = self.regions.write();
        let Some(state) = regions.get_mut(&region) else {
            return;
        };
        if let Some(row) = row {
            let buffs: Box<[_]> = row
                .active_buffs
                .iter()
                .filter(|buff| !is_deactivated(buff.buff_start_timestamp.value, buff.buff_duration))
                .cloned()
                .collect();
            if buffs.is_empty() {
                state.buffs.remove(&id);
                state.empty_buffs.insert(id);
            } else {
                state.empty_buffs.remove(&id);
                state.buffs.insert(id, buffs);
            }
        } else {
            state.buffs.remove(&id);
            state.empty_buffs.remove(&id);
        }
    }
    pub fn character_stats(
        &self,
        region: Region,
        id: u64,
        row: Option<CharacterStatsState>,
    ) -> Option<CharacterStatsChange> {
        let mut regions = self.regions.write();
        let rows = &mut regions.get_mut(&region)?.character_stats;
        let Some(row) = row.filter(|row| !row.values.is_empty()) else {
            return rows.remove(&id).map(|_| CharacterStatsChange::Reset);
        };
        let next_revision = || {
            self.character_revision
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1)
        };
        if let Some((base_revision, previous)) = rows.get_mut(&id) {
            if previous.len() == row.values.len() {
                let changes: Vec<_> = previous
                    .iter()
                    .zip(&row.values)
                    .enumerate()
                    .filter_map(|(index, (&old, &new))| {
                        let old = old.is_finite().then_some(old);
                        let new = new.is_finite().then_some(new);
                        (old != new).then_some((index as u32, new))
                    })
                    .collect();
                if changes.is_empty() {
                    return None;
                }
                let revision = next_revision();
                let delta = CharacterStatsDelta {
                    base_revision: *base_revision,
                    revision,
                    changes,
                };
                *base_revision = revision;
                *previous = row.values;
                return Some(CharacterStatsChange::Delta(delta));
            }
        }
        rows.insert(id, (next_revision(), row.values));
        Some(CharacterStatsChange::Reset)
    }
    pub fn clear_character_stats(&self, region: Region) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            state.character_stats.clear();
        }
    }
    pub fn begin_tools(&self, region: Region) {
        *self.tools.write() = ToolIndex {
            region: Some(region),
            ..Default::default()
        };
    }
    pub fn clear_tools(&self, region: Region) {
        self.update_tools(region, |index| *index = ToolIndex::default());
    }
    pub fn update_tools(&self, region: Region, update: impl FnOnce(&mut ToolIndex)) {
        let mut index = self.tools.write();
        if index.region == Some(region) {
            update(&mut index);
        }
    }
    pub fn tool_powers(&self) -> Option<ToolPowers> {
        let index = self.tools.read();
        if !index.ready {
            return None;
        }
        let mut slots_by_skill: BTreeMap<_, _> = index
            .types
            .values()
            .filter(|kind| kind.id > 0 && kind.skill_id > 0)
            .map(|kind| (kind.skill_id, kind.id - 1))
            .collect();
        // The mallet has tool type 14 even in data where it has no assigned skill.
        if index.types.contains_key(&14) {
            slots_by_skill
                .entry(SkillType::Construction as i32)
                .or_insert(13);
        }
        let mut power_by_item = BTreeMap::new();
        for tool in index.descriptions.values() {
            power_by_item.entry(tool.item_id).or_insert(tool.power);
        }
        Some(ToolPowers {
            slots_by_skill,
            power_by_item,
        })
    }
    pub fn character_snapshot(&self, region: Region, id: u64) -> CharacterStatsSnapshot {
        let regions = self.regions.read();
        // Preserve the enum-indexed layout and distinguish missing or invalid
        // data from valid zeroes (e.g. armor or crit chance).
        let row = regions
            .get(&region)
            .and_then(|state| state.character_stats.get(&id));
        let values = row.map(|(_, values)| {
            values
                .iter()
                .map(|&value| value.is_finite().then_some(value))
                .collect()
        });
        CharacterStatsSnapshot {
            player_id: id.to_string(),
            revision: row.map(|(revision, _)| *revision),
            values,
        }
    }
    pub fn buff_desc(&self, id: i32, row: Option<BuffDesc>) {
        if let Some(row) = row {
            self.buff_desc.write().insert(id, row);
        } else {
            self.buff_desc.write().remove(&id);
        }
    }
    pub fn buff_type(&self, id: i32, row: Option<BuffTypeDesc>) {
        if let Some(row) = row {
            self.buff_types.write().insert(id, row);
        } else {
            self.buff_types.write().remove(&id);
        }
    }
    pub fn begin_food(&self, region: Region) {
        *self.food_buffs.write() = FoodBuffIndex {
            region: Some(region),
            ..Default::default()
        };
    }
    pub fn food(&self, region: Region, id: i32, row: Option<FoodDesc>) {
        let mut index = self.food_buffs.write();
        if index.region != Some(region) {
            return;
        }
        if let Some(row) = row {
            index
                .foods
                .insert(id, row.buffs.iter().map(|buff| buff.buff_id).collect());
        } else {
            index.foods.remove(&id);
        }
        if index.ready {
            index.rebuild();
        }
    }
    pub fn finish_food(&self, region: Region) {
        let mut index = self.food_buffs.write();
        if index.region == Some(region) {
            index.rebuild();
            index.ready = true;
        }
    }
    pub fn invalidate_food(&self, region: Region) {
        let mut index = self.food_buffs.write();
        if index.region == Some(region) {
            *index = FoodBuffIndex::default();
        }
    }
    pub fn snapshot(&self, region: Region, id: u64, signed_in: bool) -> StatsSnapshot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        self.snapshot_at(region, id, signed_in, now)
    }

    fn snapshot_at(&self, region: Region, id: u64, signed_in: bool, now: i64) -> StatsSnapshot {
        let regions = self.regions.read();
        let descriptions = self.buff_desc.read();
        let types = self.buff_types.read();
        let food = self.food_buffs.read();
        let state = regions.get(&region).filter(|state| state.ready);
        let maximum = |stat: CharacterStatType| {
            state?
                .character_stats
                .get(&id)?
                .1
                .get(stat as usize)
                .copied()
                .filter(|value| value.is_finite() && *value > 0.0)
        };
        let buffs = state.and_then(|state| {
            state
                .buffs
                .get(&id)
                .map(Box::as_ref)
                .or_else(|| state.empty_buffs.contains(&id).then_some(&[]))
        });
        let mut buffs_complete = true;
        StatsSnapshot {
            player_id: id.to_string(),
            available: state.is_some(),
            health: state
                .and_then(|state| state.health.get(&id))
                .map(|row| row.health),
            stamina: state
                .and_then(|state| state.stamina.get(&id))
                .map(|row| row.stamina),
            max_health: maximum(CharacterStatType::MaxHealth),
            max_stamina: maximum(CharacterStatType::MaxStamina),
            buffs: buffs.map(|buffs| {
                buffs
                    .iter()
                    .filter_map(|buff| {
                        let desc = descriptions.get(&buff.buff_id);
                        let (remaining_seconds, paused) = match buff_time(
                            buff.buff_start_timestamp.value,
                            buff.buff_duration,
                            desc.map(|desc| desc.online_timestamp),
                            signed_in,
                            now,
                        ) {
                            Ok(Some(time)) => time,
                            Ok(None) => return None,
                            Err(()) => {
                                buffs_complete = false;
                                return None;
                            }
                        };
                        Some(BuffSnapshot {
                            buff_id: buff.buff_id,
                            name: descriptions
                                .get(&buff.buff_id)
                                .and_then(|desc| types.get(&desc.buff_type_id))
                                .map(|kind| kind.name.clone()),
                            description: descriptions
                                .get(&buff.buff_id)
                                .map(|desc| desc.description.clone()),
                            icon_asset_name: descriptions
                                .get(&buff.buff_id)
                                .map(|desc| desc.icon_asset_name.clone()),
                            duration: buff.buff_duration,
                            start_timestamp: buff.buff_start_timestamp.value,
                            values: buff.values.clone(),
                            remaining_seconds,
                            paused,
                            is_food_associated: food
                                .ready
                                .then(|| food.buff_ids.contains(&buff.buff_id)),
                            stats: desc.map(|desc| {
                                desc.stats
                                    .iter()
                                    .enumerate()
                                    .map(|(index, stat)| BuffStat {
                                        name: format!("{:?}", stat.id),
                                        value: buff
                                            .values
                                            .get(index)
                                            .copied()
                                            .unwrap_or(stat.value),
                                        is_percent: stat.is_pct,
                                    })
                                    .collect()
                            }),
                        })
                    })
                    .collect()
            }),
            buffs_complete,
            food_metadata_available: food.ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsynced_or_missing_rows_are_not_reported_as_zero() {
        let stats = PlayerStats::default();
        let region: Region = 1;
        assert!(!stats.snapshot_at(region, 42, true, 1000).available);
        stats.reset(region);
        assert!(!stats.snapshot_at(region, 42, true, 1000).available);
        stats.ready(region);
        let snapshot = stats.snapshot_at(region, 42, true, 1000);
        assert!(snapshot.available);
        assert!(
            snapshot.health.is_none() && snapshot.stamina.is_none() && snapshot.buffs.is_none()
        );
        stats.invalidate(region);
        assert!(!stats.snapshot_at(region, 42, true, 1000).available);
    }

    #[test]
    fn snapshots_follow_regional_updates_and_removals() {
        let stats = PlayerStats::default();
        let region: Region = 1;
        stats.reset(region);
        stats.health(
            region,
            42,
            Some(HealthState {
                entity_id: 42,
                last_health_decrease_timestamp:
                    spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
                health: 75.0,
                died_timestamp: 0,
            }),
        );
        assert_eq!(stats.snapshot_at(region, 42, true, 1000).health, None);
        stats.ready(region);
        assert_eq!(stats.snapshot_at(region, 42, true, 1000).health, Some(75.0));
        assert_eq!(stats.snapshot_at(2, 42, true, 1000).health, None);
        stats.health(region, 42, None);
        assert_eq!(stats.snapshot_at(region, 42, true, 1000).health, None);
    }

    #[test]
    fn buff_timing_excludes_expired_and_handles_paused_and_persistent() {
        assert_eq!(buff_time(0, 0, Some(true), true, 1000), Ok(None));
        assert_eq!(buff_time(900, 100, Some(false), true, 1000), Ok(None));
        assert_eq!(
            buff_time(900, 120, Some(false), true, 1000),
            Ok(Some((Some(20), false)))
        );
        assert_eq!(
            buff_time(10, 100, Some(true), false, 1000),
            Ok(Some((Some(90), true)))
        );
        assert_eq!(
            buff_time(-1, 100, Some(true), false, 1000),
            Ok(Some((Some(100), true)))
        );
        assert_eq!(
            buff_time(900, -1, None, false, 1000),
            Ok(Some((None, false)))
        );
        assert_eq!(buff_time(10, 100, None, false, 1000), Err(()));
    }

    #[test]
    fn snapshot_returns_only_active_buff_records() {
        use game_module::module_bindings::{ActiveBuff, OnlineTimestamp};
        let stats = PlayerStats::default();
        stats.reset(1);
        stats.buffs(
            1,
            42,
            Some(&ActiveBuffState {
                entity_id: 42,
                active_buffs: vec![
                    ActiveBuff {
                        buff_id: 1,
                        buff_start_timestamp: OnlineTimestamp { value: 0 },
                        buff_duration: 0,
                        values: vec![],
                    },
                    ActiveBuff {
                        buff_id: 2,
                        buff_start_timestamp: OnlineTimestamp { value: 900 },
                        buff_duration: 50,
                        values: vec![],
                    },
                    ActiveBuff {
                        buff_id: 3,
                        buff_start_timestamp: OnlineTimestamp { value: 900 },
                        buff_duration: 200,
                        values: vec![],
                    },
                    ActiveBuff {
                        buff_id: 4,
                        buff_start_timestamp: OnlineTimestamp { value: 0 },
                        buff_duration: -1,
                        values: vec![],
                    },
                ],
            }),
        );
        stats.ready(1);
        let buffs = stats.snapshot_at(1, 42, true, 1000).buffs.unwrap();
        assert_eq!(
            buffs.iter().map(|buff| buff.buff_id).collect::<Vec<_>>(),
            [3, 4]
        );
        assert_eq!(buffs[0].remaining_seconds, Some(100));
        assert_eq!(buffs[1].remaining_seconds, None);
        // An elapsed timed buff is still retained: its interpretation can depend
        // on metadata and sign-in state. The zero-duration placeholder is not.
        assert_eq!(stats.regions.read()[&1].buffs[&42].len(), 3);
    }

    #[test]
    fn empty_buff_updates_evict_payload_without_hiding_known_empty_state() {
        use game_module::module_bindings::OnlineTimestamp;

        let stats = PlayerStats::default();
        stats.reset(1);
        stats.ready(1);
        let active = ActiveBuff {
            buff_id: 1,
            buff_start_timestamp: OnlineTimestamp { value: -1 },
            buff_duration: 100,
            values: vec![42.0],
        };
        let inactive = ActiveBuff {
            buff_start_timestamp: OnlineTimestamp { value: 0 },
            ..active.clone()
        };
        let mut row = ActiveBuffState {
            entity_id: 42,
            active_buffs: vec![],
        };
        stats.buffs(1, 42, Some(&row));
        assert!(!stats.regions.read()[&1].buffs.contains_key(&42));
        let snapshot = stats.snapshot_at(1, 42, true, 1000);
        assert!(snapshot.buffs.unwrap().is_empty());

        row.active_buffs = vec![active];
        stats.buffs(1, 42, Some(&row));
        assert_eq!(stats.regions.read()[&1].buffs[&42].len(), 1);
        assert!(!stats.regions.read()[&1].empty_buffs.contains(&42));

        row.active_buffs = vec![inactive];
        stats.buffs(1, 42, Some(&row));
        assert!(!stats.regions.read()[&1].buffs.contains_key(&42));
        let snapshot = stats.snapshot_at(1, 42, true, 1000);
        assert!(snapshot.buffs.unwrap().is_empty());

        stats.buffs(1, 42, None);
        assert!(!stats.regions.read()[&1].empty_buffs.contains(&42));
        assert!(stats.snapshot_at(1, 42, true, 1000).buffs.is_none());
    }

    #[test]
    fn maximum_values_follow_character_stat_types_and_updates() {
        let stats = PlayerStats::default();
        stats.reset(1);
        stats.ready(1);
        stats.character_stats(
            1,
            42,
            Some(CharacterStatsState {
                entity_id: 42,
                values: vec![200.0, 150.0],
            }),
        );
        let snapshot = stats.snapshot_at(1, 42, true, 1000);
        assert_eq!(snapshot.max_health, Some(200.0));
        assert_eq!(snapshot.max_stamina, Some(150.0));
        stats.character_stats(
            1,
            42,
            Some(CharacterStatsState {
                entity_id: 42,
                values: vec![0.0],
            }),
        );
        let snapshot = stats.snapshot_at(1, 42, true, 1000);
        assert_eq!(snapshot.max_health, None);
        assert_eq!(snapshot.max_stamina, None);
        stats.clear_character_stats(1);
        assert_eq!(stats.snapshot_at(1, 42, true, 1000).max_health, None);
    }

    #[test]
    fn character_details_preserve_stat_indices_and_real_zeroes() {
        let stats = PlayerStats::default();
        stats.reset(2);
        stats.character_stats(
            2,
            42,
            Some(CharacterStatsState {
                entity_id: 42,
                values: vec![150.0, f32::NAN, 0.0],
            }),
        );
        let snapshot = stats.character_snapshot(2, 42);
        assert_eq!(snapshot.player_id, "42");
        assert_eq!(snapshot.values.unwrap(), [Some(150.0), None, Some(0.0)]);
        assert!(stats.character_snapshot(1, 42).values.is_none());
        stats.clear_character_stats(2);
        assert!(stats.character_snapshot(2, 42).values.is_none());
    }

    #[test]
    fn character_updates_only_send_changed_slots_and_reset_if_shape_changes() {
        let stats = PlayerStats::default();
        stats.reset(1);
        let update = |values| {
            stats.character_stats(
                1,
                42,
                Some(CharacterStatsState {
                    entity_id: 42,
                    values,
                }),
            )
        };
        assert!(matches!(
            update(vec![100.0, 0.0, f32::NAN]),
            Some(CharacterStatsChange::Reset)
        ));
        let base = stats.character_snapshot(1, 42).revision.unwrap();
        assert!(update(vec![100.0, 0.0, f32::INFINITY]).is_none());
        let Some(CharacterStatsChange::Delta(delta)) = update(vec![100.0, 4.0, 0.0]) else {
            panic!("expected changed fields");
        };
        assert_eq!(delta.base_revision, base);
        assert_eq!(delta.changes, [(1, Some(4.0)), (2, Some(0.0))]);
        assert_eq!(
            stats.character_snapshot(1, 42).revision,
            Some(delta.revision)
        );
        assert!(matches!(
            update(vec![100.0]),
            Some(CharacterStatsChange::Reset)
        ));
        assert!(matches!(
            stats.character_stats(1, 42, None),
            Some(CharacterStatsChange::Reset)
        ));
    }

    #[test]
    fn tool_powers_use_player_toolbelt_slot_and_item_id() {
        let stats = PlayerStats::default();
        stats.begin_tools(1);
        let tool = ToolDesc {
            id: 44,
            item_id: 842,
            tool_type: 4,
            level: 2,
            power: 23,
        };
        stats.update_tools(1, |index| {
            index.descriptions.insert(tool.id, tool);
            index.types.insert(
                4,
                ToolTypeDesc {
                    id: 4,
                    skill_id: SkillType::Mining as i32,
                    name: "Pickaxe".into(),
                },
            );
        });
        assert!(stats.tool_powers().is_none());
        stats.update_tools(1, |index| index.ready = true);
        stats.clear_tools(2);
        let result = stats.tool_powers().unwrap();
        assert_eq!(result.slots_by_skill[&(SkillType::Mining as i32)], 3);
        assert_eq!(result.power_by_item[&842], 23);
        stats.clear_tools(1);
        assert!(stats.tool_powers().is_none());
    }

    #[test]
    fn food_index_tracks_shared_effects_and_removals() {
        let mut index = FoodBuffIndex::default();
        index.foods.insert(1, vec![4, 5]);
        index.foods.insert(2, vec![5, 6]);
        index.rebuild();
        assert_eq!(index.buff_ids, HashSet::from([4, 5, 6]));
        index.foods.remove(&1);
        index.rebuild();
        assert_eq!(index.buff_ids, HashSet::from([5, 6]));
    }
}
