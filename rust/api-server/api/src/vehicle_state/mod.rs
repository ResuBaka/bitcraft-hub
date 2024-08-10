use entity::vehicle_state;
use log::info;
use sea_orm::{DatabaseConnection, EntityTrait, IntoActiveModel};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use struson::json_path;
use struson::reader::{JsonReader, JsonStreamReader};

pub(crate) async fn import_vehicle_state(
    conn: &DatabaseConnection,
    storage_path: &PathBuf,
) -> anyhow::Result<()> {
    let item_file = File::open(storage_path.join("State/VehicleState.json")).unwrap();

    let buff_reader = BufReader::new(item_file);

    let mut buffer_before_insert: Vec<vehicle_state::ActiveModel> = Vec::with_capacity(5000);

    let mut json_stream_reader = JsonStreamReader::new(buff_reader);

    json_stream_reader.begin_array()?;
    json_stream_reader.seek_to(&json_path!["rows"]).unwrap();
    json_stream_reader.begin_array()?;

    while let Ok(value) = json_stream_reader.deserialize_next::<vehicle_state::Model>() {
        buffer_before_insert.push(value.into_active_model());

        if buffer_before_insert.len() == 5000 {
            let _ = vehicle_state::Entity::insert_many(buffer_before_insert.to_vec())
                .on_conflict_do_nothing()
                .exec(conn)
                .await?;
            buffer_before_insert.clear();
        }
    }

    if buffer_before_insert.len() > 0 {
        vehicle_state::Entity::insert_many(buffer_before_insert.to_vec())
            .on_conflict_do_nothing()
            .exec(conn)
            .await?;
        buffer_before_insert.clear();
        info!("VehicleState last batch imported");
    }
    info!("Importing VehicleState finished");

    Ok(())
}
