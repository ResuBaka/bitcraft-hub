pub(crate) mod bitcraft;

use crate::{AppRouter, AppState};
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use entity::inventory::{
    ExpendedRefrence, ItemExpended, ItemSlotResolved, ItemType, ResolvedInventory,
};
use entity::{cargo_desc, inventory, inventory_changelog, item_desc};
use log::error;
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use service::Query as QueryCore;
use std::collections::HashMap;
use std::fs::File;
use std::ops::AddAssign;
use std::sync::Arc;
use ts_rs::TS;

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
        .route(
            "/inventory/all_inventory_stats",
            axum_codec::routing::get(all_inventory_stats).into(),
        )
}

#[derive(Deserialize)]
pub(crate) struct InventoryChangesParams {
    pub item_id: Option<i32>,
    pub item_type: Option<inventory_changelog::ItemType>,
    pub user_id: Option<i64>,
}
pub(crate) async fn read_inventory_changes(
    state: State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<InventoryChangesParams>,
) -> Result<axum_codec::Codec<Vec<inventory_changelog::Model>>, (StatusCode, &'static str)> {
    let (inventory_changes, _num_pages) = QueryCore::find_inventory_changes_by_entity_ids(
        &state.conn,
        vec![id],
        10000,
        params.item_id,
        params.item_type,
        params.user_id,
    )
    .await
    .map_err(|e| {
        error!("Error: {:?}", e);

        (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
    })?;
    Ok(axum_codec::Codec(inventory_changes))
}

pub(crate) async fn find_inventory_by_id(
    state: State<AppState>,
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

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
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

    for inventory in inventorys.into_iter() {
        let mut pockets = vec![];

        for pocket in &inventory.pockets {
            pockets.push(resolve_pocket(pocket, &state.item_desc, &state.cargo_desc));
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

#[derive(Serialize, Deserialize)]
pub(crate) struct AllInventoryStatsResponse {
    items: Vec<(i64, Option<::entity::item_desc::Model>)>,
    cargo: Vec<(i64, Option<::entity::cargo_desc::Model>)>,
}

pub(crate) async fn all_inventory_stats(
    state: State<AppState>,
) -> Result<axum_codec::Codec<AllInventoryStatsResponse>, (StatusCode, &'static str)> {
    if state.inventory_state.is_empty() {
        let inventorys = ::entity::inventory::Entity::find()
            .all(&state.conn)
            .await
            .map_err(|e| {
                error!("Error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Unexpected error")
            })?;

        for inventory in inventorys {
            state.inventory_state.insert(inventory.entity_id, inventory);
        }
    }

    let mut items: HashMap<i32, (i64, Option<::entity::item_desc::Model>)> = HashMap::new();
    let mut cargo: HashMap<i32, (i64, Option<::entity::cargo_desc::Model>)> = HashMap::new();

    for inventory in state.inventory_state.iter() {
        for pocket in &inventory.pockets {
            if let Some(contents) = pocket.contents.clone() {
                if contents.item_type == ItemType::Item {
                    items
                        .entry(contents.item_id)
                        .and_modify(|(qty, _)| qty.add_assign(contents.quantity as i64))
                        .or_insert((
                            contents.quantity as i64,
                            state
                                .item_desc
                                .get(&contents.item_id)
                                .map(|item_desc| item_desc.to_owned()),
                        ));
                } else {
                    cargo
                        .entry(contents.item_id)
                        .and_modify(|(qty, _)| qty.add_assign(contents.quantity as i64))
                        .or_insert((
                            contents.quantity as i64,
                            state
                                .cargo_desc
                                .get(&contents.item_id)
                                .map(|cargo_desc| cargo_desc.to_owned()),
                        ));
                }
            }
        }
    }

    // let (items, cargo) = inventorys
    //     .par_iter() // Parallel iterator for inventories
    //     .flat_map(|inventory| &inventory.pockets) // Flatten to iterate over all pockets
    //     .filter_map(|pocket| pocket.contents.clone()) // Only consider pockets with contents
    //     .fold(
    //         || (HashMap::<i32, (i64, Option<ExpendedRefrence>)>::new(), HashMap::<i32, (i64, Option<ExpendedRefrence>)>::new()), // Initial value for each thread
    //         |mut acc, contents| {
    //             // Accumulate results within each thread
    //             if contents.item_type == ItemType::Item {
    //                 acc.0.entry(contents.item_id).and_modify(|(qty, _): &mut (_, _)| qty.add_assign(contents.quantity)).or_insert((
    //                     contents.quantity,
    //                     resolve_contents(&Some(contents), &state.item_desc, &state.cargo_desc),
    //                 ));
    //             } else {
    //                 acc.1.entry(contents.item_id).and_modify(|(qty, _): &mut (_, _)| qty.add_assign(contents.quantity)).or_insert((
    //                     contents.quantity,
    //                     resolve_contents(&Some(contents), &state.item_desc, &state.cargo_desc),
    //                 ));
    //             }
    //             acc
    //         },
    //     )
    //     .reduce(
    //         || (HashMap::<i32, (i64, Option<ExpendedRefrence>)>::new(), HashMap::<i32, (i64, Option<ExpendedRefrence>)>::new()), // Initial value for the reduction
    //         |mut acc1, acc2| {
    //             // Merge results from different threads
    //             for (item_id, (quantity, description)) in acc2.0 {
    //                 acc1.0.entry(item_id).and_modify(|(qty, _)| qty.add_assign(quantity)).or_insert((quantity, description));
    //             }
    //             for (item_id, (quantity, description)) in acc2.1 {
    //                 acc1.1.entry(item_id).and_modify(|(qty, _)| qty.add_assign(quantity)).or_insert((quantity, description));
    //             }
    //             acc1
    //         },
    //     );

    let mut items = items.into_values().collect::<Vec<_>>();

    items.sort_by(|a, b| b.0.cmp(&a.0));

    let mut cargo = cargo.into_values().collect::<Vec<_>>();

    cargo.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(axum_codec::Codec(AllInventoryStatsResponse {
        items,
        cargo,
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
    item_desc: &Arc<dashmap::DashMap<i32, item_desc::Model>>,
    cargo_desc: &Arc<dashmap::DashMap<i32, cargo_desc::Model>>,
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
    item_desc: &Arc<dashmap::DashMap<i32, item_desc::Model>>,
    cargo_desc: &Arc<dashmap::DashMap<i32, cargo_desc::Model>>,
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
