use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use entity::claim_event::{self, ClaimEventType};
use entity::shared::Region;
use game_module::module_bindings::{
    BuildingState, BuildingStateTableAccess, ClaimTechStateTableAccess, ClaimTreasuryChangeReason,
    ClaimTreasuryEvent, ClaimTreasuryEventTableAccess, DbConnection, Reducer,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Event, EventTable, Table, TableWithPrimaryKey};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use ts_rs::TS;

#[derive(Deserialize)]
pub(crate) struct EventParams {
    page: Option<u64>,
    per_page: Option<u64>,
    region: Option<Region>,
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

fn queue(tx: &UnboundedSender<claim_event::ActiveModel>, event: claim_event::ActiveModel) {
    if let Err(error) = tx.send(event) {
        tracing::error!(%error, "Could not queue claim event");
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

pub(crate) fn register_listeners(ctx: &DbConnection, state: AppState, region: Region) {
    let (tx, mut rx) = unbounded_channel::<claim_event::ActiveModel>();
    let conn = state.conn.clone();
    let persistence_state = state.clone();
    tokio::spawn(async move {
        let mut batch = Vec::with_capacity(100);
        while rx.recv_many(&mut batch, 100).await != 0 {
            loop {
                let result = async {
                    let events = filter_building_regions(&persistence_state, &batch).await?;
                    if !events.is_empty() {
                        claim_event::Entity::insert_many(events).exec(&conn).await?;
                    }
                    Ok::<_, sea_orm::DbErr>(())
                }
                .await;
                match result {
                    Ok(_) => break,
                    Err(error) => {
                        tracing::error!(%error, region, "Could not persist claim events; retrying");
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
            batch.clear();
        }
    });

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
