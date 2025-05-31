use crate::AppState;
use crate::config::Config;
use entity::cargo_desc;
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

pub(crate) async fn import_cargo_description(
    conn: &DatabaseConnection,
    cargo_descriptions: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<cargo_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(cargo_descriptions.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
        .update_columns([
            cargo_desc::Column::Name,
            cargo_desc::Column::Description,
            cargo_desc::Column::Volume,
            cargo_desc::Column::SecondaryKnowledgeId,
            cargo_desc::Column::ModelAssetName,
            cargo_desc::Column::IconAssetName,
            cargo_desc::Column::CarriedModelAssetName,
            cargo_desc::Column::PickUpAnimationStart,
            cargo_desc::Column::PickUpAnimationEnd,
            cargo_desc::Column::DropAnimationStart,
            cargo_desc::Column::DropAnimationEnd,
            cargo_desc::Column::PickUpTime,
            cargo_desc::Column::PlaceTime,
            cargo_desc::Column::AnimatorState,
            cargo_desc::Column::MovementModifier,
            cargo_desc::Column::BlocksPath,
            cargo_desc::Column::OnDestroyYieldCargos,
            cargo_desc::Column::DespawnTime,
            cargo_desc::Column::Tier,
            cargo_desc::Column::Tag,
            cargo_desc::Column::Rarity,
            cargo_desc::Column::NotPickupable,
        ])
        .to_owned();

    let mut cargo_desc_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<cargo_desc::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let cargo_desc_from_db = cargo_desc::Entity::find()
                .filter(
                    cargo_desc::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|cargo_desc| cargo_desc.id)
                            .collect::<Vec<i32>>(),
                    ),
                )
                .all(conn)
                .await?;

            if cargo_desc_from_db.len() != buffer_before_insert.len() {
                cargo_desc_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|cargo_desc| {
                            !cargo_desc_from_db
                                .iter()
                                .any(|cargo_desc_from_db| cargo_desc_from_db.id == cargo_desc.id)
                        })
                        .map(|cargo_desc| cargo_desc.id),
                );
            }

            let cargo_desc_from_db_map = cargo_desc_from_db
                .into_iter()
                .map(|cargo_desc| (cargo_desc.id, cargo_desc))
                .collect::<HashMap<i32, cargo_desc::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(
                    |cargo_desc| match cargo_desc_from_db_map.get(&cargo_desc.id) {
                        Some(cargo_desc_from_db) => cargo_desc_from_db != *cargo_desc,
                        None => true,
                    },
                )
                .map(|cargo_desc| cargo_desc.clone().into_active_model())
                .collect::<Vec<cargo_desc::ActiveModel>>();

            if things_to_insert.is_empty() {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} cargo_desc", things_to_insert.len());
            }

            for cargo_desc in &things_to_insert {
                let cargo_desc_in = cargo_desc_to_delete
                    .iter()
                    .position(|id| id == cargo_desc.id.as_ref());
                if let Some(cargo_desc_in) = cargo_desc_in {
                    cargo_desc_to_delete.remove(cargo_desc_in);
                }
            }

            let _ = cargo_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if !buffer_before_insert.is_empty() {
        let cargo_desc_from_db = cargo_desc::Entity::find()
            .filter(
                cargo_desc::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|cargo_desc| cargo_desc.id)
                        .collect::<Vec<i32>>(),
                ),
            )
            .all(conn)
            .await?;

        let cargo_desc_from_db_map = cargo_desc_from_db
            .into_iter()
            .map(|cargo_desc| (cargo_desc.id, cargo_desc))
            .collect::<HashMap<i32, cargo_desc::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(
                |cargo_desc| match cargo_desc_from_db_map.get(&cargo_desc.id) {
                    Some(cargo_desc_from_db) => cargo_desc_from_db != *cargo_desc,
                    None => true,
                },
            )
            .map(|cargo_desc| cargo_desc.clone().into_active_model())
            .collect::<Vec<cargo_desc::ActiveModel>>();

        if things_to_insert.is_empty() {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} cargo_desc", things_to_insert.len());
            cargo_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("cargo_desc last batch imported");
    }
    info!(
        "Importing cargo_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if !cargo_desc_to_delete.is_empty() {
        info!("cargo_desc's to delete: {:?}", cargo_desc_to_delete);
        cargo_desc::Entity::delete_many()
            .filter(cargo_desc::Column::Id.is_in(cargo_desc_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn load_cargo_description_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<cargo_desc::Model>> {
    let cargo_descriptions: Vec<cargo_desc::Model> = {
        let item_file = File::open(storage_path.join("Desc/CargoDesc.json"))?;
        let cargo_description: Value = serde_json::from_reader(&item_file)?;

        serde_json::from_value(
            cargo_description
                .get(0)
                .unwrap()
                .get("rows")
                .unwrap()
                .clone(),
        )?
    };

    Ok(cargo_descriptions)
}

pub(crate) async fn load_cargo_description_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/v1/database/{database}/sql"))
        .body("SELECT * FROM cargo_desc")
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

pub(crate) async fn load_desc_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let cargo_descriptions =
        load_cargo_description_from_spacetimedb(client, domain, protocol, database).await?;

    import_cargo_description(conn, cargo_descriptions, None).await?;

    Ok(())
}

pub async fn import_job_cargo_desc(temp_config: Config) {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60 * 60));

            import_interal_cargo_desc(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_interal_cargo_desc(config.clone(), conn, client);
    }
}

fn import_interal_cargo_desc(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let cargo_desc = load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_cargo_desc) = cargo_desc {
                    info!("CargoDesc imported");
                } else {
                    error!("CargoDesc import failed: {:?}", cargo_desc);
                }
            });
    });
}

async fn get_known_cargo_desc_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_cargo_desc_ids: Vec<i64> = cargo_desc::Entity::find()
        .select_only()
        .column(cargo_desc::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;

    let known_cargo_desc_ids = known_cargo_desc_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_cargo_desc_ids)
}

async fn db_insert_cargo_descs(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<cargo_desc::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let cargo_descs_from_db = cargo_desc::Entity::find()
        .filter(
            cargo_desc::Column::Id.is_in(
                buffer_before_insert
                    .iter()
                    .map(|cargo_desc| cargo_desc.id)
                    .collect::<Vec<i32>>(),
            ),
        )
        .all(conn)
        .await?;

    let cargo_descs_from_db_map = cargo_descs_from_db
        .into_iter()
        .map(|cargo_desc| (cargo_desc.id, cargo_desc))
        .collect::<HashMap<i32, cargo_desc::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(
            |cargo_desc| match cargo_descs_from_db_map.get(&cargo_desc.id) {
                Some(cargo_desc_from_db) => cargo_desc_from_db != *cargo_desc,
                None => true,
            },
        )
        .map(|cargo_desc| cargo_desc.clone().into_active_model())
        .collect::<Vec<cargo_desc::ActiveModel>>();

    if things_to_insert.is_empty() {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
        return Ok(());
    } else {
        debug!("Inserting {} cargo_descs", things_to_insert.len());
    }

    let _ = cargo_desc::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();
    Ok(())
}

async fn delete_cargo_desc(
    conn: &DatabaseConnection,
    known_cargo_desc_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "cargo_desc's ({}) to delete: {:?}",
        known_cargo_desc_ids.len(),
        known_cargo_desc_ids
    );
    cargo_desc::Entity::delete_many()
        .filter(cargo_desc::Column::Id.is_in(known_cargo_desc_ids))
        .exec(conn)
        .await?;
    Ok(())
}
//
// pub(crate) async fn handle_initial_subscription_cargo_desc(
//     app_state: &Arc<AppState>,
//     p1: &Table,
// ) -> anyhow::Result<()> {
//     let chunk_size = 5000;
//     let mut buffer_before_insert: Vec<cargo_desc::Model> = Vec::with_capacity(chunk_size);
//
//     let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
//         .update_columns([
//             cargo_desc::Column::Name,
//             cargo_desc::Column::Description,
//             cargo_desc::Column::Volume,
//             cargo_desc::Column::SecondaryKnowledgeId,
//             cargo_desc::Column::ModelAssetName,
//             cargo_desc::Column::IconAssetName,
//             cargo_desc::Column::CarriedModelAssetName,
//             cargo_desc::Column::PickUpAnimationStart,
//             cargo_desc::Column::PickUpAnimationEnd,
//             cargo_desc::Column::DropAnimationStart,
//             cargo_desc::Column::DropAnimationEnd,
//             cargo_desc::Column::PickUpTime,
//             cargo_desc::Column::PlaceTime,
//             cargo_desc::Column::AnimatorState,
//             cargo_desc::Column::MovementModifier,
//             cargo_desc::Column::BlocksPath,
//             cargo_desc::Column::OnDestroyYieldCargos,
//             cargo_desc::Column::DespawnTime,
//             cargo_desc::Column::Tier,
//             cargo_desc::Column::Tag,
//             cargo_desc::Column::Rarity,
//             cargo_desc::Column::NotPickupable,
//         ])
//         .to_owned();
//
//     let mut known_cargo_desc_ids = get_known_cargo_desc_ids(&app_state.conn).await?;
//     for update in p1.updates.iter() {
//         for row in update.inserts.iter() {
//             match serde_json::from_str::<cargo_desc::Model>(row.as_ref()) {
//                 Ok(cargo_desc) => {
//                     if known_cargo_desc_ids.contains(&cargo_desc.id) {
//                         known_cargo_desc_ids.remove(&cargo_desc.id);
//                     }
//                     app_state
//                         .cargo_desc
//                         .insert(cargo_desc.id, cargo_desc.clone());
//                     app_state.cargo_tiers.insert(cargo_desc.tier as i64);
//                     app_state.cargo_tags.insert(cargo_desc.tag.clone());
//                     buffer_before_insert.push(cargo_desc);
//                     if buffer_before_insert.len() == chunk_size {
//                         info!("CargoDesc insert");
//                         db_insert_cargo_descs(
//                             &app_state.conn,
//                             &mut buffer_before_insert,
//                             &on_conflict,
//                         )
//                         .await?;
//                     }
//                 }
//                 Err(error) => {
//                     error!(
//                         "TransactionUpdate Insert CargoDesc Error: {error} -> {:?}",
//                         row
//                     );
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         info!("CargoDesc insert");
//         db_insert_cargo_descs(&app_state.conn, &mut buffer_before_insert, &on_conflict).await?;
//     }
//
//     if !known_cargo_desc_ids.is_empty() {
//         for cargo_desc_id in &known_cargo_desc_ids {
//             app_state.cargo_desc.remove(cargo_desc_id);
//         }
//         delete_cargo_desc(&app_state.conn, known_cargo_desc_ids).await?;
//     }
//
//     Ok(())
// }
//
// pub(crate) async fn handle_transaction_update_cargo_desc(
//     app_state: &Arc<AppState>,
//     tables: &[TableWithOriginalEventTransactionUpdate],
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
//         .update_columns([
//             cargo_desc::Column::Name,
//             cargo_desc::Column::Description,
//             cargo_desc::Column::Volume,
//             cargo_desc::Column::SecondaryKnowledgeId,
//             cargo_desc::Column::ModelAssetName,
//             cargo_desc::Column::IconAssetName,
//             cargo_desc::Column::CarriedModelAssetName,
//             cargo_desc::Column::PickUpAnimationStart,
//             cargo_desc::Column::PickUpAnimationEnd,
//             cargo_desc::Column::DropAnimationStart,
//             cargo_desc::Column::DropAnimationEnd,
//             cargo_desc::Column::PickUpTime,
//             cargo_desc::Column::PlaceTime,
//             cargo_desc::Column::AnimatorState,
//             cargo_desc::Column::MovementModifier,
//             cargo_desc::Column::BlocksPath,
//             cargo_desc::Column::OnDestroyYieldCargos,
//             cargo_desc::Column::DespawnTime,
//             cargo_desc::Column::Tier,
//             cargo_desc::Column::Tag,
//             cargo_desc::Column::Rarity,
//             cargo_desc::Column::NotPickupable,
//         ])
//         .to_owned();
//
//     let chunk_size = 5000;
//     let mut buffer_before_insert = HashMap::new();
//
//     let mut found_in_inserts = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.inserts.iter() {
//             match serde_json::from_str::<cargo_desc::Model>(row.as_ref()) {
//                 Ok(cargo_desc) => {
//                     app_state
//                         .cargo_desc
//                         .insert(cargo_desc.id, cargo_desc.clone());
//                     app_state.cargo_tiers.insert(cargo_desc.tier as i64);
//                     app_state.cargo_tags.insert(cargo_desc.tag.clone());
//                     found_in_inserts.insert(cargo_desc.id);
//                     buffer_before_insert.insert(cargo_desc.id, cargo_desc);
//
//                     if buffer_before_insert.len() == chunk_size {
//                         let mut buffer_before_insert_vec = buffer_before_insert
//                             .clone()
//                             .into_iter()
//                             .map(|x| x.1)
//                             .collect::<Vec<cargo_desc::Model>>();
//
//                         db_insert_cargo_descs(
//                             &app_state.conn,
//                             &mut buffer_before_insert_vec,
//                             &on_conflict,
//                         )
//                         .await?;
//                         buffer_before_insert.clear();
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Insert CargoDesc Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if !buffer_before_insert.is_empty() {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<cargo_desc::Model>>();
//
//         db_insert_cargo_descs(&app_state.conn, &mut buffer_before_insert_vec, &on_conflict).await?;
//         buffer_before_insert.clear();
//     }
//
//     let mut cargo_descs_to_delete = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.deletes.iter() {
//             match serde_json::from_str::<cargo_desc::Model>(row.as_ref()) {
//                 Ok(cargo_desc) => {
//                     if !found_in_inserts.contains(&cargo_desc.id) {
//                         app_state.cargo_desc.remove(&cargo_desc.id);
//                         cargo_descs_to_delete.insert(cargo_desc.id);
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Delete CargoDesc Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if !cargo_descs_to_delete.is_empty() {
//         delete_cargo_desc(&app_state.conn, cargo_descs_to_delete).await?;
//     }
//
//     Ok(())
// }
