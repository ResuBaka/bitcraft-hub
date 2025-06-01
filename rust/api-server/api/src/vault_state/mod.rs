// // use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
// use entity::vault_state::RawVaultState;
// use entity::{vault_state, vault_state_collectibles};
// use log::{debug, info};
// use migration::OnConflict;
// use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
// use sea_orm::{DbErr, IntoActiveModel};
// use std::collections::{HashMap, HashSet};

// async fn get_known_vault_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
//     let known_vault_state_ids: Vec<i64> = vault_state::Entity::find()
//         .select_only()
//         .column(vault_state::Column::EntityId)
//         .into_tuple()
//         .all(conn)
//         .await?;
//
//     let known_vault_state_ids = known_vault_state_ids.into_iter().collect::<HashSet<i64>>();
//     Ok(known_vault_state_ids)
// }
//
// async fn db_insert_vault_states(
//     conn: &DatabaseConnection,
//     buffer_before_insert: &mut Vec<RawVaultState>,
//     on_conflict: &OnConflict,
//     vault_state_collectible_on_conflict: &OnConflict,
//     list_of_vault_state_collectibles_to_delete: &mut Option<&mut HashSet<(i64, i32)>>,
// ) -> anyhow::Result<()> {
//     let (vault_state_ids, vault_state_models, vault_state_collectibles_models) =
//         buffer_before_insert
//             .iter()
//             .map(|vault_state| {
//                 let vault_state_collectibles = vault_state.to_model_collectibles();
//                 let vault_state_model = vault_state.to_model();
//
//                 (
//                     vault_state.entity_id,
//                     vault_state_model,
//                     vault_state_collectibles,
//                 )
//             })
//             .collect::<(
//                 Vec<i64>,
//                 Vec<vault_state::Model>,
//                 Vec<Vec<vault_state_collectibles::Model>>,
//             )>();
//
//     let vault_states_from_db = vault_state::Entity::find()
//         .filter(vault_state::Column::EntityId.is_in(vault_state_ids.clone()))
//         .all(conn);
//
//     let vault_states_collectibles_from_db = vault_state_collectibles::Entity::find()
//         .filter(vault_state_collectibles::Column::EntityId.is_in(vault_state_ids.clone()))
//         .all(conn);
//
//     let (vault_states_from_db, vault_states_collectibles_from_db) =
//         tokio::join!(vault_states_from_db, vault_states_collectibles_from_db,);
//
//     let vault_states_from_db = vault_states_from_db?;
//     let vault_states_collectibles_from_db = vault_states_collectibles_from_db?;
//
//     let vault_states_from_db_map = vault_states_from_db
//         .into_iter()
//         .map(|vault_state| (vault_state.entity_id, vault_state))
//         .collect::<HashMap<i64, vault_state::Model>>();
//
//     let vault_states_collectibles_from_db_map = vault_states_collectibles_from_db
//         .into_iter()
//         .map(|vault_state_collectibles| {
//             (
//                 (
//                     vault_state_collectibles.entity_id,
//                     vault_state_collectibles.id,
//                 ),
//                 vault_state_collectibles,
//             )
//         })
//         .collect::<HashMap<(i64, i32), vault_state_collectibles::Model>>();
//
//     let things_to_insert = vault_state_models
//         .into_iter()
//         .filter(
//             |vault_state| match vault_states_from_db_map.get(&vault_state.entity_id) {
//                 Some(vault_state_from_db) => vault_state_from_db != vault_state,
//                 None => true,
//             },
//         )
//         .map(|vault_state| vault_state.into_active_model())
//         .collect::<Vec<vault_state::ActiveModel>>();
//
//     let mut things_to_insert_collectibles = vault_state_collectibles_models
//         .into_iter()
//         .flatten()
//         .filter(|vault_state| {
//             if let Some(list_of_vault_state_collectibles_to_delete) =
//                 list_of_vault_state_collectibles_to_delete
//             {
//                 list_of_vault_state_collectibles_to_delete
//                     .remove(&(vault_state.entity_id, vault_state.id));
//             }
//             match vault_states_collectibles_from_db_map
//                 .get(&(vault_state.entity_id, vault_state.id))
//             {
//                 Some(vault_state_from_db) => vault_state_from_db != vault_state,
//                 None => true,
//             }
//         })
//         .map(|vault_state| vault_state.into_active_model())
//         .collect::<Vec<vault_state_collectibles::ActiveModel>>();
//
//     if things_to_insert.is_empty() {
//         debug!("Nothing to insert");
//         buffer_before_insert.clear();
//     } else {
//         let _ = vault_state::Entity::insert_many(things_to_insert)
//             .on_conflict(on_conflict.clone())
//             .exec(conn)
//             .await?;
//
//         buffer_before_insert.clear();
//     }
//
//     if things_to_insert_collectibles.is_empty() {
//         debug!("Nothing to insert");
//         things_to_insert_collectibles.clear();
//     } else {
//         for things_to_insert_chunk in things_to_insert_collectibles.chunks(500) {
//             let _ = vault_state_collectibles::Entity::insert_many(things_to_insert_chunk.to_vec())
//                 .on_conflict(vault_state_collectible_on_conflict.clone())
//                 .exec(conn)
//                 .await?;
//         }
//         things_to_insert_collectibles.clear();
//     }
//
//     Ok(())
// }
//
// async fn delete_vault_state(
//     conn: &DatabaseConnection,
//     known_vault_state_ids: HashSet<i64>,
//     cross_delete: bool,
// ) -> anyhow::Result<()> {
//     info!(
//         "vault_state's ({}) to delete: {:?}",
//         known_vault_state_ids.len(),
//         known_vault_state_ids
//     );
//     vault_state::Entity::delete_many()
//         .filter(vault_state::Column::EntityId.is_in(known_vault_state_ids.clone()))
//         .exec(conn)
//         .await?;
//
//     if cross_delete {
//         vault_state_collectibles::Entity::delete_many()
//             .filter(vault_state_collectibles::Column::EntityId.is_in(known_vault_state_ids))
//             .exec(conn)
//             .await?;
//     }
//     Ok(())
// }
//
// pub(crate) async fn handle_initial_subscription(
//     conn: &DatabaseConnection,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let chunk_size = 500;
//     let mut buffer_before_insert: Vec<RawVaultState> = Vec::with_capacity(chunk_size);
//
//     let on_conflict = sea_query::OnConflict::column(vault_state::Column::EntityId)
//         .update_columns([vault_state::Column::Shards])
//         .to_owned();
//
//     let vault_state_collectible_on_conflict = sea_query::OnConflict::columns([
//         vault_state_collectibles::Column::EntityId,
//         vault_state_collectibles::Column::Id,
//     ])
//     .update_columns([
//         vault_state_collectibles::Column::Activated,
//         vault_state_collectibles::Column::Count,
//     ])
//     .to_owned();
//
//     let (known_vault_state_ids, known_vault_state_collectibles_ids) = tokio::join!(
//         get_known_vault_state_ids(conn),
//         get_known_vault_state_collectibles_ids(conn),
//     );
//
//     let mut known_vault_state_ids = known_vault_state_ids?;
//
//     let mut known_vault_state_collectibles_ids = known_vault_state_collectibles_ids?
//         .into_iter()
//         .collect::<HashSet<(i64, i32)>>();
//
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<RawVaultState>(row.as_ref()) {
//                 Ok(vault_state) => {
//                     if known_vault_state_ids.contains(&vault_state.entity_id) {
//                         known_vault_state_ids.remove(&vault_state.entity_id);
//                     }
//                     buffer_before_insert.push(vault_state);
//                     if buffer_before_insert.len() == chunk_size {
//                         db_insert_vault_states(
//                             conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                             &vault_state_collectible_on_conflict,
//                             &mut Some(&mut known_vault_state_collectibles_ids),
//                         )
//                         .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!(
//                         "TransactionUpdate Insert RawVaultState Error: {error} -> {:?}",
//                         row
//                     );
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         db_insert_vault_states(
//             conn,
//             &mut buffer_before_insert,
//             &on_conflict,
//             &vault_state_collectible_on_conflict,
//             &mut Some(&mut known_vault_state_collectibles_ids),
//         )
//         .await?;
//     }
//
//     if !known_vault_state_ids.is_empty() {
//         delete_vault_state(conn, known_vault_state_ids, false).await?;
//     }
//
//     if !known_vault_state_collectibles_ids.is_empty() {
//         delete_vault_state_collectibles(conn, known_vault_state_collectibles_ids).await?;
//     }
//
//     Ok(())
// }
//
// fn get_known_vault_state_collectibles_ids(
//     conn: &DatabaseConnection,
// ) -> impl Future<Output = Result<Vec<(i64, i32)>, DbErr>> + Sized {
//     vault_state_collectibles::Entity::find()
//         .select_only()
//         .column(vault_state_collectibles::Column::EntityId)
//         .column(vault_state_collectibles::Column::Id)
//         .into_tuple::<(i64, i32)>()
//         .all(conn)
// }
//
// async fn delete_vault_state_collectibles(
//     conn: &DatabaseConnection,
//     p1: HashSet<(i64, i32)>,
// ) -> anyhow::Result<()> {
//     let to_chunk = p1.iter().clone().collect::<Vec<_>>();
//
//     for chunk in to_chunk.chunks(3000) {
//         let filter_to_process = chunk
//             .iter()
//             .map(|(entity_id, id)| {
//                 vault_state_collectibles::Column::EntityId
//                     .eq(*entity_id)
//                     .and(vault_state_collectibles::Column::Id.eq(*id))
//             })
//             .collect::<Vec<_>>();
//
//         let mut build_filter = filter_to_process.first().unwrap().clone();
//
//         for filter in filter_to_process.iter().skip(1) {
//             build_filter = build_filter.or(filter.clone());
//         }
//
//         info!(
//             "vault_state_collectibles's ({}) to delete: {:?}",
//             chunk.len(),
//             p1
//         );
//
//         vault_state_collectibles::Entity::delete_many()
//             .filter(build_filter)
//             .exec(conn)
//             .await?;
//     }
//
//     Ok(())
// }

// pub(crate) async fn handle_transaction_update(
//     conn: &DatabaseConnection,
//     tables: &[TableWithOriginalEventTransactionUpdate],
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::column(vault_state::Column::EntityId)
//         .update_columns([vault_state::Column::Shards])
//         .to_owned();
//
//     let vault_state_collectible_on_conflict = sea_query::OnConflict::columns([
//         vault_state_collectibles::Column::EntityId,
//         vault_state_collectibles::Column::Id,
//     ])
//     .update_columns([
//         vault_state_collectibles::Column::Activated,
//         vault_state_collectibles::Column::Count,
//     ])
//     .to_owned();
//
//     let mut buffer_before_insert = HashMap::new();
//
//     let mut found_in_inserts = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.inserts.iter() {
//             match serde_json::from_str::<RawVaultState>(row.as_ref()) {
//                 Ok(vault_state) => {
//                     found_in_inserts.insert(vault_state.entity_id);
//                     buffer_before_insert.insert(vault_state.entity_id, vault_state);
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Insert PlayerState Error: {error}");
//                 }
//             }
//         }
//     }
//
//     let mut known_vault_state_collectibles_ids = vault_state_collectibles::Entity::find()
//         .select_only()
//         .filter(vault_state_collectibles::Column::EntityId.is_in(found_in_inserts.clone()))
//         .column(vault_state_collectibles::Column::EntityId)
//         .column(vault_state_collectibles::Column::Id)
//         .into_tuple::<(i64, i32)>()
//         .all(conn)
//         .await?
//         .into_iter()
//         .collect::<HashSet<(i64, i32)>>();
//
//     if !buffer_before_insert.is_empty() {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<RawVaultState>>();
//
//         db_insert_vault_states(
//             conn,
//             &mut buffer_before_insert_vec,
//             &on_conflict,
//             &vault_state_collectible_on_conflict,
//             &mut Some(&mut known_vault_state_collectibles_ids),
//         )
//         .await?;
//     }
//
//     let mut players_to_delete = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.deletes.iter() {
//             match serde_json::from_str::<RawVaultState>(row.as_ref()) {
//                 Ok(vault_state) => {
//                     if !found_in_inserts.contains(&vault_state.entity_id) {
//                         players_to_delete.insert(vault_state.entity_id);
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Delete PlayerState Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if !players_to_delete.is_empty() {
//         delete_vault_state(conn, players_to_delete, true).await?;
//     }
//
//     if !known_vault_state_collectibles_ids.is_empty() {
//         delete_vault_state_collectibles(conn, known_vault_state_collectibles_ids).await?;
//     }
//
//     Ok(())
// }
