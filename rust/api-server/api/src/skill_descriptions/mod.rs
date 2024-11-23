use entity::skill_desc;
use log::{debug, error, info};
use migration::sea_query;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm::{IntoActiveModel, PaginatorTrait};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use reqwest::Client;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;
use crate::config::Config;
use crate::skill_descriptions;

#[allow(dead_code)]
pub(crate) async fn load_skill_desc_from_file(
    storage_path: &PathBuf,
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
        .body("SELECT * FROM SkillDesc")
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

    import_skill_descs(&conn, claim_descriptions, Some(3000)).await?;

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
                .filter(|skill_desc| {
                    match skill_descs_from_db_map.get(&skill_desc.id) {
                        Some(skill_desc_from_db) => {
                            if skill_desc_from_db != *skill_desc {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|skill_desc| skill_desc.clone().into_active_model())
                .collect::<Vec<skill_desc::ActiveModel>>();

            if things_to_insert.len() == 0 {
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
                if skill_desc_in.is_some() {
                    skill_descs_to_delete.remove(skill_desc_in.unwrap());
                }
            }

            let _ = skill_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
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
            .filter(|skill_desc| {
                match skill_descs_from_db_map.get(&skill_desc.id) {
                    Some(skill_desc_from_db) => {
                        if skill_desc_from_db != *skill_desc {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|skill_desc| skill_desc.clone().into_active_model())
            .collect::<Vec<skill_desc::ActiveModel>>();

        if things_to_insert.len() == 0 {
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

    if skill_descs_to_delete.len() > 0 {
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

                if let Ok(_) = skill_descriptions_desc {
                    info!("SkillDescriptionsDesc imported");
                } else {
                    error!("SkillDescriptionsDesc import failed: {:?}", skill_descriptions_desc);
                }
            });
    });
}

pub async fn import_job_skill_desc(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = super::create_importer_default_db_connection(config.clone()).await;
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_skill_descriptions(config.clone(), conn, client);

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