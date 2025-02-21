use crate::websocket::{Table, TableWithOriginalEventTransactionUpdate};
use crate::{AppRouter, AppState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use entity::inventory::{
    Content, ExpendedRefrence, ItemExpended, ItemSlotResolved, ItemType, Model, ResolvedInventory,
};
use entity::{cargo_desc, inventory, item_desc};
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route("/inventorys/changes/{id}", get(read_inventory_changes))
        .route(
            "/api/bitcraft/inventorys/changes/{id}",
            get(read_inventory_changes),
        )
        .route(
            "/api/bitcraft/inventorys/owner_entity_id/{id}",
            get(find_inventory_by_owner_entity_id),
        )
        .route("/inventory/{id}", get(find_inventory_by_id))
}

#[derive(Serialize, Deserialize)]
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
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<Vec<InventoryChanged>>, (StatusCode, &'static str)> {
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

            Ok(Json(inventory_changes))
        }
        Err(_e) => Err((StatusCode::NOT_FOUND, "InventoryChanged not found")),
    }
}

pub(crate) async fn find_inventory_by_id(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<inventory::Model>, (StatusCode, &'static str)> {
    let inventory = QueryCore::find_inventory_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    match inventory {
        Some(inventory) => Ok(Json(inventory)),
        None => Err((StatusCode::NOT_FOUND, "Inventory not found")),
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct InventorysResponse {
    inventorys: Vec<ResolvedInventory>,
    total: i64,
    page: i64,
    #[serde(rename = "perPage")]
    per_page: i64,
}

pub(crate) async fn find_inventory_by_owner_entity_id(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<InventorysResponse>, (StatusCode, &'static str)> {
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
            if pocket.contents.1.is_none() {
                continue;
            }

            let (_item_id, refrence) = pocket.clone().contents;
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

    for (_index, inventory) in inventorys.into_iter().enumerate() {
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

    Ok(Json(InventorysResponse {
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

async fn get_known_inventory_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i64>> {
    let known_inventory_ids: Vec<i64> = inventory::Entity::find()
        .select_only()
        .column(inventory::Column::EntityId)
        .into_tuple()
        .all(conn)
        .await?;

    let known_inventory_ids = known_inventory_ids.into_iter().collect::<HashSet<i64>>();
    Ok(known_inventory_ids)
}

async fn db_delete_inventorys(
    conn: &DatabaseConnection,
    known_inventory_ids: HashSet<i64>,
) -> anyhow::Result<()> {
    info!(
        "Inventory's ({}) to delete: {:?}",
        known_inventory_ids.len(),
        known_inventory_ids
    );
    inventory::Entity::delete_many()
        .filter(inventory::Column::EntityId.is_in(known_inventory_ids))
        .exec(conn)
        .await?;
    Ok(())
}

async fn db_insert_inventory_state(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
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
        return Ok(());
    } else {
        debug!("Inserting {} inventorys", things_to_insert.len());
    }

    let _ = inventory::Entity::insert_many(things_to_insert)
        .on_conflict(on_conflict.clone())
        .exec(conn)
        .await?;

    buffer_before_insert.clear();

    Ok(())
}

pub(crate) fn resolve_pocket(
    pocket: &inventory::ItemSlot,
    item_desc: &HashMap<i64, item_desc::Model>,
    cargo_desc: &HashMap<i64, cargo_desc::Model>,
) -> ItemSlotResolved {
    let mut contents = None;
    let (_, refrence) = pocket.clone().contents;
    contents = resolve_contents(&refrence, item_desc, cargo_desc);
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

pub(crate) async fn handle_initial_subscription(
    database_connection: &DatabaseConnection,
    table: &Table,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(inventory::Column::EntityId)
        .update_columns([
            inventory::Column::Pockets,
            inventory::Column::InventoryIndex,
            inventory::Column::CargoIndex,
            inventory::Column::OwnerEntityId,
            inventory::Column::PlayerOwnerEntityId,
        ])
        .to_owned();

    let chunk_size = Some(5000);
    let mut buffer_before_insert: Vec<inventory::Model> = vec![];

    let mut known_inventory_ids = get_known_inventory_ids(database_connection).await?;

    for update in table.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<inventory::Model>(row.as_ref()) {
                Ok(building_state) => {
                    if known_inventory_ids.contains(&building_state.entity_id) {
                        known_inventory_ids.remove(&building_state.entity_id);
                    }
                    buffer_before_insert.push(building_state);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                        db_insert_inventory_state(
                            database_connection,
                            &mut buffer_before_insert,
                            &on_conflict,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    error!("InitialSubscription Insert Inventory Error: {error:#?} -> {row}");
                }
            }
        }
    }
    if buffer_before_insert.len() > 0 {
        for buffer_chnk in buffer_before_insert.chunks(5000) {
            db_insert_inventory_state(database_connection, &mut buffer_chnk.to_vec(), &on_conflict)
                .await?;
        }
    }

    if known_inventory_ids.len() > 0 {
        db_delete_inventorys(database_connection, known_inventory_ids).await?;
    }

    Ok(())
}

pub(crate) async fn handle_transaction_update(
    database_connection: &DatabaseConnection,
    tables: &Vec<TableWithOriginalEventTransactionUpdate>,
) -> anyhow::Result<()> {
    let on_conflict = sea_query::OnConflict::column(inventory::Column::EntityId)
        .update_columns([
            inventory::Column::Pockets,
            inventory::Column::InventoryIndex,
            inventory::Column::CargoIndex,
            inventory::Column::OwnerEntityId,
            inventory::Column::PlayerOwnerEntityId,
        ])
        .to_owned();

    let mut buffer_before_insert = HashMap::new();
    let mut potential_deletes = HashSet::new();
    // let mut inventory_changes = vec![];

    for p1 in tables.iter() {
        let event_type = if p1.inserts.len() > 0 && p1.deletes.len() > 0 {
            "update"
        } else if p1.inserts.len() > 0 && p1.deletes.len() == 0 {
            "insert"
        } else if p1.deletes.len() > 0 && p1.inserts.len() == 0 {
            "delete"
        } else {
            "unknown"
        };

        if event_type == "unknown" {
            error!("Unknown event type {:?}", p1);
            continue;
        }

        if event_type == "delete" {
            for row in p1.deletes.iter() {
                match serde_json::from_str::<inventory::Model>(row.as_ref()) {
                    Ok(inventory) => {
                        potential_deletes.insert(inventory.entity_id);
                    }
                    Err(error) => {
                        error!("Event: {event_type} Error: {error} for row: {:?}", row);
                    }
                }
            }
        } else if event_type == "update" {
            let mut delete_parsed = HashMap::new();
            for row in p1.deletes.iter() {
                let parsed = serde_json::from_str::<inventory::Model>(row.as_ref());

                if parsed.is_err() {
                    error!(
                        "Could not parse delete inventory: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row
                    );
                } else {
                    let parsed = parsed.unwrap();
                    delete_parsed.insert(parsed.entity_id, parsed.clone());
                    potential_deletes.remove(&parsed.entity_id);
                }
            }

            for row in p1.inserts.iter().enumerate() {
                let parsed = serde_json::from_str::<inventory::Model>(row.1.as_ref());

                if parsed.is_err() {
                    error!(
                        "Could not parse insert inventory: {}, row: {:?}",
                        parsed.unwrap_err(),
                        row.1
                    );
                    continue;
                }

                let parsed = parsed.unwrap();
                let id = parsed.entity_id;

                match (parsed, delete_parsed.get(&id)) {
                    (new_inventory, Some(_old_inventory)) => {
                        potential_deletes.remove(&new_inventory.entity_id);
                        buffer_before_insert.insert(new_inventory.entity_id, new_inventory);
                    }
                    (new_inventory, None) => {
                        potential_deletes.remove(&new_inventory.entity_id);
                        buffer_before_insert.insert(new_inventory.entity_id, new_inventory);
                    }
                }
            }
        } else if event_type == "insert" {
        } else {
            error!("Unknown event type {:?}", p1);
            continue;
        }
    }

    if buffer_before_insert.len() > 0 {
        let mut buffer_before_insert_vec = buffer_before_insert
            .clone()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<inventory::Model>>();
        db_insert_inventory_state(
            database_connection,
            &mut buffer_before_insert_vec,
            &on_conflict,
        )
        .await?;
        buffer_before_insert.clear();
    }

    if potential_deletes.len() > 0 {
        db_delete_inventorys(database_connection, potential_deletes).await?;
    }

    Ok(())
}
