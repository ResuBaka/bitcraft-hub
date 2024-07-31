use entity::{claim_tech_desc, claim_tech_state};
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait};
use serde_json::Value;
use std::fs::File;
use std::path::PathBuf;

pub(crate) async fn import_claim_description_state(
    conn: &DatabaseConnection,
    storage_path: &PathBuf
) -> anyhow::Result<()> {
    let item_file =
        File::open(storage_path.join("State/ClaimTechState.json")).unwrap();
    let claim_tech_state: Value = serde_json::from_reader(&item_file).unwrap();
    let claim_tech_states: Vec<claim_tech_state::Model> = serde_json::from_value(
        claim_tech_state
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let count = claim_tech_states.len();
    let db_count = claim_tech_state::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let claim_tech_states: Vec<claim_tech_state::ActiveModel> = claim_tech_states
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for claim_tech_state in claim_tech_states.chunks(5000) {
        let _ = claim_tech_state::Entity::insert_many(claim_tech_state.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}

pub(crate) async fn import_claim_description_desc(conn: &DatabaseConnection, storage_path: &PathBuf) -> anyhow::Result<()> {
    let item_file =
        File::open(storage_path.join("Desc/ClaimTechDesc.json")).unwrap();
    let claim_tech_desc: Value = serde_json::from_reader(&item_file).unwrap();
    let claim_tech_descs: Vec<claim_tech_desc::Model> =
        serde_json::from_value(claim_tech_desc.get(0).unwrap().get("rows").unwrap().clone())
            .unwrap();
    let count = claim_tech_descs.len();
    let db_count = claim_tech_desc::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        return Ok(());
    }

    let claim_tech_descs: Vec<claim_tech_desc::ActiveModel> = claim_tech_descs
        .into_iter()
        .map(|x| x.into_active_model())
        .collect();

    for claim_tech_desc in claim_tech_descs.chunks(5000) {
        let _ = claim_tech_desc::Entity::insert_many(claim_tech_desc.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
