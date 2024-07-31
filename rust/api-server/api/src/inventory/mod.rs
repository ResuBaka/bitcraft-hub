use crate::{AppState, Params};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use axum_codec::Codec;
use entity::inventory::{
    Content, ExpendedRefrence, ItemExpended, ItemSlotResolved, ItemType, ResolvedInventory,
};
use entity::{building_desc, cargo_description, inventory, item};
use futures::StreamExt;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use serde::Deserialize;
use serde_json::{json, Value};
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};

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

    let inventory_chages_file = File::open(state.storage_path.join(format!(
        "Inventory/{}.json",
        id
    )));

    match inventory_chages_file {
        Ok(file) => {
            for line in BufReader::new(file).lines() {
                let line = line.unwrap();
                match serde_json::from_str(&line) {
                    Ok(data) => {
                        inventory_changes.push(data);
                    }
                    Err(e) => {
                        println!("");
                        println!("Error: {e}, line: {line}");
                        println!("");
                    }
                };
            }

            Ok(Codec(inventory_changes))
        }
        Err(e) => Err((StatusCode::NOT_FOUND, "InventoryChanged not found")),
    }
}

pub(crate) async fn find_inventory_by_id(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Codec<inventory::Model>, (StatusCode, &'static str)> {
    let inventory = QueryCore::find_inventory_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            println!("Error: {:?}", e);

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
    perPage: i64,
}

pub(crate) async fn find_inventory_by_owner_entity_id(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Codec<InventorysResponse>, (StatusCode, &'static str)> {
    let mut inventory_ids = vec![id];
    let player = QueryCore::find_player_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            println!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let mut mobile_entiety_map = HashMap::new();

    match &player {
        Some(player) => {
            let mobile_entiety_from_player =
                QueryCore::find_mobile_entity_by_owner_entity_id(&state.conn, player.entity_id)
                    .await
                    .map_err(|e| {
                        println!("Error: {:?}", e);

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
                println!("Error: {:?}", e);
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
            println!("Error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let cargo_descs = QueryCore::find_cargo_by_ids(&state.conn, cargo_ids)
        .await
        .map_err(|e| {
            println!("Error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let cargo_descs_map = cargo_descs
        .into_iter()
        .map(|cargo| (cargo.id, cargo))
        .collect::<HashMap<i64, cargo_description::Model>>();
    let item_descs_map = item_descs
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<i64, item::Model>>();

    for (index, inventory) in inventorys.into_iter().enumerate() {
        let mut pockets = vec![];

        for pocket in &inventory.pockets {
            pockets.push(resolve_pocket(pocket, &item_descs_map, &cargo_descs_map));
        }

        let nickname = match mobile_entiety_map.get(&inventory.owner_entity_id) {
            Some(nickname) => Some(nickname.clone()),
            None => match player.is_some() {
                true => {
                    if index == 0 {
                        Some(String::from("Tool belt"))
                    } else if index == 1 {
                        Some(String::from("Inventory"))
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
            nickname,
        });
    }

    resolved_inventory.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));

    Ok(Codec(InventorysResponse {
        inventorys: resolved_inventory,
        total: num_pages.number_of_items as i64,
        page: 1,
        perPage: 24,
    }))
}

pub(crate) async fn import_inventory(conn: &DatabaseConnection, storage_path: &PathBuf) -> anyhow::Result<()> {
    let item_file =
        File::open(storage_path.join("State/InventoryState.json")).unwrap();

    let buff_reader = BufReader::new(item_file);

    let mut buffer_before_insert: Vec<inventory::ActiveModel> = Vec::with_capacity(5000);

    let mut json_stream_reader = JsonStreamReader::new(buff_reader);

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"]).unwrap();
    json_stream_reader.begin_array()?;

    while let Ok(value) = json_stream_reader.deserialize_next::<inventory::Model>() {
        buffer_before_insert.push(value.into_active_model());

        if buffer_before_insert.len() == 5000 {
            let _ = inventory::Entity::insert_many(buffer_before_insert.to_vec())
                .on_conflict_do_nothing()
                .exec(conn)
                .await?;
            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        inventory::Entity::insert_many(buffer_before_insert.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
        buffer_before_insert.clear();
        println!("Inventory last batch imported");
    }
    println!("Importing inventory finished");

    Ok(())
}

pub(crate) fn resolve_pocket(
    pocket: &inventory::ItemSlot,
    item_desc: &HashMap<i64, item::Model>,
    cargo_desc: &HashMap<i64, cargo_description::Model>,
) -> ItemSlotResolved {
    let mut contents = None;
    for (pockcket_index, refrence) in pocket.clone().contents.iter() {
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
    item_desc: &HashMap<i64, item::Model>,
    cargo_desc: &HashMap<i64, cargo_description::Model>,
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
