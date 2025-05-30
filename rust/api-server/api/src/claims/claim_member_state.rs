use crate::AppState;
use crate::websocket::WebSocketMessages;
use entity::claim_member_state;
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QuerySelect};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

fn get_on_conflict() -> OnConflict {
    sea_query::OnConflict::column(claim_member_state::Column::EntityId)
        .update_columns([
            claim_member_state::Column::ClaimEntityId,
            claim_member_state::Column::PlayerEntityId,
            claim_member_state::Column::UserName,
            claim_member_state::Column::InventoryPermission,
            claim_member_state::Column::BuildPermission,
            claim_member_state::Column::OfficerPermission,
            claim_member_state::Column::CoOwnerPermission,
        ])
        .to_owned()
}

async fn known_claim_member_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_claim_ids: Vec<i64> = claim_member_state::Entity::find()
        .select_only()
        .column(claim_member_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_claim_ids = known_claim_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_claim_ids)
}

async fn db_insert_claim_member_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<claim_member_state::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let claim_from_db = claim_member_state::Entity::find()
        .filter(
            claim_member_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|claim| claim.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let claim_from_db_map = claim_from_db
        .into_iter()
        .map(|claim| (claim.entity_id, claim))
        .collect::<HashMap<i64, claim_member_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|claim| match claim_from_db_map.get(&claim.entity_id) {
            Some(claim_from_db) => claim_from_db != *claim,
            None => true,
        })
        .map(|claim| claim.clone().into_active_model())
        .collect::<Vec<claim_member_state::ActiveModel>>();

    if things_to_insert.is_empty() {
        tracing::debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        tracing::debug!("Inserting {} claim_member_state", things_to_insert.len());
    }

    let _ = claim_member_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_claim_member_state(
    conn: &DatabaseConnection,
    known_claim_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    tracing::info!(
        "claim_member_state's ({}) to delete: {:?}",
        known_claim_ids.len(),
        known_claim_ids,
    );
    claim_member_state::Entity::delete_many()
        .filter(claim_member_state::Column::EntityId.is_in(known_claim_ids.clone()))
        .exec(conn)
        .await?;

    Ok(())
}
//
// pub(crate) async fn handle_initial_subscription(
//     app_state: &Arc<AppState>,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let on_conflict = get_on_conflict();
//
//     let chunk_size = 5000;
//     let mut buffer_before_insert: Vec<claim_member_state::Model> = vec![];
//
//     let mut known_inventory_ids = known_claim_member_state_ids(&app_state.conn).await?;
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<claim_member_state::Model>(row.as_ref()) {
//                 Ok(claim_member_state) => {
//                     if known_inventory_ids.contains(&claim_member_state.entity_id) {
//                         known_inventory_ids.remove(&claim_member_state.entity_id);
//                     }
//                     buffer_before_insert.push(claim_member_state.clone());
//                     app_state.add_claim_member(claim_member_state.clone());
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_claim_member_state(
//                             &app_state.conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                         )
//                             .await?;
//                     }
//                 }
//                 Err(error) => {
//                     tracing::error!(json = row, "InitialSubscription Insert ClaimState Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         for buffer_chnk in buffer_before_insert.chunks(5000) {
//             db_insert_claim_member_state(
//                 &app_state.conn,
//                 &mut buffer_chnk.to_vec(),
//                 &on_conflict,
//             )
//                 .await?;
//         }
//     }
//
//     if !known_inventory_ids.is_empty() {
//         delete_claim_member_state(&app_state.conn, known_inventory_ids).await?;
//     }
//
//     Ok(())
// }
//
// pub(crate) async fn handle_transaction_update(
//     app_state: &Arc<AppState>,
//     tables: &[TableWithOriginalEventTransactionUpdate],
//     sender: UnboundedSender<WebSocketMessages>,
// ) -> anyhow::Result<()> {
//     let on_conflict = get_on_conflict();
//
//     let mut buffer_before_insert = HashMap::new();
//     let mut potential_deletes = HashMap::new();
//
//     for p1 in tables.iter() {
//         let event_type = if !p1.inserts.is_empty() && !p1.deletes.is_empty() {
//             "update"
//         } else if !p1.inserts.is_empty() && p1.deletes.is_empty() {
//             "insert"
//         } else if !p1.deletes.is_empty() && p1.inserts.is_empty() {
//             "delete"
//         } else {
//             "unknown"
//         };
//
//         if event_type == "unknown" {
//             tracing::error!("Unknown event type {:?}", p1);
//             continue;
//         }
//
//         if event_type == "delete" {
//             for row in p1.deletes.iter() {
//                 match serde_json::from_str::<claim_member_state::Model>(row.as_ref()) {
//                     Ok(claim_member_state) => {
//                         potential_deletes.insert(claim_member_state.entity_id, claim_member_state.clone());
//                     }
//                     Err(error) => {
//                         tracing::error!(json = row, "Event: {event_type} Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         } else if event_type == "update" {
//             let mut delete_parsed = HashMap::new();
//             for row in p1.deletes.iter() {
//                 let parsed = serde_json::from_str::<claim_member_state::Model>(row.as_ref());
//
//                 if parsed.is_err() {
//                     tracing::error!(
//                         "Could not parse delete claim_member_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row
//                     );
//                 } else {
//                     let parsed = parsed.unwrap();
//                     delete_parsed.insert(parsed.entity_id, parsed.clone());
//                     potential_deletes.remove(&parsed.entity_id);
//                 }
//             }
//
//             for row in p1.inserts.iter().enumerate() {
//                 let parsed = serde_json::from_str::<claim_member_state::Model>(row.1.as_ref());
//
//                 if parsed.is_err() {
//                     tracing::error!(
//                         "Could not parse insert claim_member_state: {}, row: {:?}",
//                         parsed.unwrap_err(),
//                         row.1
//                     );
//                     continue;
//                 }
//
//                 let parsed = parsed.unwrap();
//                 let id = parsed.entity_id;
//
//                 match (parsed, delete_parsed.get(&id)) {
//                     (new_claim_member_state, Some(_old_claim_member_state)) => {
//                         app_state.add_claim_member(
//                             new_claim_member_state.clone(),
//                         );
//                         buffer_before_insert.insert(
//                             new_claim_member_state.entity_id,
//                             new_claim_member_state.clone(),
//                         );
//                         potential_deletes.remove(&new_claim_member_state.entity_id);
//                     }
//                     (_new_claim_member_state, None) => {
//                         tracing::error!("Could not find delete state new experience state",);
//                     }
//                 }
//             }
//         } else if event_type == "insert" {
//             for row in p1.inserts.iter() {
//                 match serde_json::from_str::<claim_member_state::Model>(row.as_ref()) {
//                     Ok(claim_member_state) => {
//                         app_state.add_claim_member(
//                             claim_member_state.clone(),
//                         );
//                         buffer_before_insert.insert(
//                             claim_member_state.entity_id,
//                             claim_member_state.clone(),
//                         );
//
//                         // sender
//                         //     .send(WebSocketMessages::ClaimState(
//                         //         claim_member_state.clone(),
//                         //     ))
//                         //     .unwrap();
//                     }
//                     Err(error) => {
//                         tracing::error!("Error: {error} for row: {:?}", row);
//                     }
//                 }
//             }
//         } else {
//             tracing::error!("Unknown event type {:?}", p1);
//             continue;
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<claim_member_state::Model>>();
//         db_insert_claim_member_state(
//             &app_state.conn,
//             &mut buffer_before_insert_vec,
//             &on_conflict,
//         )
//         .await?;
//         buffer_before_insert.clear();
//     }
//
//     if !potential_deletes.is_empty() {
//         let mut potential_deletes_set = HashSet::new();
//         for (id, model) in potential_deletes.into_iter() {
//             app_state
//                 .remove_claim_member(model);
//
//             potential_deletes_set.insert(id);
//         }
//
//         delete_claim_member_state(&app_state.conn, potential_deletes_set).await?;
//     }
//
//     Ok(())
// }
