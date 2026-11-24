use super::{
    ConnectionTiming, MAX_RECONNECT_RETRIES, connect_to_db, empire, global_usernames,
    initialize_connection_metrics, leaderboard_staff, next_reconnect_failure_count,
    reconnect_delay, run_connection_until_shutdown, shared_tables,
};
use crate::{AppState, config::Config};
use game_global_module::module_bindings::DbConnection;
use parking_lot::Mutex;
use spacetimedb_sdk::DbContext;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

type Names = super::worker_channel::UnboundedSender<
    super::SpacetimeUpdateMessages<game_module::module_bindings::PlayerUsernameState>,
>;

async fn previous_partitions(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<entity::shared::Region>, sea_orm::DbErr> {
    use sea_orm::{DbBackend, FromQueryResult, Statement};
    #[derive(FromQueryResult)]
    struct Partition {
        region: entity::shared::Region,
    }
    let query = Statement::from_string(
        DbBackend::Postgres,
        "SELECT region FROM claim_state
         UNION SELECT region FROM building_nickname_state
         UNION SELECT region FROM player_username_state",
    );
    Ok(Partition::find_by_statement(query)
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.region)
        .collect())
}

pub(super) fn start(
    config: &Config,
    state: AppState,
    workers: shared_tables::Workers,
    usernames: Names,
) {
    let database = config.spacetimedb.database.trim().to_owned();
    let host = config.global_spacetimedb_url();
    let token = config.spacetimedb.password.clone();
    let compression = config.spacetimedb.compression.into();
    initialize_connection_metrics(&database);
    state.connection_state.insert(database.clone(), false);
    let stop = state.shutdown.stop.clone();
    state.shutdown.clone().spawn_connection(async move {
        let mut tries = 0;
        while !stop.is_cancelled() {
            let timing = ConnectionTiming::default();
            let result = match connect_to_db(
                DbConnection::builder().with_compression(compression),
                state.clone(),
                token.clone(),
                &database,
                &host,
                timing.clone(),
                None,
            ) {
                Ok(ctx) => {
                    let result = sync(
                        &ctx,
                        &state,
                        stop.child_token(),
                        &workers,
                        &usernames,
                    )
                    .await;
                    state.player_stats.invalidate_food(shared_tables::GLOBAL);
                    state.player_stats.clear_tools(shared_tables::GLOBAL);
                    state.player_equipment.begin_global_metadata();
                    super::player_stats::changed(&state, None);
                    super::player_stats::character_changed(&state, None, None);
                    result
                }
                Err(error) => Err(error.into()),
            };
            if stop.is_cancelled() {
                break;
            }
            if let Err(error) = result {
                tracing::warn!(%database, %host, %error, "Global sync failed");
            }
            tries = next_reconnect_failure_count(tries, timing.uptime());
            if tries > MAX_RECONNECT_RETRIES {
                tracing::error!(%database, %host, "Global sync exhausted its retry budget");
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

type SubscriptionOutcome = Arc<Mutex<Option<spacetimedb_sdk::Result<()>>>>;

fn subscribe(ctx: &DbConnection, queries: Vec<String>) -> SubscriptionOutcome {
    let outcome = Arc::new(Mutex::new(None));
    let applied = outcome.clone();
    let failed = outcome.clone();
    ctx.subscription_builder()
        .on_applied(move |_| *applied.lock() = Some(Ok(())))
        .on_error(move |_, error| *failed.lock() = Some(Err(error)))
        .subscribe(queries);
    outcome
}

async fn sync(
    ctx: &DbConnection,
    state: &AppState,
    stop: CancellationToken,
    workers: &shared_tables::Workers,
    usernames: &Names,
) -> anyhow::Result<()> {
    let drain = || run_connection_until_shutdown(ctx, ctx.run_async(), &stop);
    let staff = leaderboard_staff::listen(ctx, state.ranking_system.clone());
    let changes = empire::Changes::listen(ctx);
    // Include old partitions even if their last row disappeared while offline.
    let regions = previous_partitions(&state.conn).await?;
    let names = global_usernames::Directory::listen(ctx);
    let mut shared = workers.listen(ctx, &regions, state);
    let directory_outcome = subscribe(
        ctx,
        [
            "identity_role",
            "user_state",
            "user_region_state",
            "player_username_state",
        ]
        .map(|t| format!("SELECT * FROM {t}"))
        .into(),
    );
    let empire_outcome = subscribe(ctx, empire::QUERIES.iter().map(|q| q.to_string()).collect());
    let shared_outcome = subscribe(
        ctx,
        shared_tables::TABLES
            .iter()
            .chain(shared_tables::METADATA)
            .map(|t| format!("SELECT * FROM {t}"))
            .collect(),
    );

    let mut directory_ready = false;
    let mut ready = false;
    let result = loop {
        tokio::select! { biased;
            _ = stop.cancelled() => return drain().await,
            result = ctx.advance_one_message_async() => result?,
        }
        // The SDK calls on_applied before row callbacks. Only a fully processed
        // success is a complete snapshot (including tables with zero rows).
        if let Err(err) = shared.check() {
            break Err(err);
        }
        match shared_outcome.lock().take() {
            Some(Ok(())) => shared.finish(),
            Some(Err(err)) => break Err(err.into()),
            None => {}
        }
        match directory_outcome.lock().take() {
            Some(Ok(())) => {
                staff.finish_initial();
                names.lock().publish(usernames, &regions, true);
                directory_ready = true;
            }
            Some(Err(err)) => break Err(err.into()),
            None if directory_ready => names.lock().publish(usernames, &regions, false),
            None => {}
        }
        let snapshot = match empire_outcome.lock().take() {
            Some(Ok(())) => {
                ready = true;
                true
            }
            Some(Err(err)) => break Err(err.into()),
            None => false,
        };
        if ready {
            let batch = std::mem::take(&mut *changes.lock());
            if let Err(err) = batch.persist(&state.conn, snapshot).await {
                break Err(err);
            }
        }
    };
    stop.cancel();
    drain().await?;
    result
}
