use entity::cargo_description;
use log::{debug, error, info};
use migration::sea_query;
use reqwest::Client;
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

pub(crate) async fn import_cargo_description(
    conn: &DatabaseConnection,
    cargo_descriptions: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<cargo_description::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(cargo_descriptions.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(cargo_description::Column::Id)
        .update_columns([
            cargo_description::Column::Name,
            cargo_description::Column::Description,
            cargo_description::Column::Volume,
            cargo_description::Column::SecondaryKnowledgeId,
            cargo_description::Column::ModelAssetName,
            cargo_description::Column::IconAssetName,
            cargo_description::Column::CarriedModelAssetName,
            cargo_description::Column::PickUpAnimationStart,
            cargo_description::Column::PickUpAnimationEnd,
            cargo_description::Column::DropAnimationStart,
            cargo_description::Column::DropAnimationEnd,
            cargo_description::Column::PickUpTime,
            cargo_description::Column::PlaceTime,
            cargo_description::Column::AnimatorState,
            cargo_description::Column::MovementModifier,
            cargo_description::Column::BlocksPath,
            cargo_description::Column::OnDestroyYieldCargos,
            cargo_description::Column::DespawnTime,
            cargo_description::Column::Tier,
            cargo_description::Column::Tag,
            cargo_description::Column::Rarity,
        ])
        .to_owned();

    let mut cargo_desc_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<cargo_description::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let cargo_desc_from_db = cargo_description::Entity::find()
                .filter(
                    cargo_description::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|cargo_desc| cargo_desc.id)
                            .collect::<Vec<i64>>(),
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
                .collect::<HashMap<i64, cargo_description::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|cargo_desc| {
                    match cargo_desc_from_db_map.get(&cargo_desc.id) {
                        Some(cargo_desc_from_db) => {
                            if cargo_desc_from_db != *cargo_desc {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|cargo_desc| cargo_desc.clone().into_active_model())
                .collect::<Vec<cargo_description::ActiveModel>>();

            if things_to_insert.len() == 0 {
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
                if cargo_desc_in.is_some() {
                    cargo_desc_to_delete.remove(cargo_desc_in.unwrap());
                }
            }

            let _ = cargo_description::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let cargo_desc_from_db = cargo_description::Entity::find()
            .filter(
                cargo_description::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|cargo_desc| cargo_desc.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let cargo_desc_from_db_map = cargo_desc_from_db
            .into_iter()
            .map(|cargo_desc| (cargo_desc.id, cargo_desc))
            .collect::<HashMap<i64, cargo_description::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|cargo_desc| {
                match cargo_desc_from_db_map.get(&cargo_desc.id) {
                    Some(cargo_desc_from_db) => {
                        if cargo_desc_from_db != *cargo_desc {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|cargo_desc| cargo_desc.clone().into_active_model())
            .collect::<Vec<cargo_description::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} cargo_desc", things_to_insert.len());
            cargo_description::Entity::insert_many(things_to_insert)
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

    if cargo_desc_to_delete.len() > 0 {
        info!("cargo_desc's to delete: {:?}", cargo_desc_to_delete);
        cargo_description::Entity::delete_many()
            .filter(cargo_description::Column::Id.is_in(cargo_desc_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) async fn load_cargo_description_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<cargo_description::Model>> {
    let cargo_descriptions: Vec<cargo_description::Model> = {
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
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM CargoDesc")
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

    import_cargo_description(&conn, cargo_descriptions, None).await?;

    Ok(())
}
