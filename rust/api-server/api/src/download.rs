use crate::config::Config;
use log::error;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::info;

#[derive(clap::Subcommand, PartialEq, Eq, Debug)]
pub enum DownloadSubcommand {
    All,
    State,
    Desc,
    Rest,
    Schema,
}

pub async fn download_desc_tables(
    client: &Client,
    storage_path: &Path,
    config: &Config,
    tables: Vec<String>,
) {
    for table in &tables {
        let desc_result = download_tables(client, table, config, storage_path, "desc").await;

        if let Err(error) = desc_result {
            error!("Error while downloading desc table {table}: {error}");
        }
    }
}

pub async fn download_state_tables(
    client: &Client,
    storage_path: &Path,
    config: &Config,
    tables: Vec<String>,
) {
    for table in &tables {
        let state_result = download_tables(client, table, config, storage_path, "state").await;

        if let Err(error) = state_result {
            error!("Error while downloading state table {table}: {error}");
        }
    }
}

pub async fn download_rest_tables(
    client: &Client,
    storage_path: &Path,
    config: &Config,
    tables: Vec<String>,
) {
    for table in &tables {
        let rest_result = download_tables(client, table, config, storage_path, "rest").await;

        if let Err(error) = rest_result {
            error!("Error while downloading rest table {table}: {error}");
        }
    }
}

pub async fn download_all_tables(
    download_subcommand: DownloadSubcommand,
    client: &Client,
    storage_path: &Path,
    config: &Config,
    remote_schema: bool,
) {
    let (desc_table, state_table, rest_table) = if remote_schema {
        let schema = download_schema(client, config, storage_path, false).await;

        if let Err(error) = &schema {
            error!("Error while downloading schema {error}");
            return;
        }

        let mut map = HashMap::new();

        map.insert("desc", HashSet::new());
        map.insert("state", HashSet::new());
        map.insert("rest", HashSet::new());

        if let Some(tables) = schema.unwrap().as_object().unwrap().get("tables") {
            for table in tables.as_array().unwrap() {
                let name = table["name"].as_str().unwrap();

                if name.ends_with("_desc") {
                    map.get_mut("desc").unwrap().insert(name.to_string());
                } else if name.ends_with("_state") {
                    map.get_mut("state").unwrap().insert(name.to_string());
                } else {
                    map.get_mut("rest").unwrap().insert(name.to_string());
                }
            }
        }

        (
            map.get("desc")
                .unwrap()
                .iter()
                .map(|table| table.to_owned())
                .collect(),
            map.get("state")
                .unwrap()
                .iter()
                .map(|table| table.to_owned())
                .collect(),
            map.get("rest")
                .unwrap()
                .iter()
                .map(|table| table.to_owned())
                .collect(),
        )
    } else {
        (
            config.download.desc_tables.clone(),
            config.download.state_tables.clone(),
            config.download.rest_tables.clone(),
        )
    };

    match download_subcommand {
        DownloadSubcommand::All => {
            download_desc_tables(client, storage_path, config, desc_table).await;
            download_state_tables(client, storage_path, config, state_table).await;
            download_rest_tables(client, storage_path, config, rest_table).await;
        }
        DownloadSubcommand::Desc => {
            download_desc_tables(client, storage_path, config, desc_table).await;
        }
        DownloadSubcommand::State => {
            download_state_tables(client, storage_path, config, state_table).await;
        }
        DownloadSubcommand::Rest => {
            download_rest_tables(client, storage_path, config, rest_table).await;
        }
        DownloadSubcommand::Schema => {
            let error = download_schema(client, config, storage_path, true).await;

            if error.is_err() {
                error!("Error while downloading schema {error:?}");
            }
        }
    }
}

/// Donwload the table and save it to the storage path with the type as the folder before the name
pub async fn download_tables(
    client: &Client,
    table: &str,
    config: &Config,
    storage_path: &Path,
    folder: &str,
) -> anyhow::Result<()> {
    let domain = &config.spacetimedb.domain;
    let protocol = &config.spacetimedb.protocol;
    let database = &config.spacetimedb.database;

    let response = client
        .post(format!("{protocol}{domain}/v1/database/{database}/sql"))
        .body(format!("SELECT * FROM {table}"))
        .send()
        .await;

    let json = match response {
        Ok(response) => {
            if !response.status().is_success() {
                let error = response.text().await?;
                error!("Error: {error}");
                return Err(anyhow::anyhow!("Error: {error}"));
            }

            response.text().await?
        }
        Err(error) => {
            error!("Error: {error}");
            return Err(anyhow::anyhow!("Error: {error}"));
        }
    };

    let folder_to_create = storage_path.join(folder);
    if !folder_to_create.exists() {
        std::fs::create_dir_all(&folder_to_create)?;
    }
    let path = storage_path.join(format!("{folder}/{table}.json"));
    let mut file = File::create(&path)?;

    info!("Saving to {path:?}");

    file.write_all(json.as_bytes())?;

    Ok(())
}

pub async fn download_schema(
    client: &Client,
    config: &Config,
    storage_path: &Path,
    save_to_disk: bool,
) -> anyhow::Result<serde_json::Value> {
    let domain = &config.spacetimedb.domain;
    let protocol = &config.spacetimedb.protocol;
    let database = &config.spacetimedb.database;

    let response = client
        .get(format!(
            "{protocol}{domain}/v1/database/{database}/schema?version=9"
        ))
        .send()
        .await;

    let json = match response {
        Ok(response) => {
            if !response.status().is_success() {
                let error = response.text().await?;
                error!("Error: {error}");
                return Err(anyhow::anyhow!("Error: {error}"));
            }

            response.json::<serde_json::Value>().await?
        }
        Err(error) => {
            error!("Error: {error}");
            return Err(anyhow::anyhow!("Error: {error}"));
        }
    };

    if save_to_disk {
        let path = storage_path.join("schema.json");
        let mut file = File::create(&path)?;

        info!("Saving to {path:?}");

        file.write_all(serde_json::to_vec_pretty(&json)?.as_slice())?;
    }

    Ok(json)
}
