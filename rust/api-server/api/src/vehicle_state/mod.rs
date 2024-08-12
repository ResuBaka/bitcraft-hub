use entity::vehicle_state;
use log::{debug, error, info};
use migration::sea_query;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) async fn load_vehicle_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<vehicle_state::Model>> {
    let item_file = File::open(storage_path.join("State/VehicleState.json"))?;
    let vehicle_state: Value = serde_json::from_reader(&item_file)?;
    let vehicle_states: Vec<vehicle_state::Model> =
        serde_json::from_value(vehicle_state.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(vehicle_states)
}

pub(crate) async fn load_vehicle_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM VehicleState")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".into());
        }
    };

    Ok(json)
}

pub(crate) async fn load_vehicle_state(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let vehicle_states =
        load_vehicle_state_from_spacetimedb(client, domain, protocol, database).await?;
    import_vehicle_state(&conn, vehicle_states, None).await?;
    Ok(())
}

pub(crate) async fn import_vehicle_state(
    conn: &DatabaseConnection,
    vehicle_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<vehicle_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(vehicle_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(vehicle_state::Column::EntityId)
        .update_columns([
            vehicle_state::Column::OwnerId,
            vehicle_state::Column::Direction,
            vehicle_state::Column::VehicleDescriptionId,
            vehicle_state::Column::Nickname,
        ])
        .to_owned();

    let mut vehicle_state_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<vehicle_state::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let vehicle_state_from_db = vehicle_state::Entity::find()
                .filter(
                    vehicle_state::Column::EntityId.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|vehicle_state| vehicle_state.entity_id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if vehicle_state_from_db.len() != buffer_before_insert.len() {
                vehicle_state_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|vehicle_state| {
                            !vehicle_state_from_db.iter().any(|vehicle_state_from_db| {
                                vehicle_state_from_db.entity_id == vehicle_state.entity_id
                            })
                        })
                        .map(|vehicle_state| vehicle_state.entity_id),
                );
            }

            let vehicle_state_from_db_map = vehicle_state_from_db
                .into_iter()
                .map(|vehicle_state| (vehicle_state.entity_id, vehicle_state))
                .collect::<HashMap<i64, vehicle_state::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|vehicle_state| {
                    match vehicle_state_from_db_map.get(&vehicle_state.entity_id) {
                        Some(vehicle_state_from_db) => {
                            if vehicle_state_from_db != *vehicle_state {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|vehicle_state| vehicle_state.clone().into_active_model())
                .collect::<Vec<vehicle_state::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} vehicle_state", things_to_insert.len());
            }

            for vehicle_state in &things_to_insert {
                let vehicle_state_in = vehicle_state_to_delete
                    .iter()
                    .position(|id| id == vehicle_state.entity_id.as_ref());
                if vehicle_state_in.is_some() {
                    vehicle_state_to_delete.remove(vehicle_state_in.unwrap());
                }
            }

            let _ = vehicle_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let vehicle_state_from_db = vehicle_state::Entity::find()
            .filter(
                vehicle_state::Column::EntityId.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|vehicle_state| vehicle_state.entity_id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let vehicle_state_from_db_map = vehicle_state_from_db
            .into_iter()
            .map(|vehicle_state| (vehicle_state.entity_id, vehicle_state))
            .collect::<HashMap<i64, vehicle_state::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|vehicle_state| {
                match vehicle_state_from_db_map.get(&vehicle_state.entity_id) {
                    Some(vehicle_state_from_db) => {
                        if vehicle_state_from_db != *vehicle_state {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|vehicle_state| vehicle_state.clone().into_active_model())
            .collect::<Vec<vehicle_state::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} vehicle_state", things_to_insert.len());
            vehicle_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("vehicle_state last batch imported");
    }
    info!(
        "Importing vehicle_state finished in {}s",
        start.elapsed().as_secs()
    );

    if vehicle_state_to_delete.len() > 0 {
        info!("vehicle_state's to delete: {:?}", vehicle_state_to_delete);
        vehicle_state::Entity::delete_many()
            .filter(vehicle_state::Column::EntityId.is_in(vehicle_state_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}
