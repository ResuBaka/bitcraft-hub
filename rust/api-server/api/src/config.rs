use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: DatabaseConfig,
    #[serde(rename = "storagepath")]
    pub(crate) storage_path: String,
    pub(crate) spacetimedb: SpacetimeDbConfig,
    #[serde(default)]
    pub(crate) origins: AllowedOriginConfig,
    #[serde(rename = "liveupdates", default)]
    pub(crate) live_updates: bool,
    #[serde(rename = "liveupdatesws", default)]
    pub(crate) live_updates_ws: bool,
    #[serde(rename = "import", default)]
    #[allow(dead_code)]
    pub(crate) import_type: ImportType,
    #[serde(rename = "importenabled", default)]
    pub(crate) import_enabled: bool,
    #[serde(rename = "enabledimporter", default)]
    pub(crate) enabled_importer: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) enum ImportType {
    File,
    Game,
}

impl Default for ImportType {
    fn default() -> Self {
        Self::File
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct DatabaseConfig {
    pub(crate) url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct SpacetimeDbConfig {
    pub(crate) domain: String,
    pub(crate) protocol: String,
    pub(crate) database: String,
    pub(crate) password: String,
    #[serde(default = "default_spacetimedb_username")]
    pub(crate) username: String,
    #[serde(default = "default_spacetimedb_websocket_protocol")]
    pub(crate) websocket_protocol: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct AllowedOriginConfig {
    pub(crate) origin: Vec<String>,
}

impl Default for AllowedOriginConfig {
    fn default() -> Self {
        Self {
            origin: vec!["http://localhost:3000".to_string()],
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct StorageConfig {
    #[allow(dead_code)]
    pub(crate) path: String,
}

impl Config {
    pub fn new() -> Self {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config/config.toml").required(false))
            .add_source(
                config::Environment::with_prefix("BITCRAFT_HUB_API")
                    .separator("_")
                    .list_separator(",")
                    .with_list_parse_key("origins.origin")
                    .with_list_parse_key("enabledimporter")
                    .try_parsing(true),
            )
            .build()
            .unwrap();

        config.try_deserialize().unwrap()
    }

    pub fn server_url(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn weboosocket_url(&self) -> String {
        format!(
            "{}{}",
            self.spacetimedb.websocket_protocol, self.spacetimedb.domain
        )
    }

    #[allow(dead_code)]
    pub fn spacetimedb_url(&self) -> String {
        format!("{}{}", self.spacetimedb.protocol, self.spacetimedb.domain)
    }
}

fn default_spacetimedb_username() -> String {
    "token".to_string()
}

fn default_spacetimedb_websocket_protocol() -> String {
    "wss://".to_string()
}
