//! Volatile player stat snapshots populated from regional game subscriptions.
use entity::shared::Region;
use game_module::module_bindings::{
    ActiveBuffState, BuffDesc, BuffTypeDesc, CharacterStatType, CharacterStatsState, FoodDesc,
    HealthState, StaminaState,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

#[derive(Default)]
struct RegionStats {
    ready: bool,
    health: HashMap<u64, HealthState>,
    stamina: HashMap<u64, StaminaState>,
    buffs: HashMap<u64, ActiveBuffState>,
    character_stats: HashMap<u64, (Option<f32>, Option<f32>)>,
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

// BitCraft stores deactivated entries with start and duration set to zero.
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
    if duration == 0 || start == 0 {
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
    pub fn buffs(&self, region: Region, id: u64, row: Option<ActiveBuffState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(row) = row {
                state.buffs.insert(id, row);
            } else {
                state.buffs.remove(&id);
            }
        }
    }
    pub fn character_stats(&self, region: Region, id: u64, row: Option<CharacterStatsState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(row) = row {
                // The game indexes CharacterStatsState.values by CharacterStatType.
                let maximum = |stat: CharacterStatType| {
                    row.values
                        .get(stat as usize)
                        .copied()
                        .filter(|value| value.is_finite() && *value > 0.0)
                };
                state.character_stats.insert(
                    id,
                    (
                        maximum(CharacterStatType::MaxHealth),
                        maximum(CharacterStatType::MaxStamina),
                    ),
                );
            } else {
                state.character_stats.remove(&id);
            }
        }
    }
    pub fn clear_character_stats(&self, region: Region) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            state.character_stats.clear();
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
            max_health: state
                .and_then(|state| state.character_stats.get(&id))
                .and_then(|values| values.0),
            max_stamina: state
                .and_then(|state| state.character_stats.get(&id))
                .and_then(|values| values.1),
            buffs: state.and_then(|state| state.buffs.get(&id)).map(|row| {
                row.active_buffs
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
            Some(ActiveBuffState {
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
                        buff_start_timestamp: OnlineTimestamp { value: 900 },
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
