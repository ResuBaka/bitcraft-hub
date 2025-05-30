use crate::AppState;
// use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use entity::claim_tech_state::Model;
use entity::{claim_tech_desc, claim_tech_state};
use log::{debug, error, info};
use migration::{OnConflict, sea_query};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::sync::Arc;

#[allow(dead_code)]
pub(crate) async fn load_claim_tech_state_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<claim_tech_desc::Model>> {
    let item_file = File::open(storage_path.join("State/ClaimTechState.json"))?;
    let claim_tech_state: Value = serde_json::from_reader(&item_file)?;
    let claim_tech_states: Vec<claim_tech_desc::Model> = serde_json::from_value(
        claim_tech_state
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )?;

    Ok(claim_tech_states)
}

async fn delete_claim_tech_state(
    conn: &DatabaseConnection,
    known_claim_tech_state_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "claim_tech_desc's ({}) to delete: {:?}",
        known_claim_tech_state_ids.len(),
        known_claim_tech_state_ids,
    );
    claim_tech_state::Entity::delete_many()
        .filter(claim_tech_state::Column::EntityId.is_in(known_claim_tech_state_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_claim_tech_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let claim_tech_state_from_db = claim_tech_state::Entity::find()
        .filter(
            claim_tech_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|claim_tech_state| claim_tech_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let claim_tech_state_from_db_map = claim_tech_state_from_db
        .into_iter()
        .map(|claim_tech_state| (claim_tech_state.entity_id, claim_tech_state))
        .collect::<HashMap<i64, claim_tech_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|claim_tech_state| {
            match claim_tech_state_from_db_map.get(&claim_tech_state.entity_id) {
                Some(claim_tech_state_from_db) => claim_tech_state_from_db != *claim_tech_state,
                None => true,
            }
        })
        .map(|claim_tech_state| claim_tech_state.clone().into_active_model())
        .collect::<Vec<claim_tech_state::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} claim_tech_desc", things_to_insert.len());
    }

    let _ = claim_tech_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();

    Ok(())
}

fn get_claim_tech_state_on_conflict() -> OnConflict {
    let on_conflict = sea_query::OnConflict::column(claim_tech_state::Column::EntityId)
        .update_columns([
            claim_tech_state::Column::Learned,
            claim_tech_state::Column::Researching,
            claim_tech_state::Column::StartTimestamp,
            claim_tech_state::Column::ScheduledId,
        ])
        .to_owned();
    on_conflict
}

async fn get_known_claim_tech_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_claim_tech_state_ids: Vec<i64> = claim_tech_state::Entity::find()
        .select_only()
        .column(claim_tech_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_claim_tech_state_ids = known_claim_tech_state_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_claim_tech_state_ids)
}
//
// pub(crate) async fn handle_initial_subscription(
//     app_state: &Arc<AppState>,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let on_conflict = get_claim_tech_state_on_conflict();
//
//     let chunk_size = 5000;
//     let mut buffer_before_insert: Vec<claim_tech_state::Model> = vec![];
//
//     let mut known_building_state_ids = get_known_claim_tech_state_ids(&app_state.conn).await?;
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<claim_tech_state::Model>(row.as_ref()) {
//                 Ok(building_state) => {
//                     if known_building_state_ids.contains(&building_state.entity_id) {
//                         known_building_state_ids.remove(&building_state.entity_id);
//                     }
//                     buffer_before_insert.push(building_state);
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_claim_tech_state(
//                             &app_state.conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                         )
//                         .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!("InitialSubscription Insert ClaimTechState Error: {}", error);
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         for buffer_chnk in buffer_before_insert.chunks(5000) {
//             db_insert_claim_tech_state(&app_state.conn, &mut buffer_chnk.to_vec(), &on_conflict)
//                 .await?;
//         }
//     }
//
//     if !known_building_state_ids.is_empty() {
//         delete_claim_tech_state(&app_state.conn, known_building_state_ids).await?;
//     }
//
//     Ok(())
// }
//
// pub(crate) async fn handle_transaction_update(
//     app_state: &Arc<AppState>,
//     tables: &[TableWithOriginalEventTransactionUpdate],
// ) -> anyhow::Result<()> {
//     let on_conflict = get_claim_tech_state_on_conflict();
//
//     let mut found_in_inserts = HashSet::new();
//
//     // let mut known_player_username_state_ids = get_known_player_uusername_state_ids(p0).await?;
//     for p1 in tables.iter() {
//         for row in p1.inserts.iter() {
//             match serde_json::from_str::<claim_tech_state::Model>(row.as_ref()) {
//                 Ok(building_state) => {
//                     let current_building_state = QueryCore::find_claim_tech_state_by_ids(
//                         &app_state.conn,
//                         vec![building_state.entity_id],
//                     )
//                     .await?;
//
//                     if !current_building_state.is_empty() {
//                         let current_building_state = current_building_state.first().unwrap();
//                         if current_building_state != &building_state {
//                             found_in_inserts.insert(building_state.entity_id);
//                             let _ = claim_tech_state::Entity::insert(
//                                 building_state.clone().into_active_model(),
//                             )
//                             .on_conflict(on_conflict.clone())
//                             .exec(&app_state.conn)
//                             .await?;
//                         }
//                     } else {
//                         found_in_inserts.insert(building_state.entity_id);
//                         let _ = claim_tech_state::Entity::insert(
//                             building_state.clone().into_active_model(),
//                         )
//                         .exec(&app_state.conn)
//                         .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Insert ClaimTechState Error: {}", error);
//                 }
//             }
//         }
//     }
//
//     let mut ids_to_delete = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.deletes.iter() {
//             match serde_json::from_str::<claim_tech_state::Model>(row.as_ref()) {
//                 Ok(building_state) => {
//                     if found_in_inserts.contains(&building_state.entity_id) {
//                         continue;
//                     }
//
//                     ids_to_delete.insert(building_state.entity_id);
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Delete ClaimTechState Error: {}", error);
//                 }
//             }
//         }
//     }
//
//     if !ids_to_delete.is_empty() {
//         delete_claim_tech_state(&app_state.conn, ids_to_delete).await?;
//     }
//
//     Ok(())
// }
