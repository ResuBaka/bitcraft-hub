use crate::AppState;
use crate::config::Config;
use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use entity::claim_tech_desc;
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
};
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
pub(crate) async fn load_claim_tech_desc_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<claim_tech_desc::Model>> {
    let item_file = File::open(storage_path.join("Desc/ClaimTechDesc.json"))?;
    let claim_tech_desc: Value = serde_json::from_reader(&item_file)?;
    let claim_tech_descs: Vec<claim_tech_desc::Model> =
        serde_json::from_value(claim_tech_desc.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(claim_tech_descs)
}

pub(crate) async fn load_claim_tech_desc_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM claim_tech_desc")
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

pub(crate) async fn load_claim_tech_desc(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_tech_descs =
        load_claim_tech_desc_from_spacetimedb(client, domain, protocol, database).await?;
    import_claim_tech_desc(conn, claim_tech_descs, None).await?;
    Ok(())
}

pub(crate) async fn import_claim_tech_desc(
    conn: &DatabaseConnection,
    claim_tech_descs: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<claim_tech_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(claim_tech_descs.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(claim_tech_desc::Column::Id)
        .update_columns([
            claim_tech_desc::Column::Description,
            claim_tech_desc::Column::Tier,
            claim_tech_desc::Column::SuppliesCost,
            claim_tech_desc::Column::ResearchTime,
            claim_tech_desc::Column::Requirements,
            claim_tech_desc::Column::Input,
            claim_tech_desc::Column::Members,
            claim_tech_desc::Column::Area,
            claim_tech_desc::Column::Supply,
        ])
        .to_owned();

    let mut claim_tech_desc_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<claim_tech_desc::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let claim_tech_desc_from_db = claim_tech_desc::Entity::find()
                .filter(
                    claim_tech_desc::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|claim_tech_desc| claim_tech_desc.id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if claim_tech_desc_from_db.len() != buffer_before_insert.len() {
                claim_tech_desc_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|claim_tech_desc| {
                            !claim_tech_desc_from_db
                                .iter()
                                .any(|claim_tech_desc_from_db| {
                                    claim_tech_desc_from_db.id == claim_tech_desc.id
                                })
                        })
                        .map(|claim_tech_desc| claim_tech_desc.id),
                );
            }

            let claim_tech_desc_from_db_map = claim_tech_desc_from_db
                .into_iter()
                .map(|claim_tech_desc| (claim_tech_desc.id, claim_tech_desc))
                .collect::<HashMap<i64, claim_tech_desc::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|claim_tech_desc| {
                    match claim_tech_desc_from_db_map.get(&claim_tech_desc.id) {
                        Some(claim_tech_desc_from_db) => {
                            claim_tech_desc_from_db != *claim_tech_desc
                        }
                        None => true,
                    }
                })
                .map(|claim_tech_desc| claim_tech_desc.clone().into_active_model())
                .collect::<Vec<claim_tech_desc::ActiveModel>>();

            if things_to_insert.is_empty() {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            }

            for claim_tech_desc in &things_to_insert {
                let claim_tech_desc_in = claim_tech_desc_to_delete
                    .iter()
                    .position(|id| id == claim_tech_desc.id.as_ref());
                if let Some(claim_tech_desc_in) = claim_tech_desc_in {
                    claim_tech_desc_to_delete.remove(claim_tech_desc_in);
                }
            }

            let _ = claim_tech_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if !buffer_before_insert.is_empty() {
        let claim_tech_desc_from_db = claim_tech_desc::Entity::find()
            .filter(
                claim_tech_desc::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|claim_tech_desc| claim_tech_desc.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let claim_tech_desc_from_db_map = claim_tech_desc_from_db
            .into_iter()
            .map(|claim_tech_desc| (claim_tech_desc.id, claim_tech_desc))
            .collect::<HashMap<i64, claim_tech_desc::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(
                |claim_tech_desc| match claim_tech_desc_from_db_map.get(&claim_tech_desc.id) {
                    Some(claim_tech_desc_from_db) => claim_tech_desc_from_db != *claim_tech_desc,
                    None => true,
                },
            )
            .map(|claim_tech_desc| claim_tech_desc.clone().into_active_model())
            .collect::<Vec<claim_tech_desc::ActiveModel>>();

        if things_to_insert.is_empty() {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} claim_tech_desc", things_to_insert.len());
            claim_tech_desc::Entity::insert_many(things_to_insert)
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

    if !claim_tech_desc_to_delete.is_empty() {
        info!(
            "claim_tech_desc's to delete: {:?}",
            claim_tech_desc_to_delete
        );
        claim_tech_desc::Entity::delete_many()
            .filter(claim_tech_desc::Column::Id.is_in(claim_tech_desc_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

fn import_interal_claim_tech_desc(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let claim_tech_desc = load_claim_tech_desc(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_claim_tech_desc) = claim_tech_desc {
                    info!("ClaimTechDesc imported");
                } else {
                    error!("ClaimTechDesc import failed: {:?}", claim_tech_desc);
                }
            });
    });
}

pub async fn import_job_claim_tech_desc(temp_config: Config) {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_interal_claim_tech_desc(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_interal_claim_tech_desc(config.clone(), conn, client);
    }
}

async fn get_known_claim_tech_desc_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_claim_tech_desc_ids: Vec<i64> = claim_tech_desc::Entity::find()
        .select_only()
        .column(claim_tech_desc::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;

    let known_claim_tech_desc_ids = known_claim_tech_desc_ids
        .into_iter()
        .collect::<HashSet<i64>>();
    Ok(known_claim_tech_desc_ids)
}

async fn db_insert_claim_tech_descs(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<claim_tech_desc::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let claim_tech_descs_from_db = claim_tech_desc::Entity::find()
        .filter(
            claim_tech_desc::Column::Id.is_in(
                buffer_before_insert
                    .iter()
                    .map(|claim_tech_desc| claim_tech_desc.id)
                    .collect::<Vec<i64>>(),
            ),
        )
        .all(conn)
        .await?;

    let claim_tech_descs_from_db_map = claim_tech_descs_from_db
        .into_iter()
        .map(|claim_tech_desc| (claim_tech_desc.id, claim_tech_desc))
        .collect::<HashMap<i64, claim_tech_desc::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(
            |claim_tech_desc| match claim_tech_descs_from_db_map.get(&claim_tech_desc.id) {
                Some(claim_tech_desc_from_db) => claim_tech_desc_from_db != *claim_tech_desc,
                None => true,
            },
        )
        .map(|claim_tech_desc| claim_tech_desc.clone().into_active_model())
        .collect::<Vec<claim_tech_desc::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} claim_tech_descs", things_to_insert.len());
    }

    let _ = claim_tech_desc::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_claim_tech_desc(
    conn: &DatabaseConnection,
    known_claim_tech_desc_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "claim_tech_desc's ({}) to delete: {:?}",
        known_claim_tech_desc_ids.len(),
        known_claim_tech_desc_ids
    );
    claim_tech_desc::Entity::delete_many()
        .filter(claim_tech_desc::Column::Id.is_in(known_claim_tech_desc_ids))
        .exec(conn)
        .await?;
    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    app_state: &Arc<AppState>,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = 5000;
    let mut buffer_before_insert: Vec<claim_tech_desc::Model> = Vec::with_capacity(chunk_size);

    let on_conflict = sea_query::OnConflict::column(claim_tech_desc::Column::Id)
        .update_columns([
            claim_tech_desc::Column::Description,
            claim_tech_desc::Column::Tier,
            claim_tech_desc::Column::SuppliesCost,
            claim_tech_desc::Column::ResearchTime,
            claim_tech_desc::Column::Requirements,
            claim_tech_desc::Column::Input,
            claim_tech_desc::Column::Members,
            claim_tech_desc::Column::Area,
            claim_tech_desc::Column::Supply,
            claim_tech_desc::Column::XpToMintHexCoin,
        ])
        .to_owned();

    let mut known_claim_tech_desc_ids = get_known_claim_tech_desc_ids(&app_state.conn).await?;
    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<claim_tech_desc::Model>(row.as_ref()) {
                Ok(claim_tech_desc) => {
                    if known_claim_tech_desc_ids.contains(&claim_tech_desc.id) {
                        known_claim_tech_desc_ids.remove(&claim_tech_desc.id);
                    }
                    app_state
                        .claim_tech_desc
                        .insert(claim_tech_desc.id, claim_tech_desc.clone());
                    buffer_before_insert.push(claim_tech_desc);
                    if buffer_before_insert.len() == chunk_size {
                        info!("ClaimTechDesc insert");
                        db_insert_claim_tech_descs(
                            &app_state.conn,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert ClaimTechDesc Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        info!("ClaimTechDesc insert");
        db_insert_claim_tech_descs(&app_state.conn, &mut buffer_before_insert, &on_conflict)
            .await?;
    }

    if !known_claim_tech_desc_ids.is_empty() {
        for claim_tech_desc_id in &known_claim_tech_desc_ids {
            app_state.claim_tech_desc.remove(claim_tech_desc_id);
        }
        delete_claim_tech_desc(&app_state.conn, known_claim_tech_desc_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    app_state: &Arc<AppState>,
    tables: &[TableWithOriginalEventTransactionUpdate],
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(claim_tech_desc::Column::Id)
        .update_columns([
            claim_tech_desc::Column::Description,
            claim_tech_desc::Column::Tier,
            claim_tech_desc::Column::SuppliesCost,
            claim_tech_desc::Column::ResearchTime,
            claim_tech_desc::Column::Requirements,
            claim_tech_desc::Column::Input,
            claim_tech_desc::Column::Members,
            claim_tech_desc::Column::Area,
            claim_tech_desc::Column::Supply,
            claim_tech_desc::Column::XpToMintHexCoin,
        ])
        .to_owned();

    let chunk_size = 5000;
    let mut buffer_before_insert = HashMap::new();

    let mut found_in_inserts = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<claim_tech_desc::Model>(row.as_ref()) {
                Ok(claim_tech_desc) => {
                    app_state
                        .claim_tech_desc
                        .insert(claim_tech_desc.id, claim_tech_desc.clone());
                    found_in_inserts.insert(claim_tech_desc.id);
                    buffer_before_insert.insert(claim_tech_desc.id, claim_tech_desc);

                    if buffer_before_insert.len() == chunk_size {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<claim_tech_desc::Model>>();

                        db_insert_claim_tech_descs(
                            &app_state.conn,
                            &mut buffer_before_insert_vec,
                            &on_conflict,
                        )
                        .await?;
                        buffer_before_insert.clear();
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Insert ClaimTechDesc Error: {error}");
                }
            }
        }
    }

    if !buffer_before_insert.is_empty() {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<claim_tech_desc::Model>>();

        db_insert_claim_tech_descs(&app_state.conn, &mut buffer_before_insert_vec, &on_conflict)
            .await?;
        buffer_before_insert.clear();
    }

    let mut claim_tech_descs_to_delete = HashSet::new();

    for p1 in tables.iter() {
        for row in p1.deletes.iter() {
            match serde_json::from_str::<claim_tech_desc::Model>(row.as_ref()) {
                Ok(claim_tech_desc) => {
                    if !found_in_inserts.contains(&claim_tech_desc.id) {
                        app_state.claim_tech_desc.remove(&claim_tech_desc.id);
                        claim_tech_descs_to_delete.insert(claim_tech_desc.id);
                    }
                }
                Err(error) => {
                    error!("TransactionUpdate Delete ClaimTechDesc Error: {error}");
                }
            }
        }
    }

    if !claim_tech_descs_to_delete.is_empty() {
        delete_claim_tech_desc(&app_state.conn, claim_tech_descs_to_delete).await?;
    }

    Ok(())
}
