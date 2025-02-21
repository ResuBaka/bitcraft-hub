use crate::websocket::Table;
use entity::collectible_desc;
use log::{debug, error, info};
use migration::OnConflict;
use sea_orm::IntoActiveModel;
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::collections::{HashMap, HashSet};

async fn get_known_collectible_desc_ids(conn: &DatabaseConnection) -> anyhow::Result<HashSet<i32>> {
    let known_collectible_desc_ids: Vec<i32> = collectible_desc::Entity::find()
        .select_only()
        .column(collectible_desc::Column::Id)
        .into_tuple()
        .all(conn)
        .await?;

    let known_collectible_desc_ids = known_collectible_desc_ids
        .into_iter()
        .collect::<HashSet<i32>>();
    Ok(known_collectible_desc_ids)
}

async fn db_insert_collectible_descs(
    conn: &DatabaseConnection,
    buffer_before_insert: &mut Vec<collectible_desc::Model>,
    on_conflict: &OnConflict,
) -> anyhow::Result<()> {
    let collectible_descs_from_db = collectible_desc::Entity::find()
        .filter(
            collectible_desc::Column::Id.is_in(
                buffer_before_insert
                    .iter()
                    .map(|collectible_desc| collectible_desc.id)
                    .collect::<Vec<i32>>(),
            ),
        )
        .all(conn)
        .await?;

    let collectible_descs_from_db_map = collectible_descs_from_db
        .into_iter()
        .map(|collectible_desc| (collectible_desc.id, collectible_desc))
        .collect::<HashMap<i32, collectible_desc::Model>>();

    let things_to_insert = buffer_before_insert
        .iter()
        .filter(|collectible_desc| {
            match collectible_descs_from_db_map.get(&collectible_desc.id) {
                Some(collectible_desc_from_db) => {
                    if collectible_desc_from_db != *collectible_desc {
                        return true;
                    }
                }
                None => {
                    return true;
                }
            }

            return false;
        })
        .map(|collectible_desc| collectible_desc.clone().into_active_model())
        .collect::<Vec<collectible_desc::ActiveModel>>();

    if things_to_insert.len() == 0 {
        debug!("Nothing to insert");
        buffer_before_insert.clear();
    } else {
        info!("Inserting {} things_to_insert", things_to_insert.len());

        let _ = collectible_desc::Entity::insert_many(things_to_insert)
            .on_conflict(on_conflict.clone())
            .exec(conn)
            .await?;

        buffer_before_insert.clear();
    }

    Ok(())
}

async fn delete_collectible_desc(
    conn: &DatabaseConnection,
    known_collectible_desc_ids: HashSet<i32>,
) -> anyhow::Result<()> {
    info!(
        "collectible_desc's ({}) to delete: {:?}",
        known_collectible_desc_ids.len(),
        known_collectible_desc_ids
    );
    collectible_desc::Entity::delete_many()
        .filter(collectible_desc::Column::Id.is_in(known_collectible_desc_ids))
        .exec(conn)
        .await?;
    Ok(())
}

pub(crate) async fn handle_initial_subscription(
    p0: &DatabaseConnection,
    p1: &Table,
) -> anyhow::Result<()> {
    let chunk_size = Some(500);
    let mut buffer_before_insert: Vec<collectible_desc::Model> =
        Vec::with_capacity(chunk_size.unwrap_or(5000));

    let on_conflict = sea_query::OnConflict::column(collectible_desc::Column::Id)
        .update_columns([
            collectible_desc::Column::Name,
            collectible_desc::Column::Description,
            collectible_desc::Column::CollectibleType,
            collectible_desc::Column::InvalidatesType,
            collectible_desc::Column::AutoCollect,
            collectible_desc::Column::CollectibleRarity,
            collectible_desc::Column::StartingLoadout,
            collectible_desc::Column::Locked,
            collectible_desc::Column::Variant,
            collectible_desc::Column::Color,
            collectible_desc::Column::Emission,
            collectible_desc::Column::MaxEquipCount,
            collectible_desc::Column::ModelAssetName,
            collectible_desc::Column::VariantMaterial,
            collectible_desc::Column::IconAssetName,
            collectible_desc::Column::Tag,
            collectible_desc::Column::DisplayString,
            collectible_desc::Column::ItemDeedId,
        ])
        .to_owned();

    let mut known_collectible_desc_ids = get_known_collectible_desc_ids(p0).await?;

    for update in p1.updates.iter() {
        for row in update.inserts.iter() {
            match serde_json::from_str::<collectible_desc::Model>(row.as_ref()) {
                Ok(collectible_desc) => {
                    if known_collectible_desc_ids.contains(&collectible_desc.id) {
                        known_collectible_desc_ids.remove(&collectible_desc.id);
                    }
                    buffer_before_insert.push(collectible_desc);
                    if buffer_before_insert.len() == chunk_size.unwrap_or(5000) {
                        db_insert_collectible_descs(p0, &mut buffer_before_insert, &on_conflict)
                            .await?;
                    }
                }
                Err(error) => {
                    error!(
                        "TransactionUpdate Insert collectible_desc::Model Error: {error} -> {:?}",
                        row
                    );
                }
            }
        }
    }

    if buffer_before_insert.len() > 0 {
        db_insert_collectible_descs(p0, &mut buffer_before_insert, &on_conflict).await?;
    }

    if known_collectible_desc_ids.len() > 0 {
        delete_collectible_desc(p0, known_collectible_desc_ids).await?;
    }

    Ok(())
}

// pub(crate) async fn handle_transaction_update(
//     p0: &DatabaseConnection,
//     tables: &Vec<Table>,
// ) -> anyhow::Result<()> {
//     let on_conflict = sea_query::OnConflict::column(collectible_desc::Column::Id)
//         .update_columns([
//             collectible_desc::Column::Shards,
//         ])
//         .to_owned();
//
//     let chunk_size = Some(5000);
//     let mut buffer_before_insert = HashMap::new();
//
//     let mut found_in_inserts = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.inserts.iter() {
//             match serde_json::from_str::<collectible_desc::Model>(row.Text.as_ref()) {
//                 Ok(collectible_desc) => {
//                     found_in_inserts.insert(collectible_desc.id);
//                     buffer_before_insert.insert(collectible_desc.id, collectible_desc);
//                     if buffer_before_insert.len() == chunk_size.unwrap_or(1000) {
//                         let mut buffer_before_insert_vec = buffer_before_insert
//                             .clone()
//                             .into_iter()
//                             .map(|x| x.1)
//                             .collect::<Vec<collectible_desc::Model>>();
//
//                         db_insert_collectible_descs(p0, &mut buffer_before_insert_vec, &on_conflict)
//                             .await?;
//                         buffer_before_insert.clear();
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Insert PlayerState Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if buffer_before_insert.len() > 0 {
//         let mut buffer_before_insert_vec = buffer_before_insert
//             .clone()
//             .into_iter()
//             .map(|x| x.1)
//             .collect::<Vec<collectible_desc::Model>>();
//
//         db_insert_collectible_descs(p0, &mut buffer_before_insert_vec, &on_conflict).await?;
//         buffer_before_insert.clear();
//     }
//
//     let mut players_to_delete = HashSet::new();
//
//     for p1 in tables.iter() {
//         for row in p1.deletes.iter() {
//             match serde_json::from_str::<collectible_desc::Model>(row.Text.as_ref()) {
//                 Ok(collectible_desc) => {
//                     if !found_in_inserts.contains(&collectible_desc.id) {
//                         players_to_delete.insert(collectible_desc.id);
//                     }
//                 }
//                 Err(error) => {
//                     error!("TransactionUpdate Delete PlayerState Error: {error}");
//                 }
//             }
//         }
//     }
//
//     if players_to_delete.len() > 0 {
//         delete_collectible_desc(p0, players_to_delete).await?;
//     }
//
//     Ok(())
// }
