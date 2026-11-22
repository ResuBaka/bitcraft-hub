use super::{
    ConnectionTiming, MAX_RECONNECT_RETRIES, connect_to_db, initialize_connection_metrics,
    next_reconnect_failure_count, reconnect_delay, run_connection_until_shutdown,
};
use crate::AppState;
use crate::config::Config;
use crate::leaderboard::RankingSystem;
use game_module::module_bindings::*;
use parking_lot::Mutex;
use spacetimedb_sdk::{DbContext, Event, Identity, Table, TableWithPrimaryKey};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const QUERIES: [&str; 2] = [
    "SELECT * FROM identity_role",
    "SELECT user_state.* FROM user_state JOIN identity_role ON user_state.identity = identity_role.identity",
];

pub(super) fn start(config: &Config, state: AppState) {
    let database = config.spacetimedb.database.trim().to_owned();
    let host = config.global_spacetimedb_url();
    let token = config.spacetimedb.password.clone();
    initialize_connection_metrics(&database);
    state.connection_state.insert(database.clone(), false);
    let stop = state.shutdown.stop.clone();
    state.shutdown.clone().spawn_connection(async move {
        let mut tries = 0;
        while !stop.is_cancelled() {
            let timing = ConnectionTiming::default();
            let result = match connect_to_db(
                DbConnection::builder(), state.clone(), token.clone(), &database, &host, timing.clone(), None,
            ) {
                Ok(ctx) => sync(&ctx, state.ranking_system.clone(), stop.child_token()).await,
                Err(error) => Err(error.into()),
            };
            if stop.is_cancelled() {
                break;
            }
            if let Err(error) = result {
                tracing::warn!(%database, %host, %error, "Global leaderboard staff sync failed");
            }
            tries = next_reconnect_failure_count(tries, timing.uptime());
            if tries > MAX_RECONNECT_RETRIES {
                tracing::error!(%database, %host, "Global leaderboard staff sync exhausted its retry budget");
                break;
            }
            let wait = reconnect_delay(tries, rand::random_range(15..=45));
            tokio::select! { biased;
                _ = stop.cancelled() => break,
                _ = tokio::time::sleep(wait) => {},
            }
        }
    });
}

async fn sync(
    ctx: &DbConnection,
    ranking: Arc<RankingSystem>,
    stop: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let staff = listen(ctx, ranking);
    let outcome = Arc::new(Mutex::new(None));
    let applied = outcome.clone();
    let failed = outcome.clone();
    ctx.subscription_builder()
        .on_applied(move |_| *applied.lock() = Some(Ok(())))
        .on_error(move |_, error| *failed.lock() = Some(Err(error)))
        .subscribe(QUERIES);
    loop {
        tokio::select! { biased;
            _ = stop.cancelled() => return run_connection_until_shutdown(ctx, &stop).await,
            result = ctx.advance_one_message_async() => result?,
        }
        // on_applied runs before the row callbacks. Wait for the entire SDK
        // message so an empty table or a reconnect replaces the previous snapshot.
        let result = outcome.lock().take();
        match result {
            Some(Ok(())) => staff.finish_initial(),
            Some(Err(error)) => {
                stop.cancel();
                run_connection_until_shutdown(ctx, &stop).await?;
                return Err(error.into());
            }
            None => {}
        }
    }
}

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

struct StaffSubscription {
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

    fn finish_initial(&self) {
        self.apply(|rows| {
            rows.ready = true;
            true
        });
    }
}

fn listen(ctx: &DbConnection, ranking: Arc<RankingSystem>) -> Arc<StaffSubscription> {
    let target = Arc::new(StaffSubscription {
        ranking,
        rows: Mutex::default(),
    });
    let roles = ctx.db.identity_role();
    let sync = target.clone();
    roles.on_initial_rows(move |_, initial| {
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
    users.on_initial_rows(move |_, initial| {
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
