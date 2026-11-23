//! Regional player action snapshots. Missing subscriptions or rows are unknown, not idle.
use entity::shared::Region;
use game_module::module_bindings::PlayerActionState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Default)]
struct RegionActivity {
    ready: bool,
    players: HashMap<u64, Vec<PlayerActionState>>,
}

#[derive(Default)]
pub(crate) struct PlayerActivity {
    regions: parking_lot::RwLock<HashMap<Region, RegionActivity>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub(crate) struct ActivityAction {
    pub action_type: String,
    pub successful: bool,
    pub consumed: bool,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub recipe_id: Option<i32>,
    pub target_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub(crate) struct ActivitySnapshot {
    pub player_id: String,
    pub available: bool,
    pub signed_in: bool,
    pub walking: Option<bool>,
    pub server_time_ms: u64,
    pub actions: Option<Vec<ActivityAction>>,
}

impl PlayerActivity {
    pub fn reset(&self, region: Region) {
        self.regions
            .write()
            .insert(region, RegionActivity::default());
    }
    pub fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }
    pub fn ready(&self, region: Region) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            state.ready = true;
        }
    }
    pub fn insert(&self, region: Region, row: PlayerActionState) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            // Players usually have two action rows, but auto_id is the key, not layer.
            let actions = state
                .players
                .entry(row.entity_id)
                .or_insert_with(|| Vec::with_capacity(2));
            match actions
                .iter_mut()
                .find(|action| action.auto_id == row.auto_id)
            {
                Some(action) => *action = row,
                None => actions.push(row),
            }
        }
    }
    pub fn remove(&self, region: Region, player: u64, id: u64) {
        if let Some(state) = self.regions.write().get_mut(&region)
            && let Some(actions) = state.players.get_mut(&player)
            && let Some(index) = actions.iter().position(|action| action.auto_id == id)
        {
            actions.swap_remove(index);
            if actions.is_empty() {
                state.players.remove(&player);
            }
        }
    }
    pub fn snapshot(
        &self,
        region: Region,
        player: u64,
        signed_in: bool,
        walking: Option<bool>,
    ) -> ActivitySnapshot {
        let regions = self.regions.read();
        let state = regions.get(&region).filter(|state| state.ready);
        ActivitySnapshot {
            player_id: player.to_string(),
            available: state.is_some(),
            signed_in,
            walking,
            server_time_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            actions: state
                .and_then(|state| state.players.get(&player))
                .map(|actions| {
                    actions
                        .iter()
                        .map(|action| ActivityAction {
                            action_type: format!("{:?}", action.action_type),
                            successful: action.last_action_result
                                == game_module::module_bindings::PlayerActionResult::Success,
                            consumed: action.was_consumed,
                            start_ms: action.start_time,
                            duration_ms: action.duration,
                            recipe_id: action.recipe_id,
                            target_id: action.target.map(|id| id.to_string()),
                        })
                        .collect()
                }),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../../benches/player-activity/replay.rs"]
mod replay;

pub(crate) fn player_snapshot(state: &crate::AppState, id: u64) -> Option<ActivitySnapshot> {
    let (region, signed_in) = {
        let player = state.player_state.get(&i64::try_from(id).ok()?)?;
        (player.region, player.signed_in)
    };
    let walking = state.mobile_entity_state.get(&id).map(|row| row.is_walking);
    Some(
        state
            .player_activity
            .snapshot(region, id, signed_in, walking),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::{PlayerActionLayer, PlayerActionResult, PlayerActionType};

    fn action(id: u64, player: u64, action_type: PlayerActionType) -> PlayerActionState {
        PlayerActionState {
            auto_id: id,
            chunk_index: 0,
            entity_id: player,
            start_time: 1000,
            duration: 2000,
            target: None,
            recipe_id: Some(7),
            action_type,
            layer: PlayerActionLayer::Base,
            last_action_result: PlayerActionResult::Success,
            client_cancel: false,
            was_consumed: false,
            pad_1: 0,
            pad_2: 0,
            pad_3: 0,
        }
    }
    #[test]
    fn missing_or_unsynced_rows_are_not_idle() {
        let cache = PlayerActivity::default();
        cache.reset(1);
        let before = cache.snapshot(1, 42, true, None);
        assert!(!before.available && before.actions.is_none());
        cache.ready(1);
        let missing = cache.snapshot(1, 42, true, None);
        assert!(missing.available && missing.actions.is_none());
        cache.invalidate(1);
        assert!(!cache.snapshot(1, 42, true, None).available);
    }

    #[test]
    fn retains_multiple_layers_and_removes_only_the_matching_action() {
        let cache = PlayerActivity::default();
        cache.reset(1);
        cache.insert(1, action(10, 42, PlayerActionType::Craft));
        let mut upper = action(11, 42, PlayerActionType::PlayerMove);
        upper.layer = PlayerActionLayer::UpperBody;
        cache.insert(1, upper);
        cache.ready(1);
        let actions = cache.snapshot(1, 42, true, Some(false)).actions.unwrap();
        assert_eq!(actions.len(), 2);
        assert!(
            actions
                .iter()
                .any(|action| action.action_type == "Craft" && action.recipe_id == Some(7))
        );
        cache.remove(1, 42, 10);
        let actions = cache.snapshot(1, 42, true, None).actions.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, "PlayerMove");
        cache.remove(1, 42, 11);
        assert!(cache.snapshot(1, 42, true, None).actions.is_none());
    }

    #[test]
    fn grows_past_two_rows_and_replaces_by_auto_id() {
        let cache = PlayerActivity::default();
        cache.reset(1);
        for id in [10, 11, 12] {
            cache.insert(1, action(id, 42, PlayerActionType::Craft));
        }
        cache.insert(1, action(12, 42, PlayerActionType::Extract));
        cache.remove(1, 42, 99);
        cache.ready(1);
        let actions = cache.snapshot(1, 42, true, None).actions.unwrap();
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions
                .iter()
                .filter(|a| a.action_type == "Extract")
                .count(),
            1
        );
        cache.remove(1, 42, 12);
        let actions = cache.snapshot(1, 42, true, None).actions.unwrap();
        assert_eq!(actions.len(), 2);
        assert!(actions.iter().all(|a| a.action_type == "Craft"));
    }
}
