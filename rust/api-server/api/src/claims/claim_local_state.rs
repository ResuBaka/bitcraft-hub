use sea_orm::QueryFilter;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QuerySelect};
use tokio::sync::mpsc::UnboundedSender;
use entity::{claim_local_state};
use migration::{sea_query, OnConflict};
use crate::AppState;
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate, WebSocketMessages};

fn get_on_conflict() -> OnConflict {
    sea_query::OnConflict::column(claim_local_state::Column::EntityId)
        .update_columns([
            claim_local_state::Column::Supplies,
            claim_local_state::Column::BuildingMaintenance,
            claim_local_state::Column::NumTiles,
            claim_local_state::Column::NumTileNeighbors,
            claim_local_state::Column::Location,
            claim_local_state::Column::Treasury,
            claim_local_state::Column::XpGainedSinceLastCoinMinting,
            claim_local_state::Column::SuppliesPurchaseThreshold,
            claim_local_state::Column::SuppliesPurchasePrice,
            claim_local_state::Column::BuildingDescriptionId,
        ])
        .to_owned()
}

async fn known_claim_local_state_ids(
    conn: &DatabaseConnection,
) -> anyhow::Result<HashSet<i64>> {
    let known_claim_ids: Vec<i64> = claim_local_state::Entity::find()
        .select_only()
        .column(claim_local_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_claim_ids = known_claim_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_claim_ids)
}

async fn db_insert_claim_local_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<claim_local_state::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let claim_from_db = claim_local_state::Entity::find()
        .filter(
            claim_local_state::Column::EntityId.is_in(
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
        .collect::<HashMap<i64, claim_local_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|claim| {
            match claim_from_db_map.get(&claim.entity_id) {
                Some(claim_from_db) => claim_from_db != *claim,
                None => true,
            }
        })
        .map(|claim| claim.clone().into_active_model())
        .collect::<Vec<claim_local_state::ActiveModel>>();

    if things_to_insert.is_empty() {
        tracing::debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        tracing::debug!("Inserting {} claim_local_state", things_to_insert.len());
    }

    let _ = claim_local_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_claim_local_state(
    conn: &DatabaseConnection,
    known_claim_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    tracing::info!(
        "claim_local_state's ({}) to delete: {:?}",
        known_claim_ids.len(),
        known_claim_ids,
    );
    claim_local_state::Entity::delete_many()
        .filter(
            claim_local_state::Column::EntityId.is_in(known_claim_ids.clone()),
        )
        .exec(conn)
        .await?;

    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    app_state: &Arc<AppState>,
    p1: &Table,
) -> anyhow::Result<()> {
    let on_conflict = get_on_conflict();

    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<claim_local_state::Model> = vec![];

    let mut known_inventory_ids = known_claim_local_state_ids(&app_state.conn).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<claim_local_state::ModelRaw>(row.as_ref()) {
                Ok(claim_local_state) => {
                    let claim_local_state: claim_local_state::Model = claim_local_state.into();
                    if known_inventory_ids.contains(&claim_local_state.entity_id) {
                        known_inventory_ids.remove(&claim_local_state.entity_id);
                    }
                    buffer_before_insert.push(claim_local_state.clone());
                    app_state
                        .claim_local_state
                        .insert(claim_local_state.entity_id as u64, claim_local_state.clone());
                    if buffer_before_insert.len() == chunk_size {
                        db_insert_claim_local_state(
                            &app_state.conn,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                            .await?;
                    }
                }
                Err(error) => {
                    tracing::error!(json = row, "InitialSubscription Insert ClaimLocalState Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        for buffer_chnk in buffer_before_insert.chunks(5000) {
            db_insert_claim_local_state(
                &app_state.conn,
                &mut buffer_chnk.to_vec(),
                &on_conflict,
            )
                .await?;
        }
    }

    if !known_inventory_ids.is_empty() {
        delete_claim_local_state(&app_state.conn, known_inventory_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    app_state: &Arc<AppState>,
    tables: &[TableWithOriginalEventTransactionUpdate],
    sender: UnboundedSender<WebSocketMessages>,
) -> anyhow::Result<()> {
    let on_conflict = get_on_conflict();

    let mut buffer_before_insert = HashMap::new();
    let mut potential_deletes = HashSet::new();

    for p1 in tables.iter() {
        let event_type = if !p1.inserts.is_empty() && !p1.deletes.is_empty() {
            "update"
        } else if !p1.inserts.is_empty() && p1.deletes.is_empty() {
            "insert"
        } else if !p1.deletes.is_empty() && p1.inserts.is_empty() {
            "delete"
        } else {
            "unknown"
        };

        if event_type == "unknown" {
            tracing::error!("Unknown event type {:?}", p1);
            continue;
        }

        if event_type == "delete" {
            for row in p1.deletes.iter() {
                match serde_json::from_str::<claim_local_state::ModelRaw>(row.as_ref()) {
                    Ok(claim_local_state) => {
                        let claim_local_state: claim_local_state::Model = claim_local_state.into();
                        potential_deletes.insert(claim_local_state.entity_id);
                    }
                    Err(error) => {
                        tracing::error!(json = row, "Event: {event_type} Error: {error} for row: {:?}", row);
                    }
                }
            }
        } else if event_type == "update" {
            let mut delete_parsed = HashMap::new();
            for row in p1.deletes.iter() {
                let parsed = serde_json::from_str::<claim_local_state::ModelRaw>(row.as_ref());

                if parsed.is_err() {
                    tracing::error!(
                        "Could not parse delete claim_local_state: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row
                    );
                } else {
                    let parsed = parsed.unwrap();
                    let parsed: claim_local_state::Model = parsed.into();
                    delete_parsed.insert(parsed.entity_id, parsed.clone());
                    potential_deletes.remove(&parsed.entity_id);
                }
            }

            for row in p1.inserts.iter().enumerate() {
                let parsed = serde_json::from_str::<claim_local_state::ModelRaw>(row.1.as_ref());

                if parsed.is_err() {
                    tracing::error!(
                        "Could not parse insert claim_local_state: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row.1
                    );
                    continue;
                }

                let parsed = parsed.unwrap();
                let parsed: claim_local_state::Model = parsed.into();
                let id = parsed.entity_id;

                match (parsed, delete_parsed.get(&id)) {
                    (new_claim_local_state, Some(_old_claim_local_state)) => {
                        app_state.claim_local_state.insert(
                            new_claim_local_state.entity_id as u64,
                            new_claim_local_state.clone(),
                        );
                        buffer_before_insert.insert(
                            new_claim_local_state.entity_id,
                            new_claim_local_state.clone(),
                        );
                        potential_deletes.remove(&new_claim_local_state.entity_id);
                    }
                    (_new_claim_local_state, None) => {
                        tracing::error!("Could not find delete state new experience state",);
                    }
                }
            }
        } else if event_type == "insert" {
            for row in p1.inserts.iter() {
                match serde_json::from_str::<claim_local_state::ModelRaw>(row.as_ref()) {
                    Ok(claim_local_state) => {
                        let claim_local_state: claim_local_state::Model = claim_local_state.into();
                        app_state.claim_local_state.insert(
                            claim_local_state.entity_id as u64,
                            claim_local_state.clone(),
                        );
                        buffer_before_insert.insert(
                            claim_local_state.entity_id,
                            claim_local_state.clone(),
                        );

                        // sender
                        //     .send(WebSocketMessages::ClaimState(
                        //         claim_local_state.clone(),
                        //     ))
                        //     .unwrap();
                    }
                    Err(error) => {
                        tracing::error!("Error: {error} for row: {:?}", row);
                    }
                }
            }
        } else {
            tracing::error!("Unknown event type {:?}", p1);
            continue;
        }
    }

    if !buffer_before_insert.is_empty() {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<claim_local_state::Model>>();
        db_insert_claim_local_state(
            &app_state.conn,
            &mut buffer_before_insert_vec,
            &on_conflict,
        )
        .await?;
        buffer_before_insert.clear();
    }

    if !potential_deletes.is_empty() {
        for potential_delete in potential_deletes.iter() {
            app_state
                .claim_local_state
                .remove(&(*potential_delete as u64));
        }

        delete_claim_local_state(&app_state.conn, potential_deletes).await?;
    }

    Ok(())
}
