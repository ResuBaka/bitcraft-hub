use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use entity::claim_event::{self, ClaimEventType, ClaimMemberPermissions};
use entity::shared::Region;
use game_module::module_bindings::{
    BuildingState, BuildingStateTableAccess, ClaimMemberState, ClaimMemberStateTableAccess,
    ClaimTechStateTableAccess, ClaimTreasuryChangeReason, ClaimTreasuryEvent,
    ClaimTreasuryEventTableAccess, DbConnection, Reducer,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Event, EventTable, Table, TableWithPrimaryKey};
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use ts_rs::TS;

#[derive(Deserialize)]
pub(crate) struct EventParams {
    page: Option<u64>,
    per_page: Option<u64>,
    region: Option<Region>,
    group: Option<EventGroup>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EventGroup {
    Treasury,
    Research,
    Buildings,
    Members,
}

impl EventGroup {
    fn event_types(&self) -> Vec<ClaimEventType> {
        use ClaimEventType::*;
        match self {
            Self::Treasury => vec![TreasuryDeposit, TreasuryWithdrawal],
            Self::Research => vec![ResearchCompleted],
            Self::Buildings => vec![BuildingPlaced, BuildingRemoved],
            Self::Members => vec![MemberAdded, MemberRemoved, MemberPermissionsChanged],
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
pub(crate) struct ClaimEventsResponse {
    events: Vec<claim_event::Model>,
    total: u64,
    page: u64,
    per_page: u64,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<EventParams>,
) -> Result<axum_codec::Codec<ClaimEventsResponse>, StatusCode> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);
    if page == 0 || !(1..=100).contains(&per_page) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut query = claim_event::Entity::find().filter(claim_event::Column::ClaimEntityId.eq(id));
    if let Some(region) = params.region {
        query = query.filter(claim_event::Column::Region.eq(region));
    }
    if let Some(group) = params.group {
        query = query.filter(claim_event::Column::EventType.is_in(group.event_types()));
    }
    let paginator = query
        .order_by_desc(claim_event::Column::Timestamp)
        .order_by_desc(claim_event::Column::Id)
        .paginate(&state.conn, per_page);
    let total = paginator.num_items().await.map_err(query_error)?;
    // Avoid overflowing the offset for arbitrarily large page parameters.
    let events = if page - 1 > total / per_page {
        vec![]
    } else {
        paginator.fetch_page(page - 1).await.map_err(query_error)?
    };
    Ok(axum_codec::Codec(ClaimEventsResponse {
        events,
        total,
        page,
        per_page,
    }))
}

fn query_error(error: sea_orm::DbErr) -> StatusCode {
    tracing::error!(%error, "Could not query claim events");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn live_event(event: &Event<Reducer>) -> bool {
    matches!(event, Event::Reducer(_) | Event::Transaction)
}

fn event_timestamp(event: &Event<Reducer>) -> DateTime<Utc> {
    if let Event::Reducer(reducer) = event {
        if let Some(timestamp) =
            DateTime::from_timestamp_micros(reducer.timestamp.to_micros_since_unix_epoch())
        {
            return timestamp;
        }
    }
    // Public transaction updates do not always include reducer metadata.
    Utc::now()
}

fn new_event(
    claim_id: u64,
    region: Region,
    timestamp: DateTime<Utc>,
    event_type: ClaimEventType,
) -> claim_event::ActiveModel {
    claim_event::ActiveModel {
        claim_entity_id: Set(claim_id as i64),
        region: Set(region),
        timestamp: Set(timestamp),
        event_type: Set(event_type),
        actor_entity_id: Set(None),
        amount: Set(None),
        treasury_after: Set(None),
        research_id: Set(None),
        building_entity_id: Set(None),
        building_description_id: Set(None),
        subject_name: Set(None),
        member_entity_id: Set(None),
        permissions_before: Set(None),
        permissions_after: Set(None),
        ..Default::default()
    }
}

fn treasury_event(value: &ClaimTreasuryEvent, region: Region) -> Option<claim_event::ActiveModel> {
    let timestamp = DateTime::from_timestamp_micros(value.timestamp.to_micros_since_unix_epoch())?;
    let event_type = match value.reason {
        ClaimTreasuryChangeReason::Deposit => ClaimEventType::TreasuryDeposit,
        ClaimTreasuryChangeReason::Withdraw => ClaimEventType::TreasuryWithdrawal,
    };
    let mut event = new_event(value.claim_entity_id, region, timestamp, event_type);
    event.actor_entity_id =
        Set((value.actor_entity_id != 0).then_some(value.actor_entity_id as i64));
    event.amount = Set(Some(i64::from(value.amount)));
    event.treasury_after = Set(Some(i64::from(value.treasury_after)));
    Some(event)
}

fn completed_research(old: &[i32], new: &[i32]) -> Vec<i32> {
    let mut seen: std::collections::HashSet<i32> = old.iter().copied().collect();
    new.iter().copied().filter(|id| seen.insert(*id)).collect()
}

fn building_event(
    value: &BuildingState,
    region: Region,
    source: &Event<Reducer>,
    placed: bool,
) -> Option<claim_event::ActiveModel> {
    if value.claim_entity_id == 0 || !live_event(source) {
        return None;
    }
    let event_type = if placed {
        ClaimEventType::BuildingPlaced
    } else {
        ClaimEventType::BuildingRemoved
    };
    let mut event = new_event(
        value.claim_entity_id,
        region,
        event_timestamp(source),
        event_type,
    );
    event.building_entity_id = Set(Some(value.entity_id as i64));
    event.building_description_id = Set(Some(value.building_description_id));
    if placed && value.constructed_by_player_entity_id != 0 {
        event.actor_entity_id = Set(Some(value.constructed_by_player_entity_id as i64));
    }
    // The constructor is not necessarily the player who removed the building.
    Some(event)
}

fn member_permissions(value: &ClaimMemberState) -> ClaimMemberPermissions {
    ClaimMemberPermissions {
        inventory_permission: value.inventory_permission,
        build_permission: value.build_permission,
        officer_permission: value.officer_permission,
        co_owner_permission: value.co_owner_permission,
    }
}

fn member_event(
    value: &ClaimMemberState,
    region: Region,
    source: &Event<Reducer>,
    added: bool,
) -> Option<claim_event::ActiveModel> {
    if value.claim_entity_id == 0 || !live_event(source) {
        return None;
    }
    let mut event = new_event(
        value.claim_entity_id,
        region,
        event_timestamp(source),
        if added {
            ClaimEventType::MemberAdded
        } else {
            ClaimEventType::MemberRemoved
        },
    );
    // This is the affected player, not necessarily the player making the change.
    event.member_entity_id = Set(Some(value.player_entity_id as i64));
    event.subject_name = Set(Some(value.user_name.clone()));
    if added {
        event.permissions_after = Set(Some(member_permissions(value)));
    } else {
        event.permissions_before = Set(Some(member_permissions(value)));
    }
    Some(event)
}

fn member_update_events(
    old: &ClaimMemberState,
    new: &ClaimMemberState,
    region: Region,
    source: &Event<Reducer>,
) -> Vec<claim_event::ActiveModel> {
    if !live_event(source) {
        return vec![];
    }
    if old.claim_entity_id != new.claim_entity_id || old.player_entity_id != new.player_entity_id {
        return member_event(old, region, source, false)
            .into_iter()
            .chain(member_event(new, region, source, true))
            .collect();
    }
    let before = member_permissions(old);
    let after = member_permissions(new);
    if new.claim_entity_id == 0 || before == after {
        return vec![];
    }
    let mut event = new_event(
        new.claim_entity_id,
        region,
        event_timestamp(source),
        ClaimEventType::MemberPermissionsChanged,
    );
    event.member_entity_id = Set(Some(new.player_entity_id as i64));
    event.subject_name = Set(Some(new.user_name.clone()));
    event.permissions_before = Set(Some(before));
    event.permissions_after = Set(Some(after));
    vec![event]
}

const CLAIM_EVENT_QUEUE_CAPACITY: usize = 5_000;
const CLAIM_EVENT_BATCH_SIZE: usize = 100;
const CLAIM_EVENT_MAX_ATTEMPTS: usize = 3;

fn queue(tx: &Sender<claim_event::ActiveModel>, event: claim_event::ActiveModel) {
    if let Err(error) = tx.try_send(event) {
        metrics::counter!("claim_events_dropped_total", "reason" => "queue_unavailable")
            .increment(1);
        tracing::error!(%error, "Dropping claim event: persistence queue is full or closed");
    }
}

fn building_event_is_in_main_region(
    event_type: &ClaimEventType,
    event_region: Region,
    claim_region: Option<Region>,
) -> bool {
    match event_type {
        ClaimEventType::BuildingPlaced | ClaimEventType::BuildingRemoved => {
            claim_region == Some(event_region)
        }
        _ => true,
    }
}

async fn filter_building_regions(
    state: &AppState,
    events: &[claim_event::ActiveModel],
) -> Result<Vec<claim_event::ActiveModel>, sea_orm::DbErr> {
    let mut claim_regions = std::collections::HashMap::new();
    let mut missing_claims = std::collections::HashSet::new();
    for event in events {
        if !matches!(
            event.event_type.as_ref(),
            ClaimEventType::BuildingPlaced | ClaimEventType::BuildingRemoved
        ) {
            continue;
        }
        let claim_id = *event.claim_entity_id.as_ref();
        if let Some(claim) = state.claim_state.get(&claim_id) {
            claim_regions.insert(claim_id, claim.region);
        } else {
            missing_claims.insert(claim_id);
        }
    }
    // Resolve claims not yet in the in-memory cache before persisting. In particular,
    // building_state replicas must not determine the region: waypoints appear in all regions.
    if !missing_claims.is_empty() {
        for claim in entity::claim_state::Entity::find()
            .filter(entity::claim_state::Column::EntityId.is_in(missing_claims))
            .all(&state.conn)
            .await?
        {
            claim_regions.insert(claim.entity_id, claim.region);
        }
    }
    Ok(events
        .iter()
        .filter(|event| {
            building_event_is_in_main_region(
                event.event_type.as_ref(),
                *event.region.as_ref(),
                claim_regions.get(event.claim_entity_id.as_ref()).copied(),
            )
        })
        .cloned()
        .collect())
}

/// Start once per region, outside the connection/reconnect lifecycle.
pub(crate) fn start_persistence_worker(
    state: AppState,
    region: Region,
) -> Sender<claim_event::ActiveModel> {
    let (tx, rx) = channel(CLAIM_EVENT_QUEUE_CAPACITY);
    state.shutdown.clone().spawn_worker(async move {
        run_persistence_worker(
            rx,
            region,
            |batch| {
                let state = &state;
                async move {
                    let events = filter_building_regions(state, &batch).await?;
                    if !events.is_empty() {
                        claim_event::Entity::insert_many(events)
                            .exec(&state.conn)
                            .await?;
                    }
                    Ok::<_, sea_orm::DbErr>(())
                }
            },
            Duration::from_secs(5),
            Duration::from_secs(30),
        )
        .await;
    });
    tx
}

async fn run_persistence_worker<F, Fut>(
    mut rx: Receiver<claim_event::ActiveModel>,
    region: Region,
    mut persist: F,
    retry_delay: Duration,
    attempt_timeout: Duration,
) where
    F: FnMut(Vec<claim_event::ActiveModel>) -> Fut,
    Fut: Future<Output = Result<(), sea_orm::DbErr>>,
{
    let mut batch = Vec::with_capacity(CLAIM_EVENT_BATCH_SIZE);
    while rx.recv_many(&mut batch, CLAIM_EVENT_BATCH_SIZE).await != 0 {
        for attempt in 1..=CLAIM_EVENT_MAX_ATTEMPTS {
            match tokio::time::timeout(attempt_timeout, persist(batch.clone())).await {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    tracing::error!(%error, region, attempt, "Could not persist claim event batch");
                }
                Err(_) => {
                    tracing::error!(region, attempt, "Claim event persistence timed out");
                }
            }
            if attempt == CLAIM_EVENT_MAX_ATTEMPTS {
                metrics::counter!("claim_events_dropped_total", "reason" => "persistence_failed")
                    .increment(batch.len() as u64);
                tracing::error!(
                    region,
                    count = batch.len(),
                    "Dropping claim event batch after retry limit"
                );
            } else {
                tokio::time::sleep(retry_delay).await;
            }
        }
        batch.clear();
    }
}

pub(crate) fn register_listeners(
    ctx: &DbConnection,
    state: AppState,
    region: Region,
    tx: Sender<claim_event::ActiveModel>,
) {
    let treasury_tx = tx.clone();
    EventTable::on_insert(&ctx.db.claim_treasury_event(), move |context, value| {
        if live_event(&context.event) {
            if let Some(event) = treasury_event(value, region) {
                queue(&treasury_tx, event);
            }
        }
    });

    let research_tx = tx.clone();
    let research_state = state.clone();
    ctx.db
        .claim_tech_state()
        .on_update(move |context, old, new| {
            if !live_event(&context.event) {
                return;
            }
            let timestamp = event_timestamp(&context.event);
            for id in completed_research(&old.learned, &new.learned) {
                let mut event = new_event(
                    new.entity_id,
                    region,
                    timestamp,
                    ClaimEventType::ResearchCompleted,
                );
                event.research_id = Set(Some(id));
                event.subject_name = Set(research_state
                    .claim_tech_desc
                    .get(&id)
                    .map(|desc| desc.name.clone()));
                queue(&research_tx, event);
            }
        });

    let member_tx = tx.clone();
    ctx.db
        .claim_member_state()
        .on_insert(move |context, value| {
            if let Some(event) = member_event(value, region, &context.event, true) {
                queue(&member_tx, event);
            }
        });
    let member_tx = tx.clone();
    ctx.db
        .claim_member_state()
        .on_delete(move |context, value| {
            if let Some(event) = member_event(value, region, &context.event, false) {
                queue(&member_tx, event);
            }
        });
    let member_tx = tx.clone();
    ctx.db
        .claim_member_state()
        .on_update(move |context, old, new| {
            for event in member_update_events(old, new, region, &context.event) {
                queue(&member_tx, event);
            }
        });

    let building_tx = tx.clone();
    let building_state = state.clone();
    ctx.db.building_state().on_insert(move |context, value| {
        if let Some(mut event) = building_event(value, region, &context.event, true) {
            event.subject_name = Set(building_state
                .building_desc
                .get(&(value.building_description_id as i64))
                .map(|desc| desc.name.clone()));
            queue(&building_tx, event);
        }
    });
    ctx.db.building_state().on_delete(move |context, value| {
        if let Some(mut event) = building_event(value, region, &context.event, false) {
            event.subject_name = Set(state
                .building_desc
                .get(&(value.building_description_id as i64))
                .map(|desc| desc.name.clone()));
            queue(&tx, event);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_groups_cover_each_event_type_once() {
        use sea_orm::Iterable;
        let groups = [
            EventGroup::Treasury,
            EventGroup::Research,
            EventGroup::Buildings,
            EventGroup::Members,
        ];
        let grouped: Vec<_> = groups.iter().flat_map(EventGroup::event_types).collect();
        for event_type in ClaimEventType::iter() {
            assert_eq!(
                grouped.iter().filter(|value| **value == event_type).count(),
                1
            );
        }
        assert_eq!(
            EventGroup::Treasury.event_types(),
            vec![
                ClaimEventType::TreasuryDeposit,
                ClaimEventType::TreasuryWithdrawal
            ]
        );
        assert_eq!(
            EventGroup::Research.event_types(),
            vec![ClaimEventType::ResearchCompleted]
        );
        assert_eq!(
            EventGroup::Buildings.event_types(),
            vec![
                ClaimEventType::BuildingPlaced,
                ClaimEventType::BuildingRemoved
            ]
        );
        assert_eq!(
            EventGroup::Members.event_types(),
            vec![
                ClaimEventType::MemberAdded,
                ClaimEventType::MemberRemoved,
                ClaimEventType::MemberPermissionsChanged
            ]
        );
    }

    #[test]
    fn event_group_query_is_optional_and_validated() {
        assert!(
            serde_json::from_value::<EventParams>(serde_json::json!({}))
                .unwrap()
                .group
                .is_none()
        );
        for group in ["treasury", "research", "buildings", "members"] {
            assert!(
                serde_json::from_value::<EventParams>(serde_json::json!({"group": group}))
                    .unwrap()
                    .group
                    .is_some()
            );
        }
        assert!(
            serde_json::from_value::<EventParams>(serde_json::json!({"group": "unknown"})).is_err()
        );
    }

    fn persistence_event(claim_id: u64) -> claim_event::ActiveModel {
        new_event(claim_id, 2, Utc::now(), ClaimEventType::MemberAdded)
    }

    #[test]
    fn persistence_queue_is_bounded_and_keeps_existing_events_on_overflow() {
        let (tx, mut rx) = channel(CLAIM_EVENT_QUEUE_CAPACITY);
        for _ in 0..CLAIM_EVENT_QUEUE_CAPACITY {
            queue(&tx, persistence_event(42));
        }
        queue(&tx, persistence_event(43));
        assert_eq!(rx.len(), CLAIM_EVENT_QUEUE_CAPACITY);
        while let Ok(event) = rx.try_recv() {
            assert_eq!(event.claim_entity_id, Set(42));
        }
    }

    #[tokio::test]
    async fn failing_batch_is_dropped_and_next_batch_is_processed() {
        let (tx, rx) = channel(CLAIM_EVENT_BATCH_SIZE + 1);
        for _ in 0..CLAIM_EVENT_BATCH_SIZE {
            queue(&tx, persistence_event(42));
        }
        // A replacement connection uses the same queue, not a new worker.
        let replacement_tx = tx.clone();
        drop(tx);
        queue(&replacement_tx, persistence_event(43));
        drop(replacement_tx);

        let mut attempts = 0;
        let mut persisted = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(1),
            run_persistence_worker(
                rx,
                2,
                |batch| {
                    attempts += 1;
                    let result = if batch[0].claim_entity_id == Set(42) {
                        Err(sea_orm::DbErr::Custom("permanent failure".to_string()))
                    } else {
                        persisted.extend(batch);
                        Ok(())
                    };
                    std::future::ready(result)
                },
                Duration::ZERO,
                Duration::from_secs(1),
            ),
        )
        .await
        .expect("worker must drain and exit after all connections drop their senders");

        assert_eq!(attempts, CLAIM_EVENT_MAX_ATTEMPTS + 1);
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].claim_entity_id, Set(43));
    }

    #[tokio::test]
    async fn stalled_persistence_has_bounded_attempts_and_worker_exits() {
        let (tx, rx) = channel(1);
        queue(&tx, persistence_event(42));
        drop(tx);
        let mut attempts = 0;
        tokio::time::timeout(
            Duration::from_secs(1),
            run_persistence_worker(
                rx,
                2,
                |_| {
                    attempts += 1;
                    std::future::pending::<Result<(), sea_orm::DbErr>>()
                },
                Duration::ZERO,
                Duration::from_millis(1),
            ),
        )
        .await
        .expect("stalled database must not keep a worker alive indefinitely");
        assert_eq!(attempts, CLAIM_EVENT_MAX_ATTEMPTS);
    }

    #[tokio::test]
    async fn transient_persistence_failure_is_retried() {
        let (tx, rx) = channel(1);
        queue(&tx, persistence_event(42));
        drop(tx);
        let mut attempts = 0;
        run_persistence_worker(
            rx,
            2,
            |_| {
                attempts += 1;
                std::future::ready(if attempts == 1 {
                    Err(sea_orm::DbErr::Custom("temporary failure".to_string()))
                } else {
                    Ok(())
                })
            },
            Duration::ZERO,
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(attempts, 2);
    }

    fn member() -> ClaimMemberState {
        ClaimMemberState {
            entity_id: 10,
            claim_entity_id: 42,
            player_entity_id: 7,
            user_name: "Member".into(),
            inventory_permission: true,
            build_permission: true,
            officer_permission: false,
            co_owner_permission: false,
        }
    }

    #[test]
    fn member_events_record_affected_player_and_permissions() {
        let value = member();
        let added = member_event(&value, 2, &Event::Transaction, true).unwrap();
        assert_eq!(added.event_type, Set(ClaimEventType::MemberAdded));
        assert_eq!(added.claim_entity_id, Set(42));
        assert_eq!(added.region, Set(2));
        assert_eq!(added.member_entity_id, Set(Some(7)));
        assert_eq!(added.actor_entity_id, Set(None));
        assert_eq!(added.subject_name, Set(Some("Member".into())));
        assert_eq!(added.permissions_before, Set(None));
        assert_eq!(
            added.permissions_after,
            Set(Some(member_permissions(&value)))
        );
        let removed = member_event(&value, 2, &Event::Transaction, false).unwrap();
        assert_eq!(removed.event_type, Set(ClaimEventType::MemberRemoved));
        assert_eq!(
            removed.permissions_before,
            Set(Some(member_permissions(&value)))
        );
        assert_eq!(removed.permissions_after, Set(None));
    }

    #[test]
    fn each_member_permission_change_records_before_and_after() {
        let old = member();
        for permission in 0..4 {
            let mut new = old.clone();
            match permission {
                0 => new.inventory_permission = !old.inventory_permission,
                1 => new.build_permission = !old.build_permission,
                2 => new.officer_permission = !old.officer_permission,
                _ => new.co_owner_permission = !old.co_owner_permission,
            }
            let events = member_update_events(&old, &new, 2, &Event::Transaction);
            assert_eq!(events.len(), 1);
            assert_eq!(
                events[0].event_type,
                Set(ClaimEventType::MemberPermissionsChanged)
            );
            assert_eq!(events[0].member_entity_id, Set(Some(7)));
            assert_eq!(
                events[0].permissions_before,
                Set(Some(member_permissions(&old)))
            );
            assert_eq!(
                events[0].permissions_after,
                Set(Some(member_permissions(&new)))
            );
        }
    }

    #[test]
    fn member_events_ignore_sync_disconnect_and_name_only_changes() {
        let old = member();
        let mut new = old.clone();
        new.user_name = "Renamed".into();
        assert!(member_update_events(&old, &new, 2, &Event::Transaction).is_empty());
        new.officer_permission = true;
        for source in [
            Event::SubscribeApplied,
            Event::UnsubscribeApplied,
            Event::Disconnected,
        ] {
            assert!(member_event(&old, 2, &source, true).is_none());
            assert!(member_event(&old, 2, &source, false).is_none());
            assert!(member_update_events(&old, &new, 2, &source).is_empty());
        }
        new.claim_entity_id = 0;
        assert!(member_event(&new, 2, &Event::Transaction, true).is_none());
    }

    #[test]
    fn moving_a_member_logs_removal_and_addition() {
        let old = member();
        let mut new = old.clone();
        new.claim_entity_id = 43;
        let events = member_update_events(&old, &new, 2, &Event::Transaction);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, Set(ClaimEventType::MemberRemoved));
        assert_eq!(events[0].claim_entity_id, Set(42));
        assert_eq!(events[1].event_type, Set(ClaimEventType::MemberAdded));
        assert_eq!(events[1].claim_entity_id, Set(43));
    }

    #[test]
    fn replicated_building_events_only_persist_in_the_claim_main_region() {
        for event_type in [
            ClaimEventType::BuildingPlaced,
            ClaimEventType::BuildingRemoved,
        ] {
            assert!(building_event_is_in_main_region(&event_type, 2, Some(2)));
            assert!(!building_event_is_in_main_region(&event_type, 1, Some(2)));
            assert!(!building_event_is_in_main_region(&event_type, 3, Some(2)));
            assert!(!building_event_is_in_main_region(&event_type, 2, None));
        }
    }

    #[test]
    fn building_region_filter_does_not_affect_treasury_or_research() {
        for event_type in [
            ClaimEventType::TreasuryDeposit,
            ClaimEventType::TreasuryWithdrawal,
            ClaimEventType::ResearchCompleted,
        ] {
            assert!(building_event_is_in_main_region(&event_type, 1, Some(2)));
            assert!(building_event_is_in_main_region(&event_type, 1, None));
        }
    }

    #[test]
    fn only_newly_learned_research_is_logged() {
        assert_eq!(completed_research(&[1, 2], &[1, 2, 3, 3, 4]), vec![3, 4]);
        assert!(completed_research(&[1, 2], &[2, 1]).is_empty());
        assert!(completed_research(&[1, 2], &[1]).is_empty());
    }

    #[test]
    fn initial_sync_and_disconnect_are_not_live_events() {
        assert!(!live_event(&Event::SubscribeApplied));
        assert!(!live_event(&Event::UnsubscribeApplied));
        assert!(!live_event(&Event::Disconnected));
        assert!(live_event(&Event::Transaction));
    }

    #[test]
    fn building_events_only_record_live_changes_in_claims() {
        let mut value = BuildingState {
            entity_id: 10,
            claim_entity_id: 42,
            direction_index: 0,
            building_description_id: 5,
            constructed_by_player_entity_id: 7,
        };
        let placed = building_event(&value, 2, &Event::Transaction, true).unwrap();
        assert_eq!(placed.event_type, Set(ClaimEventType::BuildingPlaced));
        assert_eq!(placed.claim_entity_id, Set(42));
        assert_eq!(placed.building_entity_id, Set(Some(10)));
        assert_eq!(placed.building_description_id, Set(Some(5)));
        assert_eq!(placed.actor_entity_id, Set(Some(7)));

        let removed = building_event(&value, 2, &Event::Transaction, false).unwrap();
        assert_eq!(removed.event_type, Set(ClaimEventType::BuildingRemoved));
        assert_eq!(removed.actor_entity_id, Set(None));
        assert!(building_event(&value, 2, &Event::SubscribeApplied, true).is_none());
        assert!(building_event(&value, 2, &Event::Disconnected, false).is_none());
        value.claim_entity_id = 0;
        assert!(building_event(&value, 2, &Event::Transaction, true).is_none());
    }

    #[test]
    fn treasury_events_keep_reason_amount_actor_and_balance() {
        for (reason, expected) in [
            (
                ClaimTreasuryChangeReason::Deposit,
                ClaimEventType::TreasuryDeposit,
            ),
            (
                ClaimTreasuryChangeReason::Withdraw,
                ClaimEventType::TreasuryWithdrawal,
            ),
        ] {
            let value = ClaimTreasuryEvent {
                claim_entity_id: 42,
                actor_entity_id: 7,
                reason,
                amount: 1,
                treasury_after: 100,
                timestamp: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(123456),
            };
            let event = treasury_event(&value, 2).unwrap();
            assert_eq!(event.event_type, Set(expected));
            assert_eq!(event.amount, Set(Some(1)));
            assert_eq!(event.actor_entity_id, Set(Some(7)));
            assert_eq!(event.treasury_after, Set(Some(100)));
            assert_eq!(event.region, Set(2));
        }
    }
}
