use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use entity::deployable_state;
use entity::deployable_state::Model;
use log::{debug, error, info};
use migration::{OnConflict, sea_query};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;

#[allow(dead_code)]
pub(crate) async fn load_deployable_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<deployable_state::Model>> {
    let item_file = File::open(storage_path.join("State/DeployableState.json"))?;
    let deployable_state: Value = serde_json::from_reader(&item_file)?;
    let deployable_states: Vec<deployable_state::Model> = serde_json::from_value(
        deployable_state
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )?;

    Ok(deployable_states)
}

async fn delete_deployable_state(
    conn: &DatabaseConnection,
    known_deployable_state_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "deployable_state's ({}) to delete: {:?}",
        known_deployable_state_ids.len(),
        known_deployable_state_ids
    );
    deployable_state::Entity::delete_many()
        .filter(deployable_state::Column::EntityId.is_in(known_deployable_state_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_deployable_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let deployable_state_from_db = deployable_state::Entity::find()
        .filter(
            deployable_state::Column::EntityId.is_in(
                buffer_before_insert
                    .iter()
                    .map(|deployable_state| deployable_state.entity_id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let deployable_state_from_db_map = deployable_state_from_db
        .into_iter()
        .map(|deployable_state| (deployable_state.entity_id, deployable_state))
        .collect::<HashMap<i64, deployable_state::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|deployable_state| {
            match deployable_state_from_db_map.get(&deployable_state.entity_id) {
                Some(deployable_state_from_db) => {
                    if deployable_state_from_db != *deployable_state {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|deployable_state| deployable_state.clone().into_active_model())
        .collect::<Vec<deployable_state::ActiveModel>>();

    if things_to_insert.len() == 0 {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} deployable_state", things_to_insert.len());
    }

    let _ = deployable_state::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn known_deployable_state_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_deployable_state_ids: Vec<i64> = deployable_state::Entity::find()
        .select_only()
        .column(deployable_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_deployable_state_ids = known_deployable_state_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_deployable_state_ids)
}

fn get_deployable_state_on_conflict() -> OnConflict {
    sea_query::OnConflict::column(deployable_state::Column::EntityId)
        .update_columns([
            deployable_state::Column::OwnerId,
            deployable_state::Column::ClaimEntityId,
            deployable_state::Column::Direction,
            deployable_state::Column::DeployableDescriptionId,
            deployable_state::Column::Nickname,
            deployable_state::Column::Hidden,
        ])
        .to_owned()
}

pub(crate) async fn handle_transaction_update(
    p0: &DatabaseConnection,
    tables: &Vec<TableWithOriginalEventTransactionUpdate>,
) -> anyhow::Result<()> {
    let on_conflict = get_deployable_state_on_conflict();

    let mut buffer_before_insert = HashMap::new();
    let mut found_in_inserts = HashSet::new();
    let chunk_size = Some(1000);

    // let mut known_player_username_state_ids = get_known_player_uusername_state_ids(p0).await?;
    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<deployable_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    found_in_inserts.insert(building_state.entity_id);
                    buffer_before_insert.insert(building_state.entity_id, building_state);

                    if buffer_before_insert.len() == chunk_size.unwrap_or(1000) {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<deployable_state::Model>>();

                        db_insert_deployable_state(p0, &mut buffer_before_insert_vec, &on_conflict)
                            .await?;
                        buffer_before_insert.clear();
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert DeployableState Error: {error}");
                }
            }
        }
    }

    if buffer_before_insert.len() > 0 {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<deployable_state::Model>>();
        db_insert_deployable_state(p0, &mut buffer_before_insert_vec, &on_conflict).await?;
        buffer_before_insert.clear();
    }

    let mut players_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<deployable_state::Model>(row.as_ref()) {
                Ok(deployable_state) => {
                    if !found_in_inserts.contains(&deployable_state.entity_id) {
                        players_to_delete.insert(deployable_state.entity_id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete DeployableState Error: {error}");
                }
            }
        }
    }

    if players_to_delete.len() > 0 {
        delete_deployable_state(p0, players_to_delete).await?;
    }

    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    conn: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let on_conflict = get_deployable_state_on_conflict();

    let mut known_deployable_state_ids = known_deployable_state_ids(conn).await?;

    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<deployable_state::Model> = vec![];

    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<deployable_state::Model>(row.as_ref()) {
                Ok(building_state) => {
                    if known_deployable_state_ids.contains(&building_state.entity_id) {
                        known_deployable_state_ids.remove(&building_state.entity_id);
                    }
                    buffer_before_insert.push(building_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                        db_insert_deployable_state(conn, &mut buffer_before_insert, &on_conflict)
                            .await?;
                    }
                }
                Err(error) => {
                    error!("InitialSubscription Insert BuildingState Error: {error}");
                }
            }
        }
    }
    if buffer_before_insert.len() > 0 {
        for buffer_chnk in buffer_before_insert.chunks(5000) {
            db_insert_deployable_state(conn, &mut buffer_chnk.to_vec(), &on_conflict).await?;
        }
    }

    if known_deployable_state_ids.len() > 0 {
        delete_deployable_state(conn, known_deployable_state_ids).await?;
    }

    Ok(())
}
