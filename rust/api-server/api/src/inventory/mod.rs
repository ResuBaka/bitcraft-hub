use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use entity::inventory::{
    ExpendedRefrence, ItemExpended, ItemSlotResolved, ItemType, ResolvedInventory,
};
use entity::{cargo_desc, inventory, item_desc};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub(crate) fn get_routes() -> AppRouter {
    Router::new()
        .route(
            "/inventorys/changes/{id}",
            axum_codec::routing::get(read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/changes/{id}",
            axum_codec::routing::get(read_inventory_changes).into(),
        )
        .route(
            "/api/bitcraft/inventorys/owner_entity_id/{id}",
            axum_codec::routing::get(find_inventory_by_owner_entity_id).into(),
        )
        .route(
            "/inventory/{id}",
            axum_codec::routing::get(find_inventory_by_id).into(),
        )
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
) -> Result<axum_codec::Codec<Vec<InventoryChanged>>, (StatusCode, &'static str)> {
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

            Ok(axum_codec::Codec(inventory_changes))
        }
        Err(_e) => Err((StatusCode::NOT_FOUND, "InventoryChanged not found")),
    }
}

pub(crate) async fn find_inventory_by_id(
    state: State<std::sync::Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<axum_codec::Codec<inventory::Model>, (StatusCode, &'static str)> {
    let inventory = QueryCore::find_inventory_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    match inventory {
        Some(inventory) => Ok(axum_codec::Codec(inventory)),
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
) -> Result<axum_codec::Codec<InventorysResponse>, (StatusCode, &'static str)> {
    let mut inventory_ids = vec![id];
    let player = QueryCore::find_player_by_id(&state.conn, id)
        .await
        .map_err(|e| {
            error!("Error: {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
        })?;

    let mut mobile_entiety_map: HashMap<i64, String> = HashMap::new();

    if let Some(player) = &player {
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
            if let Some(contents) = pocket.contents.clone() {
                if contents.item_type == ItemType::Item {
                    item_ids.push(contents.item_id);
                } else {
                    cargo_ids.push(contents.item_id);
                }
            }
        }
    }

    let mut resolved_inventory = vec![];

    let item_descs_map = state
        .item_desc
        .iter()
        .filter_map(|item| {
            if !item_ids.contains(&item.id) {
                return None;
            }

            Some((item.id, item.clone()))
        })
        .collect::<HashMap<i32, item_desc::Model>>();

    let cargo_descs_map = state
        .cargo_desc
        .iter()
        .filter_map(|cargo| {
            if !cargo_ids.contains(&cargo.id) {
                return None;
            }

            Some((cargo.id, cargo.clone()))
        })
        .collect::<HashMap<i32, cargo_desc::Model>>();

    for inventory in inventorys.into_iter() {
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

    Ok(axum_codec::Codec(InventorysResponse {
        inventorys: resolved_inventory,
        total: num_pages.number_of_items as i64,
        page: 1,
        per_page: 24,
    }))
}

#[allow(dead_code)]
pub(crate) async fn load_inventory_state_from_file(
    storage_path: &std::path::Path,
) -> anyhow::Result<Vec<inventory::Model>> {
    let item_file = File::open(storage_path.join("State/InventoryState.json"))?;
    let inventory: Value = serde_json::from_reader(&item_file)?;
    let inventory: Vec<inventory::Model> =
        serde_json::from_value(inventory.get(0).unwrap().get("rows").unwrap().clone())?;

    Ok(inventory)
}

pub(crate) fn resolve_pocket(
    pocket: &inventory::Pocket,
    item_desc: &HashMap<i32, item_desc::Model>,
    cargo_desc: &HashMap<i32, cargo_desc::Model>,
) -> ItemSlotResolved {
    let contents = resolve_contents(&pocket.contents, item_desc, cargo_desc);
    ItemSlotResolved {
        volume: pocket.volume as i64,
        contents,
        locked: pocket.locked,
    }
}

pub(crate) fn resolve_contents(
    contents: &Option<inventory::ItemStack>,
    item_desc: &HashMap<i32, item_desc::Model>,
    cargo_desc: &HashMap<i32, cargo_desc::Model>,
) -> Option<ExpendedRefrence> {
    if let Some(content) = contents {
        if content.item_type == ItemType::Item {
            return Some(ExpendedRefrence {
                item_id: content.item_id,
                item: ItemExpended::Item(item_desc.get(&content.item_id).unwrap().clone()),
                quantity: content.quantity,
                item_type: ItemType::Item,
                durability: content.durability,
            });
        } else {
            return Some(ExpendedRefrence {
                item_id: content.item_id,
                item: ItemExpended::Cargo(cargo_desc.get(&content.item_id).unwrap().clone()),
                quantity: content.quantity,
                item_type: ItemType::Cargo,
                durability: content.durability,
            });
        }
    }
    None
}
