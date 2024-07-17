use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use std::fs::File;
use serde_json::Value;
use entity::{cargo_description};

pub(crate) async fn import_cargo_description(
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/Desc/CargoDesc.json").unwrap();
    let cargo_description: Value = serde_json::from_reader(&item_file).unwrap();
    let cargo_descriptions: Vec<cargo_description::Model> = serde_json::from_value(cargo_description.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = cargo_descriptions.len();
    let db_count = cargo_description::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let cargo_descriptions: Vec<cargo_description::ActiveModel> = cargo_descriptions.into_iter().map(|x| x.into_active_model()).collect();

    for cargo_description in cargo_descriptions.chunks(5000) {
        let _ = cargo_description::Entity::insert_many(cargo_description.to_vec()).exec(conn).await;
    }

    Ok(())
}