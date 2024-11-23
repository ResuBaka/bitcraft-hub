use crate::{buildings, AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Router;
use axum_codec::Codec;
use entity::{building_desc, building_state};
use log::{debug, error, info};
use reqwest::Client;
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
};
use serde::Deserialize;
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;
use crate::config::Config;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/buildings",
            axum_codec::routing::get(find_building_states).into(),
        )
        .route(
            "/api/bitcraft/buildings",
            axum_codec::routing::get(find_building_states).into(),
        )
        .route(
            "/buildings/:id",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/buildings/:id",
            axum_codec::routing::get(find_building_state).into(),
        )
        .route(
            "/api/bitcraft/desc/buildings",
            axum_codec::routing::get(find_building_descriptions).into(),
        )
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct BuildingDescriptionsResponse {
    buildings: Vec<building_desc::ApiResponse>,
    per_page: u64,
    total: u64,
    page: u64,
}

pub(crate) async fn find_building_descriptions(
    state: State<AppState>,
    Query(params): Query<Params>,
) -> Result<Codec<BuildingDescriptionsResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);
    let search = params.search;

    let posts = QueryCore::find_building_descs(&state.conn, page, per_page, search)
        .await
        .expect("Cannot find posts in page");

    let building_ids = posts.0.iter().map(|x| x.id).collect::<Vec<i64>>();
    let building_states = QueryCore::find_building_state_by_desc_ids(&state.conn, building_ids)
        .await
        .expect("Cannot find building states");

    let buildings = posts
        .0
        .into_iter()
        .map(|x| building_desc::ApiResponse {
            id: x.id,
            functions: x.functions,
            name: x.name,
            description: x.description,
            rested_buff_duration: x.rested_buff_duration,
            light_radius: x.light_radius,
            model_asset_name: x.model_asset_name,
            icon_asset_name: x.icon_asset_name,
            unenterable: x.unenterable,
            wilderness: x.wilderness,
            footprint: x.footprint,
            max_health: x.max_health,
            decay: x.decay,
            maintenance: x.maintenance,
            interaction_level: x.interaction_level,
            has_action: x.has_action,
            show_in_compendium: x.show_in_compendium,
            is_ruins: x.is_ruins,
            count: building_states
                .iter()
                .filter(|building_state| building_state.building_description_id as i64 == (x.id))
                .count() as i32,
        })
        .collect::<Vec<building_desc::ApiResponse>>();

    return Ok(Codec(BuildingDescriptionsResponse {
        buildings,
        per_page,
        total: posts.1.number_of_items,
        page,
    }));
}

pub(crate) async fn find_claim_description(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<building_desc::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_desc_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingDesc not found"));
    }

    return Ok(Codec(posts.unwrap()));
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct BuildingStatesResponse {
    buildings: Vec<building_state::Model>,
    per_page: u64,
    total: u64,
    page: u64,
}

#[derive(Deserialize)]
pub(crate) struct BuildingStatesParams {
    page: Option<u64>,
    per_page: Option<u64>,
    claim_entity_id: Option<i64>,
    with_inventory: Option<bool>,
}

pub(crate) async fn find_building_states(
    state: State<AppState>,
    Query(params): Query<BuildingStatesParams>,
) -> Result<Codec<BuildingStatesResponse>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(30);
    let search = params.claim_entity_id;
    let with_inventory = params.with_inventory.unwrap_or(false);
    let mut buildings_with_inventory_ids = None;

    if with_inventory {
        let buildings_with_inventory = QueryCore::find_building_descs_with_inventory(&state.conn)
            .await
            .expect("Cannot find posts in page");

        buildings_with_inventory_ids = Some(
            buildings_with_inventory
                .iter()
                .map(|building| building.id)
                .collect(),
        );
    }

    let posts = QueryCore::find_building_states(
        &state.conn,
        page,
        per_page,
        search,
        buildings_with_inventory_ids,
    )
    .await
    .expect("Cannot find posts in page");

    return Ok(Codec(BuildingStatesResponse {
        buildings: posts.0,
        per_page,
        total: posts.1.number_of_items,
        page,
    }));
}

pub(crate) async fn find_building_state(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<building_state::Model>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_building_state_by_id(&state.conn, id as i64)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "BuildingState not found"));
    }

    return Ok(Codec(posts.unwrap()));
}

pub(crate) async fn load_building_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<building_state::Model>> {
    let building_states: Vec<building_state::Model> = {
        let item_file = File::open(storage_path.join("State/BuildingState.json"))?;
        let building_state: Value = serde_json::from_reader(&item_file)?;

        serde_json::from_value(building_state.get(0).unwrap().get("rows").unwrap().clone())?
    };

    Ok(building_states)
}

pub(crate) async fn load_building_state_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM BuildingState")
        .send()
        .await;

    let json = match response {
        Ok(response) => response.text().await?,
        Err(error) => {
            error!("Error: {error:?}");
            return Ok("".into());
        }
    };

    Ok(json)
}

pub(crate) async fn load_state_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let building_descs =
        load_building_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_building_state(&conn, building_descs, None).await?;

    Ok(())
}

pub(crate) async fn import_building_state(
    conn: &DatabaseConnection,
    building_states: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<building_state::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(building_states.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(building_state::Column::EntityId)
        .update_columns([
            building_state::Column::ClaimEntityId,
            building_state::Column::DirectionIndex,
            building_state::Column::BuildingDescriptionId,
            building_state::Column::ConstructedByPlayerEntityId,
            building_state::Column::Nickname,
        ])
        .to_owned();

    let mut building_state_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<building_state::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let building_state_from_db = building_state::Entity::find()
                .filter(
                    building_state::Column::EntityId.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|building_state| building_state.entity_id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if building_state_from_db.len() != buffer_before_insert.len() {
                building_state_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|building_state| {
                            !building_state_from_db.iter().any(|building_state_from_db| {
                                building_state_from_db.entity_id == building_state.entity_id
                            })
                        })
                        .map(|building_state| building_state.entity_id),
                );
            }

            let building_state_from_db_map = building_state_from_db
                .into_iter()
                .map(|building_state| (building_state.entity_id, building_state))
                .collect::<HashMap<i64, building_state::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|building_state| {
                    match building_state_from_db_map.get(&building_state.entity_id) {
                        Some(building_state_from_db) => {
                            if building_state_from_db != *building_state {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|building_state| building_state.clone().into_active_model())
                .collect::<Vec<building_state::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} building_state", things_to_insert.len());
            }

            for building_state in &things_to_insert {
                let building_state_in = building_state_to_delete
                    .iter()
                    .position(|id| id == building_state.entity_id.as_ref());
                if building_state_in.is_some() {
                    building_state_to_delete.remove(building_state_in.unwrap());
                }
            }

            let _ = building_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let building_state_from_db = building_state::Entity::find()
            .filter(
                building_state::Column::EntityId.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|building_state| building_state.entity_id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let building_state_from_db_map = building_state_from_db
            .into_iter()
            .map(|building_state| (building_state.entity_id, building_state))
            .collect::<HashMap<i64, building_state::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|building_state| {
                match building_state_from_db_map.get(&building_state.entity_id) {
                    Some(building_state_from_db) => {
                        if building_state_from_db != *building_state {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|building_state| building_state.clone().into_active_model())
            .collect::<Vec<building_state::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} building_state", things_to_insert.len());
            building_state::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("building_state last batch imported");
    }
    info!(
        "Importing building_state finished in {}s",
        start.elapsed().as_secs()
    );

    if building_state_to_delete.len() > 0 {
        info!("building_state's to delete: {:?}", building_state_to_delete);
        building_state::Entity::delete_many()
            .filter(building_state::Column::EntityId.is_in(building_state_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) async fn load_building_desc_from_spacetimedb(
    client: &Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM BuildingDesc")
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
    let building_descs =
        load_building_desc_from_spacetimedb(client, domain, protocol, database).await?;

    import_building_desc(&conn, building_descs, None).await?;

    Ok(())
}

pub(crate) async fn load_building_desc_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<building_desc::Model>> {
    let building_descs: Vec<building_desc::Model> = {
        let item_file = File::open(storage_path.join("Desc/BuildingDesc.json"))?;
        let building_desc: Value = serde_json::from_reader(&item_file)?;

        serde_json::from_value(building_desc.get(0).unwrap().get("rows").unwrap().clone())?
    };

    Ok(building_descs)
}

pub(crate) async fn import_building_desc(
    conn: &DatabaseConnection,
    building_descs: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<building_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(building_descs.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(building_desc::Column::Id)
        .update_columns([
            building_desc::Column::Functions,
            building_desc::Column::Name,
            building_desc::Column::Description,
            building_desc::Column::RestedBuffDuration,
            building_desc::Column::LightRadius,
            building_desc::Column::ModelAssetName,
            building_desc::Column::IconAssetName,
            building_desc::Column::Unenterable,
            building_desc::Column::Wilderness,
            building_desc::Column::Footprint,
            building_desc::Column::MaxHealth,
            building_desc::Column::Decay,
            building_desc::Column::Maintenance,
            building_desc::Column::InteractionLevel,
            building_desc::Column::HasAction,
            building_desc::Column::ShowInCompendium,
            building_desc::Column::IsRuins,
        ])
        .to_owned();

    let mut buildings_desc_to_delete = Vec::new();

    while let Ok(value) = json_stream_reader.deserialize_next::<building_desc::Model>() {
        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let buildings_desc_from_db = building_desc::Entity::find()
                .filter(
                    building_desc::Column::Id.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|buildings_desc| buildings_desc.id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            if buildings_desc_from_db.len() != buffer_before_insert.len() {
                buildings_desc_to_delete.extend(
                    buffer_before_insert
                        .iter()
                        .filter(|buildings_desc| {
                            !buildings_desc_from_db.iter().any(|buildings_desc_from_db| {
                                buildings_desc_from_db.id == buildings_desc.id
                            })
                        })
                        .map(|buildings_desc| buildings_desc.id),
                );
            }

            let buildings_desc_from_db_map = buildings_desc_from_db
                .into_iter()
                .map(|buildings_desc| (buildings_desc.id, buildings_desc))
                .collect::<HashMap<i64, building_desc::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|buildings_desc| {
                    match buildings_desc_from_db_map.get(&buildings_desc.id) {
                        Some(buildings_desc_from_db) => {
                            if buildings_desc_from_db != *buildings_desc {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|buildings_desc| buildings_desc.clone().into_active_model())
                .collect::<Vec<building_desc::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} buildings_desc", things_to_insert.len());
            }

            for buildings_desc in &things_to_insert {
                let buildings_desc_in = buildings_desc_to_delete
                    .iter()
                    .position(|id| id == buildings_desc.id.as_ref());
                if buildings_desc_in.is_some() {
                    buildings_desc_to_delete.remove(buildings_desc_in.unwrap());
                }
            }

            let _ = building_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let buildings_desc_from_db = building_desc::Entity::find()
            .filter(
                building_desc::Column::Id.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|buildings_desc| buildings_desc.id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let buildings_desc_from_db_map = buildings_desc_from_db
            .into_iter()
            .map(|buildings_desc| (buildings_desc.id, buildings_desc))
            .collect::<HashMap<i64, building_desc::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|buildings_desc| {
                match buildings_desc_from_db_map.get(&buildings_desc.id) {
                    Some(buildings_desc_from_db) => {
                        if buildings_desc_from_db != *buildings_desc {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|buildings_desc| buildings_desc.clone().into_active_model())
            .collect::<Vec<building_desc::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} buildings_desc", things_to_insert.len());
            building_desc::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("Buildings_desc last batch imported");
    }
    info!(
        "Importing buildings_desc finished in {}s",
        start.elapsed().as_secs()
    );

    if buildings_desc_to_delete.len() > 0 {
        info!("Buildings_desc's to delete: {:?}", buildings_desc_to_delete);
        building_desc::Entity::delete_many()
            .filter(building_desc::Column::Id.is_in(buildings_desc_to_delete))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub async fn import_job_building_desc(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = super::create_importer_default_db_connection(config.clone()).await;
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_building_state(config.clone(), conn, client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_building_state(config.clone(), conn, client);
    }
}

pub async fn import_job_building_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        loop {
            let conn = super::create_importer_default_db_connection(config.clone()).await;
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_building_desc(config.clone(), conn, client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_building_desc(config.clone(), conn, client);
    }
}

fn import_internal_building_state(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let building_state = load_state_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                    .await;

                if let Ok(_building_state) = building_state {
                    info!("BuildingState imported");
                } else {
                    error!("BuildingState import failed: {:?}", building_state);
                }
            });
    });
}

fn import_internal_building_desc(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let building_desc = load_desc_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                    .await;

                if let Ok(_building_desc) = building_desc {
                    info!("BuildingDesc imported");
                } else {
                    error!("BuildingDesc import failed: {:?}", building_desc);
                }
            });
    });
}