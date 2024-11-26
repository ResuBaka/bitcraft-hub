use crate::config::Config;
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Router;
use axum_codec::Codec;
use entity::inventory::{
    Content, ExpendedRefrence, ItemExpended, ItemSlotResolved, ItemType, ResolvedInventory,
};
use entity::{cargo_desc, inventory, item_desc};
use log::{debug, error, info};
use reqwest::Client;
use sea_orm::sqlx::Encode;
use sea_orm::{
    sea_query, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityName, EntityTrait,
    IntoActiveModel, QueryFilter, QuerySelect,
};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Add;
use std::path::PathBuf;
use std::time::Duration;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};
use tokio::time::Instant;

pub(crate) fn get_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/inventorys/changes/:id",
            axum_codec::routing::get(read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/changes/:id",
            axum_codec::routing::get(read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/owner_entity_id/:id",
            axum_codec::routing::get(find_inventory_by_owner_entity_id).into(),
        )
        .route(
            "/inventory/:id",
            axum_codec::routing::get(find_inventory_by_id).into(),
        )
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct InventoryChanged {
    inventory_id: i64,
    identity: String,
    player_name: Option<String>,
    player_entity_id: Option<i64>,
    timestamp: i64,
    created: Option<Value>,
    deleted: Option<Value>,
    diff: Option<HashMap<i64, HashMap<String, Option<ExpendedRefrence>>>>,
}

pub(crate) async fn read_inventory_changes(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Codec<Vec<InventoryChanged>>, (StatusCode, &'static str)> {
    let mut inventory_changes = vec![];

    let inventory_chages_file =
        File::open(state.storage_path.join(format!("Inventory/{}.json", id)));

    match inventory_chages_file {
        Ok(file) => {
            for line in BufReader::new(file).lines() {
                let line = line.unwrap();
                match serde_json::from_str(&line) {
                    Ok(data) => {
                        inventory_changes.push(data);
                    }
                    Err(e) => {
                        error!("Error: {e}, line: {line}");
                    }
                };
            }

            Ok(Codec(inventory_changes))
        }
        Err(_e) => Err((StatusCode::NOT_FOUND, "InventoryChanged not found")),
    }
}

pub(crate) async fn find_inventory_by_id(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Codec<inventory::Model>, (StatusCode, &'static str)> {
    let inventory = QueryCore::find_inventory_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    match inventory {
        Some(inventory) => Ok(Codec(inventory)),
        None => Err((StatusCode::NOT_FOUND, "Inventory not found")),
    }
}

#[axum_codec::apply(encode, decode)]
pub(crate) struct InventorysResponse {
    inventorys: Vec<ResolvedInventory>,
    total: i64,
    page: i64,
    #[serde(rename = "perPage")]
    per_page: i64,
}

pub(crate) async fn find_inventory_by_owner_entity_id(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Codec<InventorysResponse>, (StatusCode, &'static str)> {
    let mut inventory_ids = vec![id];
    let player = QueryCore::find_player_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let mut mobile_entiety_map: HashMap<i64, String> = HashMap::new();

    match &player {
        Some(player) => {
            let mobile_entiety_from_player =
                QueryCore::find_deployable_entity_by_owner_entity_id(&state.conn, player.entity_id)
                    .await
                    .map_err(|e| {
                        error!("Error: {:?}", e);

                        (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
                    })?;

            for mobile_entiety in mobile_entiety_from_player {
                mobile_entiety_map.insert(mobile_entiety.entity_id, mobile_entiety.nickname);
                inventory_ids.push(mobile_entiety.entity_id);
            }

            ()
        }
        None => (),
    };

    let (inventorys, num_pages) =
        QueryCore::find_inventory_by_owner_entity_ids(&state.conn, inventory_ids)
            .await
            .map_err(|e| {
                error!("Error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
            })?;

    let mut item_ids = vec![];
    let mut cargo_ids = vec![];

    for inventory in &inventorys {
        for pocket in &inventory.pockets {
            if pocket.contents.len() == 0 {
                continue;
            }

            for (_item_id, refrence) in pocket.clone().contents.iter() {
                if refrence.is_some() {
                    let refrence = refrence.clone().unwrap();

                    let content = serde_json::from_value::<Content>(refrence);

                    if content.is_ok() {
                        let content = content.unwrap();

                        let item_type = content
                            .item_type
                            .as_object()
                            .unwrap()
                            .keys()
                            .next()
                            .unwrap();

                        if item_type == "0" {
                            item_ids.push(content.item_id.clone());
                        } else {
                            cargo_ids.push(content.item_id.clone());
                        }
                    }
                }
            }
        }
    }

    let mut resolved_inventory = vec![];

    let item_descs = QueryCore::find_item_by_ids(&state.conn, item_ids)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let cargo_descs = QueryCore::find_cargo_by_ids(&state.conn, cargo_ids)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let cargo_descs_map = cargo_descs
        .into_iter()
        .map(|cargo| (cargo.id, cargo))
        .collect::<HashMap<i64, cargo_desc::Model>>();
    let item_descs_map = item_descs
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<i64, item_desc::Model>>();

    for (index, inventory) in inventorys.into_iter().enumerate() {
        let mut pockets = vec![];

        for pocket in &inventory.pockets {
            pockets.push(resolve_pocket(pocket, &item_descs_map, &cargo_descs_map));
        }

        let nickname = match mobile_entiety_map.get(&inventory.owner_entity_id) {
            Some(nickname) => Some(nickname.clone()),
            None => match player.is_some() {
                true => {
                    let player = player.as_ref().unwrap();
                    if inventory.owner_entity_id == player.entity_id
                        && inventory.inventory_index == 0
                    {
                        Some(String::from("Inventory"))
                    } else if inventory.owner_entity_id == player.entity_id
                        && inventory.inventory_index == 2
                    {
                        Some(String::from("Wallet"))
                    } else if inventory.owner_entity_id == player.entity_id
                        && inventory.inventory_index == 1
                    {
                        Some(String::from("Tool belt"))
                    } else {
                        None
                    }
                }
                false => None,
            },
        };

        resolved_inventory.push(ResolvedInventory {
            entity_id: inventory.entity_id,
            pockets,
            inventory_index: inventory.inventory_index,
            cargo_index: inventory.cargo_index,
            owner_entity_id: inventory.owner_entity_id,
            player_owner_entity_id: inventory.player_owner_entity_id,
            nickname,
        });
    }

    resolved_inventory.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));

    Ok(Codec(InventorysResponse {
        inventorys: resolved_inventory,
        total: num_pages.number_of_items as i64,
        page: 1,
        per_page: 24,
    }))
}

#[allow(dead_code)]
pub(crate) async fn load_inventory_state_from_file(
    storage_path: &PathBuf,
) -> anyhow::Result<Vec<inventory::Model>> {
    let item_file = File::open(storage_path.join("State/InventoryState.json"))?;
    let inventory: Value = serde_json::from_reader(&item_file)?;
    let inventory: Vec<inventory::Model> =
        serde_json::from_value(inventory.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(inventory)
}

pub(crate) async fn load_inventory_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
) -> anyhow::Result<String> {
    let response = client
        .post(format!("{protocol}{domain}/database/sql/{database}"))
        .body("SELECT * FROM InventoryState")
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

pub(crate) async fn load_state_from_spacetimedb(
    client: &reqwest::Client,
    domain: &str,
    protocol: &str,
    database: &str,
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let claim_descriptions =
        load_inventory_state_from_spacetimedb(client, domain, protocol, database).await?;

    import_inventory(&conn, claim_descriptions, Some(3000)).await?;

    Ok(())
}

pub(crate) async fn import_inventory(
    conn: &DatabaseConnection,
    inventorys: String,
    chunk_size: Option<usize>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    let mut buffer_before_insert: Vec<inventory::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let mut json_stream_reader = JsonStreamReader::new(inventorys.as_bytes());

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"])?;
    json_stream_reader.begin_array()?;

    let on_conflict = sea_query::OnConflict::column(inventory::Column::EntityId)
        .update_columns([
            inventory::Column::Pockets,
            inventory::Column::InventoryIndex,
            inventory::Column::CargoIndex,
            inventory::Column::OwnerEntityId,
        ])
        .to_owned();

    let known_inventory_ids: Vec<i64> = inventory::Entity::find()
        .select_only()
        .column(inventory::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let mut known_inventory_ids = known_inventory_ids.into_iter().collect::<HashSet<i64>>();

    while let Ok(value) = json_stream_reader.deserialize_next::<inventory::Model>() {
        if known_inventory_ids.contains(&value.entity_id) {
            known_inventory_ids.remove(&value.entity_id);
        }

        buffer_before_insert.push(value);

        if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
            let inventorys_from_db = inventory::Entity::find()
                .filter(
                    inventory::Column::EntityId.is_in(
                        buffer_before_insert
                            .iter()
                            .map(|inventory| inventory.entity_id)
                            .collect::<Vec<i64>>(),
                    ),
                )
                .all(conn)
                .await?;

            let inventorys_from_db_map = inventorys_from_db
                .into_iter()
                .map(|inventory| (inventory.entity_id, inventory))
                .collect::<HashMap<i64, inventory::Model>>();

            let things_to_insert = buffer_before_insert
                .iter()
                .filter(|inventory| {
                    match inventorys_from_db_map.get(&inventory.entity_id) {
                        Some(inventory_from_db) => {
                            if inventory_from_db != *inventory {
                                return true;
                            }
                        }
                        None => {
                            return true;
                        }
                    }

                    return false;
                })
                .map(|inventory| inventory.clone().into_active_model())
                .collect::<Vec<inventory::ActiveModel>>();

            if things_to_insert.len() == 0 {
                debug!("Nothing to insert");
                buffer_before_insert.clear();
                continue;
            } else {
                debug!("Inserting {} inventorys", things_to_insert.len());
            }

            let _ = inventory::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict.clone())
                .exec(conn)
                .await?;

            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        let inventorys_from_db = inventory::Entity::find()
            .filter(
                inventory::Column::EntityId.is_in(
                    buffer_before_insert
                        .iter()
                        .map(|inventory| inventory.entity_id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(conn)
            .await?;

        let inventorys_from_db_map = inventorys_from_db
            .into_iter()
            .map(|inventory| (inventory.entity_id, inventory))
            .collect::<HashMap<i64, inventory::Model>>();

        let things_to_insert = buffer_before_insert
            .iter()
            .filter(|inventory| {
                match inventorys_from_db_map.get(&inventory.entity_id) {
                    Some(inventory_from_db) => {
                        if inventory_from_db != *inventory {
                            return true;
                        }
                    }
                    None => {
                        return true;
                    }
                }

                return false;
            })
            .map(|inventory| inventory.clone().into_active_model())
            .collect::<Vec<inventory::ActiveModel>>();

        if things_to_insert.len() == 0 {
            debug!("Nothing to insert");
            buffer_before_insert.clear();
        } else {
            debug!("Inserting {} inventorys", things_to_insert.len());
            inventory::Entity::insert_many(things_to_insert)
                .on_conflict(on_conflict)
                .exec(conn)
                .await?;
        }

        buffer_before_insert.clear();
        info!("Inventory last batch imported");
    }
    info!(
        "Importing inventory finished in {}s",
        start.elapsed().as_secs()
    );

    if known_inventory_ids.len() > 0 {
        info!(
            "Inventory's ({}) to delete: {:?}",
            known_inventory_ids.len(),
            known_inventory_ids
        );
        inventory::Entity::delete_many()
            .filter(inventory::Column::EntityId.is_in(known_inventory_ids))
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) fn resolve_pocket(
    pocket: &inventory::ItemSlot,
    item_desc: &HashMap<i64, item_desc::Model>,
    cargo_desc: &HashMap<i64, cargo_desc::Model>,
) -> ItemSlotResolved {
    let mut contents = None;
    for (_, refrence) in pocket.clone().contents.iter() {
        contents = resolve_contents(refrence, item_desc, cargo_desc);
    }
    ItemSlotResolved {
        volume: pocket.volume,
        contents,
        locked: pocket.locked,
    }
}

pub(crate) fn resolve_contents(
    contents: &Option<sea_orm::JsonValue>,
    item_desc: &HashMap<i64, item_desc::Model>,
    cargo_desc: &HashMap<i64, cargo_desc::Model>,
) -> Option<ExpendedRefrence> {
    if contents.is_none() {
        return None;
    }

    let refrence = contents.clone().unwrap();

    let content = serde_json::from_value::<Content>(refrence.clone());

    if content.is_err() {
        return None;
    }

    let content = content.unwrap();

    let item_type = content
        .item_type
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();

    let key = content
        .durability
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();

    let durability = if key == "0" {
        let durability = content.durability.as_object().unwrap().get(key).unwrap();

        Some(durability.as_i64().unwrap())
    } else {
        None
    };

    if item_type == "0" {
        Some(ExpendedRefrence {
            item_id: content.item_id,
            item: ItemExpended::Item(item_desc.get(&content.item_id).unwrap().clone()),
            quantity: content.quantity,
            item_type: ItemType::Item,
            durability,
        })
    } else {
        Some(ExpendedRefrence {
            item_id: content.item_id,
            item: ItemExpended::Cargo(cargo_desc.get(&content.item_id).unwrap().clone()),
            quantity: content.quantity,
            item_type: ItemType::Cargo,
            durability,
        })
    }
}

pub async fn import_job_inventory_state(temp_config: Config) -> () {
    let config = temp_config.clone();
    if config.live_updates {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        loop {
            let client = super::create_default_client(config.clone());

            let now = Instant::now();
            let now_in = now.add(Duration::from_secs(60));

            import_internal_inventory(config.clone(), conn.clone(), client);

            let now = Instant::now();
            let wait_time = now_in.duration_since(now);

            if wait_time.as_secs() > 0 {
                tokio::time::sleep(wait_time).await;
            }
        }
    } else {
        let conn = super::create_importer_default_db_connection(config.clone()).await;
        let client = super::create_default_client(config.clone());

        import_internal_inventory(config.clone(), conn, client);
    }
}
fn import_internal_inventory(config: Config, conn: DatabaseConnection, client: Client) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let vehicle_state = load_state_from_spacetimedb(
                    &client,
                    &config.spacetimedb.domain,
                    &config.spacetimedb.protocol,
                    &config.spacetimedb.database,
                    &conn,
                )
                .await;

                if let Ok(_vehicle_state) = vehicle_state {
                    info!("Inventory imported");
                } else {
                    error!("Inventory import failed: {:?}", vehicle_state);
                }
            });
    });
}
