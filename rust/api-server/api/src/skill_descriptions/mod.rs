use crate::config::Config;
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use crate::{AppState, skill_descriptions};
use entity::skill_desc;
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm::{IntoActiveModel, QuerySelect};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

#[allow(dead_code)]
pub(crate) async fn load_skill_desc_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<skill_desc::Model>> {
    let skill_desc_file = File::open(storage_path.join("Desc/SkillDesc.json"))?;
    let skill_desc: Value = serde_json::from_reader(&skill_desc_file)?;
    let skill_desc: Vec<skill_desc::Model> =
        serde_json::from_value(skill_desc.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(skill_desc)
}

pub(crate) async fn load_skill_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM skill_desc")
        .send()
        .await;
    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error}");
            return Ok("".to_string());
        }
    };

    Ok(json)
}

pub(crate) async fn load_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_skill_desc_from_spacetimedb(client, domain, protocol, database).await?;

    import_skill_descs(conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}

pub(crate) async fn import_skill_descs(
    conn: &DatabaseConnection,
    skill_descs: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<skill_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(skill_descs.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
        .update_columns([
            skill_desc::Column::Name,
            skill_desc::Column::Description,
            skill_desc::Column::IconAssetName,
            skill_desc::Column::Title,
            skill_desc::Column::SkillCategory,
            skill_desc::Column::Skill,
        ])
        .to_owned();

    let mut skill_descs_to_delete = Vec::new();
    while let Ok(value) = json_stream_reader.deserialize_next::<skill_desc::SkillDescRaw>() {
        let value = value.to_model()?;

        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let skill_descs_from_db = skill_desc::Entity::find()
                .filter(
                    skill_desc::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|skill_desc| skill_desc.id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if skill_descs_from_db.len() != buffer_before_insert.len() {
                skill_descs_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|skill_desc| {
                            !skill_descs_from_db
                                .iter()
                                .any(|skill_desc_from_db| skill_desc_from_db.id == skill_desc.id)
                        })
                        .map(|skill_desc| skill_desc.id),
                );
            }

            let skill_descs_from_db_map = skill_descs_from_db
                .into_iter()
                .map(|skill_desc| (skill_desc.id, skill_desc))
                .collect::<HashMap<i64, skill_desc::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(
                    |skill_desc| match skill_descs_from_db_map.get(&skill_desc.id) {
                        Some(skill_desc_from_db) => skill_desc_from_db != *skill_desc,
                        None => true,
                    },
                )
                .map(|skill_desc| skill_desc.clone().into_active_model())
                .collect::<Vec<skill_desc::ActiveModel>>();

            if things_to_insert.is_empty() {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} skill_descs", things_to_insert.len());
            }

            for skill_desc in &things_to_insert {
                let skill_desc_in = skill_descs_to_delete
                    .iter()
                    .position(|id| id == skill_desc.id.as_ref());
                if let Some(skill_desc_in) = skill_desc_in {
                    skill_descs_to_delete.remove(skill_desc_in);
                }
            }

            let _ = skill_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if !buffer_before_insert.is_empty() {
        let skill_descs_from_db = skill_desc::Entity::find()
            .filter(
                skill_desc::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|skill_desc| skill_desc.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let skill_descs_from_db_map = skill_descs_from_db
            .into_iter()
            .map(|skill_desc| (skill_desc.id, skill_desc))
            .collect::<HashMap<i64, skill_desc::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(
                |skill_desc| match skill_descs_from_db_map.get(&skill_desc.id) {
                    Some(skill_desc_from_db) => skill_desc_from_db != *skill_desc,
                    None => true,
                },
            )
            .map(|skill_desc| skill_desc.clone().into_active_model())
            .collect::<Vec<skill_desc::ActiveModel>>();

        if things_to_insert.is_empty() {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} skill_descs", things_to_insert.len());
            skill_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("skill_desc last batch imported");
    }
    info!(
        "Importing skill_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if !skill_descs_to_delete.is_empty() {
        info!("skill_desc's to delete: {:?}", skill_descs_to_delete);
        skill_desc::Entity::delete_many()
            .filter(skill_desc::Column::Id.is_in(skill_descs_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

fn import_skill_descriptions(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let skill_descriptions_desc = skill_descriptions::load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                    // &config,
                )
                .await;

                if skill_descriptions_desc.is_ok() {
                    info!("SkillDescriptionsDesc imported");
                } else {
                    error!(
                        "SkillDescriptionsDesc import failed: {:?}",
                        skill_descriptions_desc
                    );
                }
            });
    });
}

pub async fn import_job_skill_desc(temp_config: Config) {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_skill_descriptions(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_skill_descriptions(config.clone(), conn, client);
    }
}

async fn get_known_skill_desc_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_skill_desc_ids: Vec<i64> = skill_desc::Entity::find()
        .select_only()
        .column(skill_desc::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;

    let known_skill_desc_ids = known_skill_desc_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_skill_desc_ids)
}

async fn db_insert_skill_descs(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<skill_desc::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let skill_descs_from_db = skill_desc::Entity::find()
        .filter(
            skill_desc::Column::Id.is_in(
                buffer_before_insert
                    .iter()
                    .map(|skill_desc| skill_desc.id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let skill_descs_from_db_map = skill_descs_from_db
        .into_iter()
        .map(|skill_desc| (skill_desc.id, skill_desc))
        .collect::<HashMap<i64, skill_desc::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(
            |skill_desc| match skill_descs_from_db_map.get(&skill_desc.id) {
                Some(skill_desc_from_db) => skill_desc_from_db != *skill_desc,
                None => true,
            },
        )
        .map(|skill_desc| skill_desc.clone().into_active_model())
        .collect::<Vec<skill_desc::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} skill_descs", things_to_insert.len());
    }

    let _ = skill_desc::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_skill_desc(
    conn: &DatabaseConnection,
    known_skill_desc_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "skill_desc's ({}) to delete: {:?}",
        known_skill_desc_ids.len(),
        known_skill_desc_ids
    );
    skill_desc::Entity::delete_many()
        .filter(skill_desc::Column::Id.is_in(known_skill_desc_ids))
        .exec(conn)
        .await?;
    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    app_state: &Arc<AppState>,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<skill_desc::Model> = Vec::with_capacity(chunk_size);

    let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
        .update_columns([
            skill_desc::Column::Skill,
            skill_desc::Column::Name,
            skill_desc::Column::Description,
            skill_desc::Column::IconAssetName,
            skill_desc::Column::Title,
            skill_desc::Column::SkillCategory,
        ])
        .to_owned();

    let mut known_skill_desc_ids = get_known_skill_desc_ids(&app_state.conn).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<skill_desc::Model>(row.as_ref()) {
                Ok(skill_desc) => {
                    if known_skill_desc_ids.contains(&skill_desc.id) {
                        known_skill_desc_ids.remove(&skill_desc.id);
                    }
                    app_state
                        .skill_desc
                        .insert(skill_desc.id, skill_desc.clone());
                    buffer_before_insert.push(skill_desc);
                    if buffer_before_insert.len() == chunk_size {
                        info!("SkillDesc insert");
                        db_insert_skill_descs(
                            &app_state.conn,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert SkillDesc Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        info!("SkillDesc insert");
        db_insert_skill_descs(&app_state.conn, &mut buffer_before_insert, &on_conflict).await?;
    }

    if !known_skill_desc_ids.is_empty() {
        for skill_desc_id in &known_skill_desc_ids {
            app_state.skill_desc.remove(skill_desc_id);
        }
        delete_skill_desc(&app_state.conn, known_skill_desc_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    app_state: &Arc<AppState>,
    tables: &[TableWithOriginalEventTransactionUpdate],
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
        .update_columns([
            skill_desc::Column::Skill,
            skill_desc::Column::Name,
            skill_desc::Column::Description,
            skill_desc::Column::IconAssetName,
            skill_desc::Column::Title,
            skill_desc::Column::SkillCategory,
        ])
        .to_owned();

    let chunk_size = 5000;
    let mut buffer_before_insert = HashMap::new();

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<skill_desc::Model>(row.as_ref()) {
                Ok(skill_desc) => {
                    app_state
                        .skill_desc
                        .insert(skill_desc.id, skill_desc.clone());
                    found_in_inserts.insert(skill_desc.id);
                    buffer_before_insert.insert(skill_desc.id, skill_desc);

                    if buffer_before_insert.len() == chunk_size {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<skill_desc::Model>>();

                        db_insert_skill_descs(
                            &app_state.conn,
                            &mut buffer_before_insert_vec,
                            &on_conflict,
                        )
                        .await?;
                        buffer_before_insert.clear();
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert SkillDesc Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<skill_desc::Model>>();

        db_insert_skill_descs(&app_state.conn, &mut buffer_before_insert_vec, &on_conflict).await?;
        buffer_before_insert.clear();
    }

    let mut skill_descs_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<skill_desc::Model>(row.as_ref()) {
                Ok(skill_desc) => {
                    if !found_in_inserts.contains(&skill_desc.id) {
                        app_state.skill_desc.remove(&skill_desc.id);
                        skill_descs_to_delete.insert(skill_desc.id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete SkillDesc Error: {error}");
                }
            }
        }
    }

    if !skill_descs_to_delete.is_empty() {
        delete_skill_desc(&app_state.conn, skill_descs_to_delete).await?;
    }

    Ok(())
}
