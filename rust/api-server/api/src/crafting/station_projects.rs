use game_module::module_bindings::ProgressiveActionState;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};

type StationRecipe = (u64, i32);
type ProjectOwner = (u64, u64);

#[derive(Default)]
struct Index {
    projects: HashMap<StationRecipe, Vec<ProjectOwner>>,
    public: HashSet<u64>,
}

/// The action identifies a station and recipe even without a continue-start
/// event. Seed from the regional snapshots so this also works after a restart.
#[derive(Default)]
pub(crate) struct StationProjects(RwLock<Index>);

fn key(row: &ProgressiveActionState) -> StationRecipe {
    (row.building_entity_id, row.recipe_id)
}

impl StationProjects {
    pub fn reset(&self, rows: &[ProgressiveActionState]) {
        let mut projects: HashMap<_, Vec<_>> = HashMap::new();
        for row in rows {
            projects
                .entry(key(row))
                .or_default()
                .push((row.entity_id, row.owner_entity_id));
        }
        self.0.write().projects = projects;
    }

    pub fn change(
        &self,
        old: Option<&ProgressiveActionState>,
        new: Option<&ProgressiveActionState>,
    ) {
        let entry = |r: &ProgressiveActionState| (key(r), (r.entity_id, r.owner_entity_id));
        let (old, new) = (old.map(entry), new.map(entry));
        if old == new {
            return;
        } // Progress and lock updates don't change the index.
        let mut index = self.0.write();
        if let Some((key, (id, _))) = old
            && let Some(rows) = index.projects.get_mut(&key)
        {
            rows.retain(|&(p, _)| p != id);
            if rows.is_empty() {
                index.projects.remove(&key);
            }
        }
        if let Some((key, project)) = new {
            let rows = index.projects.entry(key).or_default();
            if !rows.contains(&project) {
                rows.push(project);
            }
        }
    }

    pub fn reset_public(&self, ids: impl Iterator<Item = u64>) {
        self.0.write().public = ids.collect();
    }
    pub fn change_public(&self, old: Option<u64>, new: Option<u64>) {
        if old == new {
            return;
        }
        let mut index = self.0.write();
        if let Some(id) = old {
            index.public.remove(&id);
        }
        if let Some(id) = new {
            index.public.insert(id);
        }
    }
    pub fn resolve(&self, player: u64, building: u64, recipe: i32) -> Option<ProjectOwner> {
        let index = self.0.read();
        let mut eligible = index
            .projects
            .get(&(building, recipe))?
            .iter()
            .copied()
            .filter(|(id, owner)| *owner == player || index.public.contains(id));
        let project = eligible.next()?;
        eligible.next().is_none().then_some(project)
    }
}
