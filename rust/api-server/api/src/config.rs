use serde::Deserialize;
use std::fmt;
use tracing::Level;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: DatabaseConfig,
    pub(crate) log_type: LogType,
    pub(crate) log_level: LogLevel,
    #[serde(alias = "storagepath")]
    pub(crate) storage_path: String,
    pub(crate) spacetimedb: SpacetimeDbConfig,
    #[serde(default)]
    pub(crate) origins: AllowedOriginConfig,
    #[serde(alias = "liveupdates")]
    pub(crate) live_updates: bool,
    #[serde(alias = "liveupdatesws")]
    pub(crate) live_updates_ws: bool,
    #[serde(rename = "import")]
    #[allow(dead_code)]
    pub(crate) import_type: ImportType,
    #[serde(rename = "importenabled")]
    pub(crate) import_enabled: bool,
    #[serde(rename = "enabledimporter")]
    pub(crate) enabled_importer: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8000,
            database: DatabaseConfig::default(),
            log_type: LogType::default(),
            log_level: LogLevel::default(),
            storage_path: "./storage".to_string(),
            spacetimedb: SpacetimeDbConfig::default(),
            origins: AllowedOriginConfig::default(),
            live_updates: false,
            live_updates_ws: false,
            import_type: ImportType::File,
            import_enabled: false,
            enabled_importer: vec!["".to_string()],
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub enum LogLevel {
    #[serde(alias = "error")]
    Error,
    #[serde(alias = "warn")]
    Warn,
    #[serde(alias = "info")]
    #[default]
    Info,
    #[serde(alias = "debug")]
    Debug,
    #[serde(alias = "trace")]
    Trace,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => Level::ERROR,
            Self::Warn => Level::WARN,
            Self::Info => Level::INFO,
            Self::Debug => Level::DEBUG,
            Self::Trace => Level::TRACE,
        }
        .fmt(f)
    }
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
#[serde(default)]
pub(crate) struct DatabaseConfig {
    pub(crate) url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "".to_string(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct SpacetimeDbConfig {
    pub(crate) domain: String,
    pub(crate) protocol: String,
    pub(crate) database: String,
    pub(crate) password: String,
    pub(crate) username: String,
    pub(crate) websocket_protocol: String,
}

impl Default for SpacetimeDbConfig {
    fn default() -> Self {
        Self {
            domain: "localhost".to_string(),
            protocol: "https://".to_string(),
            database: "".to_string(),
            password: "".to_string(),
            username: "token".to_string(),
            websocket_protocol: "wss://".to_string(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
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
pub(crate) enum LogType {
    Default,
    #[serde(alias = "json")]
    Json,
}

impl Default for LogType {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Default)]
pub(crate) struct CliConfigParameters {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) database_url: Option<String>,
    pub(crate) storage_path: Option<String>,
    pub(crate) config_path: Option<String>,
}

impl Config {
    pub fn new(cli_config_parameters: Option<CliConfigParameters>) -> anyhow::Result<Self> {
        let config_path = if let Some(cli_config_parameters) = &cli_config_parameters {
            if let Some(config_path) = &cli_config_parameters.config_path {
                config_path
            } else {
                "config/config"
            }
        } else {
            "config/config"
        };

        let mut config = config::Config::builder()
            .add_source(config::File::with_name(config_path).required(false));

        config = if let Some(cli_config_parameters) = cli_config_parameters {
            let mut cli_overrides = config::Config::builder();

            if let Some(host) = cli_config_parameters.host {
                cli_overrides = cli_overrides.set_default("host", host)?;
            }

            if let Some(port) = cli_config_parameters.port {
                cli_overrides = cli_overrides.set_default("port", port)?;
            }

            if let Some(database_url) = cli_config_parameters.database_url {
                cli_overrides = cli_overrides.set_default("database.url", database_url)?;
            }

            if let Some(storage_path) = cli_config_parameters.storage_path {
                cli_overrides = cli_overrides.set_default("storage_path", storage_path)?;
            }

            config.add_source(cli_overrides.build()?)
        } else {
            config
        };

        config = config.add_source(
            config::Environment::with_prefix("BITCRAFT_HUB_API")
                .separator("__")
                .list_separator(",")
                .with_list_parse_key("origins.origin")
                .with_list_parse_key("enabledimporter")
                .try_parsing(true),
        );
        let config = config.build()?;

        Ok(config.try_deserialize()?)
    }

    pub fn server_url(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    #[allow(dead_code)]
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
