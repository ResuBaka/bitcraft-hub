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
    pub download: DownloadConfig,
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
            download: DownloadConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DownloadConfig {
    pub desc_tables: Vec<String>,
    pub state_tables: Vec<String>,
    pub rest_tables: Vec<String>,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            desc_tables: vec![
                "achievement_desc".to_string(),
                "alert_desc".to_string(),
                "biome_desc".to_string(),
                "buff_desc".to_string(),
                "buff_type_desc".to_string(),
                "building_claim_desc".to_string(),
                "building_desc".to_string(),
                "building_function_type_mapping_desc".to_string(),
                "building_portal_desc".to_string(),
                "building_repairs_desc".to_string(),
                "building_spawn_desc".to_string(),
                "building_type_desc".to_string(),
                "cargo_desc".to_string(),
                "character_stat_desc".to_string(),
                "chest_rarity_desc".to_string(),
                "claim_tech_desc".to_string(),
                "climb_requirement_desc".to_string(),
                "clothing_desc".to_string(),
                "collectible_desc".to_string(),
                "combat_action_desc".to_string(),
                "construction_recipe_desc".to_string(),
                "crafting_recipe_desc".to_string(),
                "deconstruction_recipe_desc".to_string(),
                "deployable_desc".to_string(),
                "dimension_description_state".to_string(),
                "distant_visible_entity_desc".to_string(),
                "elevator_desc".to_string(),
                "emote_desc".to_string(),
                "empire_color_desc".to_string(),
                "empire_icon_desc".to_string(),
                "empire_notification_desc".to_string(),
                "empire_rank_desc".to_string(),
                "empire_supplies_desc".to_string(),
                "empire_territory_desc".to_string(),
                "enemy_ai_params_desc".to_string(),
                "enemy_desc".to_string(),
                "environment_debuff_desc".to_string(),
                "equipment_desc".to_string(),
                "extraction_recipe_desc".to_string(),
                "food_desc".to_string(),
                "gate_desc".to_string(),
                "interior_instance_desc".to_string(),
                "interior_network_desc".to_string(),
                "interior_portal_connections_desc".to_string(),
                "interior_shape_desc".to_string(),
                "interior_spawn_desc".to_string(),
                "item_conversion_recipe_desc".to_string(),
                "item_desc".to_string(),
                "item_list_desc".to_string(),
                "knowledge_scroll_desc".to_string(),
                "knowledge_scroll_type_desc".to_string(),
                "knowledge_stat_modifier_desc".to_string(),
                "loot_chest_desc".to_string(),
                "loot_rarity_desc".to_string(),
                "loot_table_desc".to_string(),
                "npc_desc".to_string(),
                "onboarding_reward_desc".to_string(),
                "parameters_desc".to_string(),
                "parameters_player_move_desc".to_string(),
                "pathfinding_desc".to_string(),
                "paving_tile_desc".to_string(),
                "pillar_shaping_desc".to_string(),
                "player_action_desc".to_string(),
                "player_housing_desc".to_string(),
                "private_parameters_desc".to_string(),
                "resource_clump_desc".to_string(),
                "resource_desc".to_string(),
                "resource_growth_recipe_desc".to_string(),
                "resource_placement_recipe_desc".to_string(),
                "secondary_knowledge_desc".to_string(),
                "single_resource_to_clump_desc".to_string(),
                "skill_desc".to_string(),
                "targeting_matrix_desc".to_string(),
                "teleport_item_desc".to_string(),
                "terraform_recipe_desc".to_string(),
                "tool_desc".to_string(),
                "tool_type_desc".to_string(),
                "traveler_task_desc".to_string(),
                "traveler_trade_order_desc".to_string(),
                "wall_desc".to_string(),
                "weapon_desc".to_string(),
                "weapon_type_desc".to_string(),
            ],
            state_tables: vec![
                "a_i_debug_state".to_string(),
                "action_state".to_string(),
                "active_buff_state".to_string(),
                "alert_state".to_string(),
                "attached_herds_state".to_string(),
                "attack_outcome_state".to_string(),
                "auto_claim_state".to_string(),
                "barter_stall_state".to_string(),
                "building_nickname_state".to_string(),
                "building_state".to_string(),
                "buy_order_state".to_string(),
                "character_stats_state".to_string(),
                "chat_message_state".to_string(),
                "claim_local_state".to_string(),
                "claim_member_state".to_string(),
                "claim_recruitment_state".to_string(),
                "claim_state".to_string(),
                "claim_tech_state".to_string(),
                "claim_tile_state".to_string(),
                "closed_listing_state".to_string(),
                "combat_state".to_string(),
                "deployable_collectible_state".to_string(),
                "deployable_state".to_string(),
                "dimension_description_state".to_string(),
                "dimension_network_state".to_string(),
                "dropped_inventory_state".to_string(),
                "duel_state".to_string(),
                "empire_chunk_state".to_string(),
                "empire_directive_state".to_string(),
                "empire_emblem_state".to_string(),
                "empire_expansion_state".to_string(),
                "empire_foundry_state".to_string(),
                "empire_log_state".to_string(),
                "empire_node_siege_state".to_string(),
                "empire_node_state".to_string(),
                "empire_notification_state".to_string(),
                "empire_player_data_state".to_string(),
                "empire_player_log_state".to_string(),
                "empire_rank_state".to_string(),
                "empire_settlement_state".to_string(),
                "empire_siege_engine_state".to_string(),
                "empire_state".to_string(),
                "enemy_mob_monitor_state".to_string(),
                "enemy_state".to_string(),
                "equipment_state".to_string(),
                "experience_state".to_string(),
                "exploration_chunks_state".to_string(),
                "extract_outcome_state".to_string(),
                "footprint_tile_state".to_string(),
                "global_search_state".to_string(),
                "growth_state".to_string(),
                "health_state".to_string(),
                "herd_state".to_string(),
                "interior_collapse_trigger_state".to_string(),
                "inventory_state".to_string(),
                "knowledge_achievement_state".to_string(),
                "knowledge_battle_action_state".to_string(),
                "knowledge_building_state".to_string(),
                "knowledge_cargo_state".to_string(),
                "knowledge_claim_state".to_string(),
                "knowledge_construction_state".to_string(),
                "knowledge_craft_state".to_string(),
                "knowledge_deployable_state".to_string(),
                "knowledge_enemy_state".to_string(),
                "knowledge_extract_state".to_string(),
                "knowledge_item_state".to_string(),
                "knowledge_lore_state".to_string(),
                "knowledge_npc_state".to_string(),
                "knowledge_paving_state".to_string(),
                "knowledge_pillar_shaping_state".to_string(),
                "knowledge_resource_placement_state".to_string(),
                "knowledge_resource_state".to_string(),
                "knowledge_ruins_state".to_string(),
                "knowledge_secondary_state".to_string(),
                "knowledge_vault_state".to_string(),
                "light_source_state".to_string(),
                "location_state".to_string(),
                "loot_chest_state".to_string(),
                "lost_items_state".to_string(),
                "mobile_entity_state".to_string(),
                "mounting_state".to_string(),
                "move_validation_strike_counter_state".to_string(),
                "npc_state".to_string(),
                "onboarding_state".to_string(),
                "passive_craft_state".to_string(),
                "paved_tile_state".to_string(),
                "permission_state".to_string(),
                "pillar_shaping_state".to_string(),
                "player_action_state".to_string(),
                "player_housing_state".to_string(),
                "player_lowercase_username_state".to_string(),
                "player_note_state".to_string(),
                "player_prefs_state".to_string(),
                "player_queue_state".to_string(),
                "player_report_state".to_string(),
                "player_shard_state".to_string(),
                "player_state".to_string(),
                "player_timestamp_state".to_string(),
                "player_username_state".to_string(),
                "player_vote_state".to_string(),
                "portal_state".to_string(),
                "progressive_action_state".to_string(),
                "project_site_state".to_string(),
                "rent_state".to_string(),
                "resource_health_state".to_string(),
                "resource_state".to_string(),
                "rez_sick_long_term_state".to_string(),
                "satiation_state".to_string(),
                "sell_order_state".to_string(),
                "signed_in_player_state".to_string(),
                "stamina_state".to_string(),
                "starving_player_state".to_string(),
                "target_state".to_string(),
                "targetable_state".to_string(),
                "teleportation_energy_state".to_string(),
                "terraform_progress_state".to_string(),
                "terrain_chunk_state".to_string(),
                "threat_state".to_string(),
                "toolbar_state".to_string(),
                "trade_order_state".to_string(),
                "trade_session_state".to_string(),
                "traveler_task_state".to_string(),
                "unclaimed_collectibles_state".to_string(),
                "unclaimed_shards_state".to_string(),
                "user_authentication_state".to_string(),
                "user_moderation_state".to_string(),
                "user_previous_region_state".to_string(),
                "user_region_state".to_string(),
                "user_state".to_string(),
                "vault_state".to_string(),
                "world_region_name_state".to_string(),
                "world_region_state".to_string(),
            ],
            rest_tables: vec![
                "admin_broadcast".to_string(),
                "claim_tile_cost".to_string(),
                "config".to_string(),
                "distant_visible_entity".to_string(),
                "empire_craft_supplies_timer".to_string(),
                "empire_decay_loop_timer".to_string(),
                "empire_siege_loop_timer".to_string(),
                "force_generate_types".to_string(),
                "globals".to_string(),
                "identity_role".to_string(),
                "inter_module_message".to_string(),
                "inter_module_message_counter".to_string(),
                "player_notification_event".to_string(),
                "player_vote_conclude_timer".to_string(),
                "region_connection_info".to_string(),
                "region_population_info".to_string(),
                "resource_count".to_string(),
                "server_identity".to_string(),
                "staged_static_data".to_string(),
                "the_great_placeholder_table".to_string(),
            ],
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
pub(crate) enum ImportType {
    File,
    Game,
}

impl Default for ImportType {
    fn default() -> Self {
        Self::File
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
            max_connections: 100,
            min_connections: 5,
            connect_timeout: 15,
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
}

impl Default for SpacetimeDbConfig {
    fn default() -> Self {
        Self {
            domain: "localhost".to_string(),
            protocol: "https://".to_string(),
            database: "".to_string(),
            databases: vec!["".to_string()],
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
    let addr_with_port = format!("{}:{}", addr, port);
    addr_with_port.to_socket_addrs().map(|iter| iter.collect())
}
