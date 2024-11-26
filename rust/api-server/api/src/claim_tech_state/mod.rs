use crate::config::Config;
use entity::{claim_tech_desc, claim_tech_state};
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QuerySelect,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) async fn load_claim_tech_state_from_file(
    storage_path: &PathBuf,
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

pub(crate) async fn load_claim_tech_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM ClaimTechState")
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

pub(crate) async fn load_claim_tech_state(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_tech_states =
        load_claim_tech_state_from_spacetimedb(client, domain, protocol, database).await?;
    import_claim_tech_state(&conn, claim_tech_states, None).await?;
    Ok(())
}

pub(crate) async fn import_claim_tech_state(
    conn: &DatabaseConnection,
    claim_tech_descs: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<claim_tech_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(claim_tech_descs.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(claim_tech_state::Column::EntityId)
        .update_columns([
            claim_tech_state::Column::Learned,
            claim_tech_state::Column::Researching,
            claim_tech_state::Column::StartTimestamp,
            claim_tech_state::Column::ScheduledId,
        ])
        .to_owned();

    let known_claim_tech_state_ids: Vec<i64> = claim_tech_state::Entity::find()
        .select_only()
        .column(claim_tech_state::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let mut known_claim_tech_state_ids = known_claim_tech_state_ids
        .into_iter()
        .collect::<HashSet<i64>>();

    while let Ok(value) = json_stream_reader.deserialize_next::<claim_tech_state::Model>() {
        if known_claim_tech_state_ids.contains(&value.entity_id) {
            known_claim_tech_state_ids.remove(&value.entity_id);
        }

        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
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
                        Some(claim_tech_state_from_db) => {
                            if claim_tech_state_from_db != *claim_tech_state {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|claim_tech_state| claim_tech_state.clone().into_active_model())
                .collect::<Vec<claim_tech_state::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            }

            let _ = claim_tech_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
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
                    Some(claim_tech_state_from_db) => {
                        if claim_tech_state_from_db != *claim_tech_state {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|claim_tech_state| claim_tech_state.clone().into_active_model())
            .collect::<Vec<claim_tech_state::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            claim_tech_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("claim_tech_desc last batch imported");
    }
    info!(
        "Importing claim_tech_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if known_claim_tech_state_ids.len() > 0 {
        info!(
            "claim_tech_desc's ({}) to delete: {:?}",
            known_claim_tech_state_ids.len(),
            known_claim_tech_state_ids,
        );
        claim_tech_state::Entity::delete_many()
            .filter(claim_tech_state::Column::EntityId.is_in(known_claim_tech_state_ids))
            .exec(conn)
            .await?;
    }

    Ok(())
}

fn import_internal_claim_tech_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let claim_tech_state = load_claim_tech_state(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_claim_tech_state) = claim_tech_state {
                    info!("ClaimTechState imported");
                } else {
                    error!("ClaimTechState import failed: {:?}", claim_tech_state);
                }
            });
    });
}

pub async fn import_job_claim_tech_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_claim_tech_state(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_claim_tech_state(config.clone(), conn, client);
    }
}
