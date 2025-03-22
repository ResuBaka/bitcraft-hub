use crate::config::Config;
use log::error;
use log::info;
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(clap::Subcommand, PartialEq, Eq, Debug)]
pub enum DownloadSubcommand {
    All,
    State,
    Desc,
    Rest,
}

pub async fn download_desc_tables(client: &Client, storage_path: &Path, config: &Config) {
    for table in &config.download.desc_tables {
        let desc_result = download_tables(client, table, config, storage_path, "desc").await;

        if let Err(error) = desc_result {
            error!("Error while downloading desc table {table}: {error}");
        }
    }
}

pub async fn download_state_tables(client: &Client, storage_path: &Path, config: &Config) {
    for table in &config.download.state_tables {
        let state_result = download_tables(client, table, config, storage_path, "state").await;

        if let Err(error) = state_result {
            error!("Error while downloading state table {table}: {error}");
        }
    }
}

pub async fn download_rest_tables(client: &Client, storage_path: &Path, config: &Config) {
    for table in &config.download.rest_tables {
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
) {
    match download_subcommand {
        DownloadSubcommand::All => {
            download_desc_tables(client, storage_path, config).await;
            download_state_tables(client, storage_path, config).await;
            download_rest_tables(client, storage_path, config).await;
        }
        DownloadSubcommand::Desc => {
            download_desc_tables(client, storage_path, config).await;
        }
        DownloadSubcommand::State => {
            download_state_tables(client, storage_path, config).await;
        }
        DownloadSubcommand::Rest => {
            download_rest_tables(client, storage_path, config).await;
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
        .post(format!("{protocol}{domain}/database/sql/{database}"))
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
