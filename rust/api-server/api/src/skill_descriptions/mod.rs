use entity::skill_desc;
use log::info;
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm::{IntoActiveModel, PaginatorTrait};
use serde_json::Value;
use std::fs::File;
use std::path::PathBuf;

pub(crate) async fn import_skill_descriptions(
    conn: &DatabaseConnection,
    storage_path: &PathBuf,
) -> anyhow::Result<()> {
    let item_file = File::open(storage_path.join("State/SkillDesc.json")).unwrap();
    let skill_descriptions: Value = serde_json::from_reader(&item_file).unwrap();
    let skill_descriptions: Vec<skill_desc::Model> = serde_json::from_value(
        skill_descriptions
            .get(0)
            .unwrap()
            .get("rows")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let count = skill_descriptions.len();
    let db_count = skill_desc::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        info!("SkillDescriptions already imported");
        return Ok(());
    }

    let skill_descriptions = skill_descriptions
        .into_iter()
        .map(|x| x.into_active_model())
        .collect::<Vec<skill_desc::ActiveModel>>();

    for skill_description in skill_descriptions.chunks(5000) {
        let _ = skill_desc::Entity::insert_many(skill_description.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
    }

    Ok(())
}
