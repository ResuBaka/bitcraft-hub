use entity::skill_desc::Model;
use std::collections::HashMap;
use std::ops::Deref;

// Captured XP stacks use IDs 1..=22. Keep the common rescan independent of
// descriptor size, without sizing an allocation by an upstream or database ID.
const DENSE_IDS: usize = 64;

pub(crate) struct SkillDefinitions {
    rows: HashMap<i64, Model>,
    categories: [Option<i32>; DENSE_IDS],
}

impl SkillDefinitions {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            rows: HashMap::with_capacity(capacity),
            categories: [None; DENSE_IDS],
        }
    }

    pub(crate) fn category(&self, id: i32) -> Option<i32> {
        self.categories
            .get(id as usize)
            .copied()
            .flatten()
            .or_else(|| self.rows.get(&i64::from(id)).map(|row| row.skill_category))
    }

    fn slot(&mut self, id: i64) -> Option<&mut Option<i32>> {
        self.categories.get_mut(usize::try_from(id).ok()?)
    }

    pub(crate) fn insert(&mut self, id: i64, row: Model) {
        if let Some(slot) = self.slot(id) {
            *slot = Some(row.skill_category);
        }
        self.rows.insert(id, row);
    }

    pub(crate) fn remove(&mut self, id: &i64) {
        if let Some(slot) = self.slot(*id) {
            *slot = None;
        }
        self.rows.remove(id);
    }
}

impl Default for SkillDefinitions {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

// Existing descriptor consumers get ordinary map reads. No DerefMut: all
// writes maintain the index under the same write guard as the full rows.
impl Deref for SkillDefinitions {
    type Target = HashMap<i64, Model>;

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}
