use crate::config::Config;
use entity::deployable_state;
use log::{debug, error, info};
use migration::{sea_query, OnConflict};
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
use entity::deployable_state::Model;
use crate::Table;

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

    let on_conflict = get_deployable_state_on_conflict();

    let mut known_deployable_state_ids = known_deployable_state_ids(conn).await?;

    while let Ok(value) = json_stream_reader.deserialize_next::<deployable_state::Model>() {
        if known_deployable_state_ids.contains(&value.entity_id) {
            known_deployable_state_ids.remove(&value.entity_id);
        }

        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            db_insert_deployable_state(conn, &mut buffer_before_insert, &on_conflict).await;
        }
    }

    if buffer_before_insert.len() > 0 {
        db_insert_deployable_state(conn, &mut buffer_before_insert, &on_conflict).await;
        info!("deployable_state last batch imported");
    }
    info!(
        "Importing deployable_state finished in {}s",
        start.elapsed().as_secs()
    );

    if known_deployable_state_ids.len() > 0 {
        delete_deployable_state(conn, known_deployable_state_ids).await?;
    }

    Ok(())
}

async fn delete_deployable_state(conn: &DatabaseConnection, mut known_deployable_state_ids: HashSet<i64>) -> anyhow::Result<()> {
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

async fn db_insert_deployable_state(conn: &DatabaseConnection, mut buffer_before_insert: &mut Vec<Model>, on_conflict: &OnConflict) -> anyhow::Result<()> {
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

    let mut known_deployable_state_ids = known_deployable_state_ids
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

pub async fn import_job_deployable_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_deployable_state(config.clone(), conn.clone(), client);

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

pub(crate) async fn handle_transaction_update(
    p0: &DatabaseConnection,
    tables: &Vec<Table>,
) -> anyhow::Result<()> {
    let on_conflict = get_deployable_state_on_conflict();

    let mut buffer_before_insert = HashMap::new();
    let mut found_in_inserts = HashSet::new();
    let chunk_size = Some(1000);

    // let mut known_player_username_state_ids = get_known_player_uusername_state_ids(p0).await?;
    for p1 in tables.iter() {
        for row in p1.inserts.iter() {
            match serde_json::from_str::<deployable_state::Model>(row.Text.as_ref()) {
                Ok(building_state) => {
                    found_in_inserts.insert(building_state.entity_id);
                    buffer_before_insert.insert(building_state.entity_id, building_state);

                    if buffer_before_insert.len() == chunk_size.unwrap_or(1000) {
                        let mut buffer_before_insert_vec = buffer_before_insert
                            .clone()
                            .into_iter()
                            .map(|x| x.1)
                            .collect::<Vec<deployable_state::Model>>();

                        db_insert_deployable_state(
                            p0,
                            &mut buffer_before_insert_vec,
                            &on_conflict,
                        )
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
            match serde_json::from_str::<deployable_state::Model>(row.Text.as_ref()) {
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

    for row in p1.inserts.iter() {
        match serde_json::from_str::<deployable_state::Model>(row.Text.as_ref()) {
            Ok(building_state) => {
                if known_deployable_state_ids.contains(&building_state.entity_id) {
                    known_deployable_state_ids.remove(&building_state.entity_id);
                }
                buffer_before_insert.push(building_state);
                if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                    db_insert_deployable_state(conn, &mut buffer_before_insert, &on_conflict).await?;
                }
            }
            Err(error) => {
                error!("InitialSubscription Insert BuildingState Error: {error}");
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