use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{SocketAddr, ToSocketAddrs};
use tracing::Level;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: DatabaseConfig,
    pub(crate) log_type: LogType,
    pub(crate) log_level: LogLevel,
    pub(crate) spacetimedb: SpacetimeDbConfig,
    #[serde(default)]
    pub(crate) origins: AllowedOriginConfig,
    #[serde(alias = "liveupdatesws")]
    pub(crate) live_updates_ws: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8000,
            database: DatabaseConfig::default(),
            log_type: LogType::default(),
            log_level: LogLevel::default(),
            spacetimedb: SpacetimeDbConfig::default(),
            origins: AllowedOriginConfig::default(),
            live_updates_ws: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct DatabaseConfig {
    pub(crate) url: String,
    pub(crate) max_connections: u32,
    pub(crate) min_connections: u32,
    pub(crate) connect_timeout: u64,
    pub(crate) idle_timeout: u64,
    pub(crate) max_lifetime: Option<u64>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "".to_string(),
            max_connections: 200,
            min_connections: 30,
            connect_timeout: 10,
            idle_timeout: 60 * 5,
            max_lifetime: None,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct SpacetimeDbConfig {
    pub(crate) domain: String,
    pub(crate) protocol: String,
    pub(crate) database: String,
    pub(crate) databases: Vec<String>,
    pub(crate) password: String,
    pub(crate) username: String,
    pub(crate) websocket_protocol: String,
    pub(crate) cleanup: bool,
}

impl Default for SpacetimeDbConfig {
    fn default() -> Self {
        Self {
            domain: "localhost".to_string(),
            protocol: "https://".to_string(),
            database: "".to_string(),
            databases: vec![],
            password: "".to_string(),
            username: "token".to_string(),
            websocket_protocol: "wss://".to_string(),
            cleanup: false,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) enum LogType {
    Default,
    #[serde(alias = "json")]
    Json,
    #[serde(alias = "pretty")]
    Pretty,
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
    pub(crate) live_updates_ws: Option<bool>,
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

            if let Some(live_updates_ws) = cli_config_parameters.live_updates_ws {
                cli_overrides = cli_overrides.set_default("live_updates_ws", live_updates_ws)?;
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
                .with_list_parse_key("spacetimedb.databases")
                .with_list_parse_key("enabledimporter")
                .try_parsing(true),
        );
        let config = config.build()?;

        Ok(config.try_deserialize()?)
    }

    pub fn server_url(&self) -> anyhow::Result<SocketAddr> {
        match resolve_socket_addrs(self.host.as_str(), self.port) {
            Ok(addrs) => {
                if let Some(addr) = addrs.into_iter().next() {
                    return Ok(addr);
                }
            }
            Err(err) => return Err(err.into()),
        }

        Err(anyhow::anyhow!("Server URL resolution failed"))
    }

    #[allow(dead_code)]
    pub fn weboosocket_url(&self) -> String {
        format!(
            "{}{}/v1",
            self.spacetimedb.websocket_protocol, self.spacetimedb.domain
        )
    }

    #[allow(dead_code)]
    pub fn spacetimedb_url(&self) -> String {
        format!("{}{}", self.spacetimedb.protocol, self.spacetimedb.domain)
    }
}

fn resolve_socket_addrs(addr: &str, port: u16) -> Result<Vec<SocketAddr>, std::io::Error> {
    let addr_with_port = format!("{addr}:{port}");
    addr_with_port.to_socket_addrs().map(|iter| iter.collect())
}
