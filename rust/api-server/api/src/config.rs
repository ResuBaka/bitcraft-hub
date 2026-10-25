use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{SocketAddr, ToSocketAddrs};
use tracing::Level;

pub(crate) type TechTierResearchMap = std::collections::HashMap<i32, i32>;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: DatabaseConfig,
    pub(crate) log_type: LogType,
    pub(crate) log_level: LogLevel,
    pub(crate) spacetimedb: SpacetimeDbConfig,
    pub(crate) shutdown: ShutdownConfig,
    pub(crate) leaderboard: LeaderboardConfig,
    #[serde(default)]
    pub(crate) origins: AllowedOriginConfig,
    #[serde(alias = "liveupdatesws")]
    pub(crate) live_updates_ws: bool,
    pub(crate) tech_tier_research_map: TechTierResearchMap,
}

impl Default for Config {
    fn default() -> Self {
        let mut tech_tier_research_map = std::collections::HashMap::new();
        tech_tier_research_map.insert(200, 2);
        tech_tier_research_map.insert(300, 3);
        tech_tier_research_map.insert(400, 4);
        tech_tier_research_map.insert(500, 5);
        tech_tier_research_map.insert(600, 6);
        tech_tier_research_map.insert(700, 7);
        tech_tier_research_map.insert(800, 8);
        tech_tier_research_map.insert(900, 9);
        tech_tier_research_map.insert(1000, 10);

        Self {
            host: "localhost".to_string(),
            port: 8000,
            database: DatabaseConfig::default(),
            log_type: LogType::default(),
            log_level: LogLevel::default(),
            spacetimedb: SpacetimeDbConfig::default(),
            shutdown: ShutdownConfig::default(),
            leaderboard: LeaderboardConfig::default(),
            origins: AllowedOriginConfig::default(),
            live_updates_ws: false,
            tech_tier_research_map,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct LeaderboardConfig {
    /// Fixed bucket widths keyed by metric board name (e.g. level, skill_1).
    /// Unspecified boards retain the existing tiered buckets; level defaults to 50.
    pub(crate) bucket_widths: std::collections::HashMap<String, std::num::NonZeroU32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ShutdownConfig {
    /// Stop ingestion/HTTP after this many seconds of serving. None disables it.
    pub(crate) after_seconds: Option<u64>,
    /// Additional time allowed to drain accepted work.
    pub(crate) grace_seconds: u64,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            after_seconds: None,
            grace_seconds: 60,
        }
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    #[test]
    fn leaderboard_bucket_widths_are_optional_and_positive() {
        let default: Config = serde_json::from_str("{}").unwrap();
        assert!(default.leaderboard.bucket_widths.is_empty());
        let configured: Config =
            serde_json::from_str(r#"{"leaderboard":{"bucket_widths":{"level":5}}}"#).unwrap();
        assert_eq!(configured.leaderboard.bucket_widths["level"].get(), 5);
        for width in ["0", "-1", "1.5"] {
            assert!(
                serde_json::from_str::<Config>(&format!(
                    r#"{{"leaderboard":{{"bucket_widths":{{"level":{width}}}}}}}"#
                ))
                .is_err()
            );
        }
    }
    #[test]
    fn default_is_unlimited_and_six_minute_config_is_explicit() {
        let default: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(default.shutdown.after_seconds, None);
        assert_eq!(default.shutdown.grace_seconds, 60);
        let timed: Config =
            serde_json::from_str(r#"{"shutdown":{"after_seconds":360,"grace_seconds":90}}"#)
                .unwrap();
        assert_eq!(timed.shutdown.after_seconds, Some(360));
        assert_eq!(timed.shutdown.grace_seconds, 90);
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
            max_connections: 50,
            min_connections: 3,
            connect_timeout: 10,
            idle_timeout: 90,
            max_lifetime: Some(150),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct SpacetimeDbConfig {
    /// Application-side initial snapshot chunks for inventory/vault only.
    /// Zero restores the original single-vector handoff for A/B comparisons.
    pub(crate) initial_batch_rows: usize,
    /// Temporary RSS isolation: keep connections/workers but subscribe to nothing.
    /// Set false to restore state-table synchronization for the comparison.
    pub(crate) skip_state_tables: bool,
    pub(crate) domain: String,
    pub(crate) protocol: String,
    pub(crate) database: String,
    pub(crate) databases: Vec<String>,
    pub(crate) password: String,
    pub(crate) username: String,
    pub(crate) websocket_protocol: String,
}

impl Default for SpacetimeDbConfig {
    fn default() -> Self {
        Self {
            // TEMPORARY: zero subscriptions by default while diagnosing sync RSS.
            skip_state_tables: false,
            initial_batch_rows: 1_000,
            domain: "localhost".to_string(),
            protocol: "https://".to_string(),
            database: "".to_string(),
            databases: vec![],
            password: "".to_string(),
            username: "token".to_string(),
            websocket_protocol: "wss://".to_string(),
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub(crate) enum LogType {
    #[default]
    Default,
    #[serde(alias = "json")]
    Json,
    #[serde(alias = "pretty")]
    Pretty,
}

#[derive(Default)]
pub(crate) struct CliConfigParameters {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) database_url: Option<String>,
    pub(crate) storage_path: Option<String>,
    pub(crate) config_path: Option<String>,
    pub(crate) live_updates_ws: Option<bool>,
    pub(crate) shutdown_after_seconds: Option<u64>,
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
            if let Some(seconds) = cli_config_parameters.shutdown_after_seconds {
                cli_overrides = cli_overrides.set_default("shutdown.after_seconds", seconds)?;
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
        {
            let addrs = resolve_socket_addrs(self.host.as_str(), self.port)?;
            if let Some(addr) = addrs.into_iter().next() {
                return Ok(addr);
            }
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
