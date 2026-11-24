//! Regional player action snapshots. Missing subscriptions or rows are unknown, not idle.
use crate::crafting::station_projects::StationProjects;
use entity::shared::Region;
use game_module::module_bindings::{PlayerActionState, PlayerActionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use ts_rs::TS;

#[derive(Default)]
struct RegionActivity {
    ready: bool,
    players: HashMap<u64, Vec<PlayerActionState>>,
    craft_starts: HashMap<u64, CraftStart>,
    station_projects: Arc<StationProjects>,
}

struct CraftStart {
    timestamp: u64,
    project: u64,
    owner: u64,
}

impl RegionActivity {
    fn craft_project(&self, action: &PlayerActionState) -> Option<(u64, u64)> {
        if action.action_type != PlayerActionType::Craft {
            return None;
        }
        let player = action.entity_id;
        if let Some(start) = self
            .craft_starts
            .get(&player)
            .filter(|s| s.timestamp == action.start_time)
        {
            return Some((start.project, start.owner));
        }
        self.station_projects
            .resolve(player, action.target?, action.recipe_id?)
    }
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
    pub craft_id: Option<String>,
    pub craft_owner_id: Option<String>,
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
    // The action targets the station. Only the continue-start event identifies
    // the project, and its request timestamp is also the action's start_time.
    pub fn craft_started(
        &self,
        region: Region,
        player: u64,
        timestamp: u64,
        project: u64,
        owner: u64,
    ) -> bool {
        let mut regions = self.regions.write();
        let Some(state) = regions.get_mut(&region) else {
            return false;
        };
        state.craft_starts.insert(
            player,
            CraftStart {
                timestamp,
                project,
                owner,
            },
        );
        // Either subscription may deliver first. If the action already arrived,
        // publish again now that we can identify it.
        state.players.get(&player).is_some_and(|actions| {
            actions.iter().any(|a| {
                a.action_type == game_module::module_bindings::PlayerActionType::Craft
                    && a.start_time == timestamp
            })
        })
    }

    pub fn reset(&self, region: Region) -> Arc<StationProjects> {
        let state = RegionActivity::default();
        let projects = state.station_projects.clone();
        self.regions.write().insert(region, state);
        projects
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
                state.craft_starts.remove(&player);
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
            actions: state.and_then(|state| {
                Some(
                    state
                        .players
                        .get(&player)?
                        .iter()
                        .map(|action| {
                            let craft = state.craft_project(action);
                            ActivityAction {
                                action_type: format!("{:?}", action.action_type),
                                successful: action.last_action_result
                                    == game_module::module_bindings::PlayerActionResult::Success,
                                consumed: action.was_consumed,
                                start_ms: action.start_time,
                                duration_ms: action.duration,
                                recipe_id: action.recipe_id,
                                target_id: action.target.map(|id| id.to_string()),
                                craft_id: craft.map(|(project, _)| project.to_string()),
                                craft_owner_id: craft.map(|(_, owner)| owner.to_string()),
                            }
                        })
                        .collect(),
                )
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
    fn restarted_region_resolves_a_public_station_project_without_a_start_event() {
        use game_module::module_bindings::ProgressiveActionState;
        let cache = PlayerActivity::default();
        let stations = cache.reset(1);
        let public_project = ProgressiveActionState {
            entity_id: 9007199254740993,
            building_entity_id: 12,
            owner_entity_id: 43,
            recipe_id: 203006,
            function_type: 0,
            progress: 17656,
            craft_count: 2000,
            last_crit_outcome: 0,
            lock_expiration: spacetimedb_sdk::Timestamp::UNIX_EPOCH,
            preparation: false,
        };
        let private_project = ProgressiveActionState {
            entity_id: 15,
            owner_entity_id: 44,
            ..public_project.clone()
        };
        stations.reset(&[public_project.clone(), private_project.clone()]);
        stations.reset_public([public_project.entity_id].into_iter());
        let mut row = action(10, 42, PlayerActionType::Craft);
        row.target = Some(12);
        row.recipe_id = Some(203006);
        cache.insert(1, row);
        cache.ready(1);
        let project = || {
            let a = cache.snapshot(1, 42, true, None).actions.unwrap().remove(0);
            (a.craft_id, a.craft_owner_id)
        };
        assert_eq!(
            project(),
            (Some("9007199254740993".into()), Some("43".into()))
        );
        // A second accessible project makes a station-only match ambiguous.
        stations.change_public(None, Some(private_project.entity_id));
        assert_eq!(project(), (None, None));
        stations.change(Some(&private_project), None);
        assert_eq!(
            project(),
            (Some("9007199254740993".into()), Some("43".into()))
        );
        stations.change_public(Some(public_project.entity_id), None);
        assert_eq!(project(), (None, None));
    }

    #[test]
    fn ties_shared_projects_to_the_exact_action_in_either_delivery_order() {
        let cache = PlayerActivity::default();
        cache.reset(1);
        cache.ready(1);
        let mut row = action(10, 42, PlayerActionType::Craft);
        row.target = Some(2);
        let project = |cache: &PlayerActivity| {
            let a = cache.snapshot(1, 42, true, None).actions.unwrap().remove(0);
            assert_eq!(a.target_id.as_deref(), Some("2"));
            (a.craft_id, a.craft_owner_id)
        };
        assert!(!cache.craft_started(1, 42, row.start_time, 123, 43));
        cache.insert(1, row.clone());
        assert_eq!(project(&cache), (Some("123".into()), Some("43".into())));
        row.start_time += 1;
        cache.insert(1, row.clone());
        assert_eq!(project(&cache), (None, None));
        assert!(cache.craft_started(1, 42, row.start_time, 124, 44));
        assert_eq!(project(&cache), (Some("124".into()), Some("44".into())));
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
