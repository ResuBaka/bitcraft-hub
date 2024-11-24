use crate::config::Config;
use entity::deployable_state;
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

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

pub(crate) async fn load_deployable_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM DeployableState")
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

pub(crate) async fn load_deployable_state(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let deployable_states =
        load_deployable_state_from_spacetimedb(client, domain, protocol, database).await?;
    import_deployable_state(&conn, deployable_states, None).await?;
    Ok(())
}

pub(crate) async fn import_deployable_state(
    conn: &DatabaseConnection,
    deployable_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<deployable_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(deployable_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(deployable_state::Column::EntityId)
        .update_columns([
            deployable_state::Column::OwnerId,
            deployable_state::Column::ClaimEntityId,
            deployable_state::Column::Direction,
            deployable_state::Column::DeployableDescriptionId,
            deployable_state::Column::Nickname,
            deployable_state::Column::Hidden,
        ])
        .to_owned();

    let mut deployable_state_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<deployable_state::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
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

            if deployable_state_from_db.len() != buffer_before_insert.len() {
                deployable_state_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|deployable_state| {
                            !deployable_state_from_db
                                .iter()
                                .any(|deployable_state_from_db| {
                                    deployable_state_from_db.entity_id == deployable_state.entity_id
                                })
                        })
                        .map(|deployable_state| deployable_state.entity_id),
                );
            }

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
                continue;
            } else {
                debug!("Inserting {} deployable_state", things_to_insert.len());
            }

            for deployable_state in &things_to_insert {
                let deployable_state_in = deployable_state_to_delete
                    .iter()
                    .position(|id| id == deployable_state.entity_id.as_ref());
                if deployable_state_in.is_some() {
                    deployable_state_to_delete.remove(deployable_state_in.unwrap());
                }
            }

            let _ = deployable_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
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
        } else {
            debug!("Inserting {} deployable_state", things_to_insert.len());
            deployable_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("deployable_state last batch imported");
    }
    info!(
        "Importing deployable_state finished in {}s",
        start.elapsed().as_secs()
    );

    if deployable_state_to_delete.len() > 0 {
        info!(
            "deployable_state's to delete: {:?}",
            deployable_state_to_delete
        );
        deployable_state::Entity::delete_many()
            .filter(deployable_state::Column::EntityId.is_in(deployable_state_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub async fn import_job_deployable_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = super::create_importer_default_db_connection(config.clone()).await;
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_deployable_state(config.clone(), conn, client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_deployable_state(config.clone(), conn, client);
    }
}

fn import_internal_deployable_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let deployable_state = load_deployable_state(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_) = deployable_state {
                    info!("DeployableState imported");
                } else {
                    error!("DeployableState import failed: {:?}", deployable_state);
                }
            });
    });
}
