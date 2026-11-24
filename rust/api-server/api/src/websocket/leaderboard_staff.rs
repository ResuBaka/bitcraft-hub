use crate::leaderboard::RankingSystem;
use game_global_module::module_bindings::*;
use parking_lot::Mutex;
use spacetimedb_sdk::{Event, Identity, Table, TableWithPrimaryKey};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
struct RegionRows {
    users: HashMap<Identity, i64>,
    staff: HashSet<Identity>,
    ready: bool,
}

impl RegionRows {
    fn role(&mut self, identity: Identity, role: Option<Role>) -> bool {
        if matches!(role, Some(Role::Gm | Role::Admin)) {
            self.staff.insert(identity)
        } else {
            self.staff.remove(&identity)
        }
    }

    fn user(&mut self, identity: Identity, entity_id: Option<i64>) -> bool {
        let previous = match entity_id {
            Some(id) => self.users.insert(identity, id),
            None => self.users.remove(&identity),
        };
        entity_id != previous && self.staff.contains(&identity)
    }

    fn staff_players(&self) -> HashSet<i64> {
        self.staff
            .iter()
            .filter_map(|id| self.users.get(id).copied())
            .collect()
    }
}

pub(super) struct StaffSubscription {
    ranking: Arc<RankingSystem>,
    rows: Mutex<RegionRows>,
}

impl StaffSubscription {
    fn apply(&self, update: impl FnOnce(&mut RegionRows) -> bool) {
        let mut rows = self.rows.lock();
        if update(&mut rows) && rows.ready {
            self.ranking.replace_staff(rows.staff_players());
        }
    }

    pub(super) fn finish_initial(&self) {
        self.apply(|rows| {
            rows.ready = true;
            true
        });
    }
}

pub(super) fn listen(ctx: &DbConnection, ranking: Arc<RankingSystem>) -> Arc<StaffSubscription> {
    let target = Arc::new(StaffSubscription {
        ranking,
        rows: Mutex::default(),
    });
    let roles = ctx.db.identity_role();
    let sync = target.clone();
    roles.on_initial(move |_, initial| {
        sync.apply(|rows| {
            for row in initial.iter() {
                rows.role(row.identity, Some(row.role));
            }
            true
        })
    });
    let sync = target.clone();
    roles.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            sync.apply(|rows| rows.role(row.identity, Some(row.role)));
        }
    });
    let sync = target.clone();
    roles.on_update(move |_, _, row| sync.apply(|rows| rows.role(row.identity, Some(row.role))));
    let sync = target.clone();
    roles.on_delete(move |_, row| sync.apply(|rows| rows.role(row.identity, None)));

    let users = ctx.db.user_state();
    let sync = target.clone();
    users.on_initial(move |_, initial| {
        sync.apply(|rows| {
            for row in initial.iter() {
                rows.user(row.identity, Some(row.entity_id as i64));
            }
            true
        })
    });
    let sync = target.clone();
    users.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            sync.apply(|rows| rows.user(row.identity, Some(row.entity_id as i64)));
        }
    });
    let sync = target.clone();
    users.on_update(move |_, old, row| {
        sync.apply(|rows| {
            let removed = old.identity != row.identity && rows.user(old.identity, None);
            rows.user(row.identity, Some(row.entity_id as i64)) || removed
        })
    });
    let sync = target.clone();
    users.on_delete(move |_, row| sync.apply(|rows| rows.user(row.identity, None)));
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription(ranking: &Arc<RankingSystem>) -> StaffSubscription {
        StaffSubscription {
            ranking: ranking.clone(),
            rows: Mutex::default(),
        }
    }

    #[tokio::test]
    async fn global_staff_snapshot_hides_staff_and_compacts_ranks() {
        let ranking = Arc::new(RankingSystem::default());
        ranking.await_staff_snapshot();
        let skill = ranking.ensure_skill_leaderboard(10);
        let boards = [&ranking.global_leaderboard, &ranking.time_played, &skill];
        for board in boards {
            for id in 1..=4 {
                board.update(id, id * 100);
            }
        }
        ranking.shutdown().await;
        let mut updates = ranking.subscribe_publications();
        let global = subscription(&ranking);
        let gm = Identity::from_byte_array([1; 32]);
        let admin = Identity::from_byte_array([2; 32]);
        let relay = Identity::from_byte_array([3; 32]);
        global.apply(|rows| rows.role(gm, Some(Role::Gm)));
        global.apply(|rows| rows.user(gm, Some(4)));
        assert!(ranking.global_leaderboard.get_range(0, 10).is_empty());
        assert!(!updates.has_changed().unwrap());

        global.apply(|rows| rows.user(admin, Some(2)));
        global.apply(|rows| rows.role(admin, Some(Role::Admin)));
        global.apply(|rows| rows.user(relay, Some(3)));
        global.apply(|rows| rows.role(relay, Some(Role::Relay)));
        global.finish_initial();
        assert!(updates.has_changed().unwrap());
        for board in boards {
            assert_eq!(board.get_rank(4), None);
            assert_eq!(board.get_rank(2), None);
            assert_eq!(board.get_rank(3), Some(1));
            assert_eq!(board.get_rank(1), Some(2));
            let second = &board.get_range(1, 1)[0];
            assert_eq!((second.user_id, second.rank), (1, 2));
        }

        updates.borrow_and_update();
        global.apply(|rows| rows.role(gm, Some(Role::Player)));
        assert!(updates.has_changed().unwrap());
        assert_eq!(ranking.global_leaderboard.get_rank(4), Some(1));
        assert_eq!(ranking.global_leaderboard.get_rank(3), Some(2));

        // A reconnect with an empty snapshot replaces the old global data.
        let reconnected = subscription(&ranking);
        assert_eq!(ranking.global_leaderboard.get_rank(2), None);
        reconnected.finish_initial();
        assert_eq!(ranking.global_leaderboard.get_rank(2), Some(3));

        // Live promotions and mapping deletions do not need a score event.
        reconnected.apply(|rows| rows.user(relay, Some(3)));
        reconnected.apply(|rows| rows.role(relay, Some(Role::Admin)));
        assert_eq!(ranking.global_leaderboard.get_rank(3), None);
        reconnected.apply(|rows| rows.user(relay, None));
        assert_eq!(ranking.global_leaderboard.get_rank(3), Some(2));
    }
}
