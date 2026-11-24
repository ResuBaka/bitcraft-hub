CREATE TABLE empire_state (
    entity_id bigint PRIMARY KEY,
    capital_building_entity_id bigint NOT NULL,
    name text NOT NULL,
    shard_treasury bigint NOT NULL,
    nobility_threshold integer NOT NULL,
    num_claims integer NOT NULL,
    x integer NOT NULL, z integer NOT NULL, dimension bigint NOT NULL,
    empire_currency_treasury bigint NOT NULL,
    owner_type smallint NOT NULL
);
CREATE INDEX empire_name_idx ON empire_state (lower(name));

CREATE TABLE empire_player_data_state (
    entity_id bigint PRIMARY KEY,
    empire_entity_id bigint NOT NULL,
    rank smallint NOT NULL,
    donated_shards bigint NOT NULL,
    noble timestamptz,
    donated_empire_currency bigint NOT NULL
);
CREATE INDEX empire_member_rank_idx ON empire_player_data_state (empire_entity_id, rank);

CREATE TABLE empire_rank_state (
    entity_id bigint PRIMARY KEY,
    empire_entity_id bigint NOT NULL,
    rank smallint NOT NULL,
    title text NOT NULL,
    permissions boolean[] NOT NULL
);
CREATE INDEX empire_rank_idx ON empire_rank_state (empire_entity_id, rank);

CREATE TABLE empire_settlement_state (
    building_entity_id bigint PRIMARY KEY,
    claim_entity_id bigint NOT NULL,
    empire_entity_id bigint NOT NULL,
    chunk_index bigint NOT NULL,
    can_house_empire_storehouse boolean NOT NULL,
    members_donations bigint NOT NULL,
    x integer NOT NULL, z integer NOT NULL, dimension bigint NOT NULL
);
CREATE INDEX empire_settlement_claim_idx ON empire_settlement_state (claim_entity_id);
CREATE INDEX empire_settlement_owner_idx ON empire_settlement_state (empire_entity_id);

CREATE TABLE empire_node_state (
    entity_id bigint PRIMARY KEY,
    empire_entity_id bigint NOT NULL,
    chunk_index bigint NOT NULL,
    energy integer NOT NULL,
    active boolean NOT NULL,
    upkeep integer NOT NULL,
    x integer NOT NULL, z integer NOT NULL, dimension bigint NOT NULL
);
CREATE INDEX empire_node_owner_idx ON empire_node_state (empire_entity_id);

CREATE TABLE empire_chunk_state (
    chunk_index bigint PRIMARY KEY,
    empire_entity_id bigint NOT NULL,
    watchtower_entity_id bigint NOT NULL
);
CREATE INDEX empire_chunk_owner_idx ON empire_chunk_state (empire_entity_id);
CREATE INDEX empire_chunk_watchtower_idx ON empire_chunk_state (watchtower_entity_id);

CREATE TABLE empire_node_siege_state (
    entity_id bigint PRIMARY KEY,
    building_entity_id bigint NOT NULL,
    empire_entity_id bigint NOT NULL,
    energy integer NOT NULL,
    active boolean NOT NULL,
    start_timestamp timestamptz
);
CREATE INDEX empire_siege_building_idx ON empire_node_siege_state (building_entity_id);
CREATE INDEX empire_siege_owner_idx ON empire_node_siege_state (empire_entity_id);

CREATE TABLE empire_emblem_state (
    entity_id bigint PRIMARY KEY,
    icon_id integer NOT NULL, shape_id integer NOT NULL,
    color_1_id integer NOT NULL, color_2_id integer NOT NULL
);
CREATE TABLE empire_directive_state (
    entity_id bigint PRIMARY KEY,
    directive_message text NOT NULL,
    directive_message_timestamp timestamptz
);
CREATE TABLE empire_foundry_state (
    entity_id bigint PRIMARY KEY,
    empire_entity_id bigint NOT NULL,
    hexite_capsules integer NOT NULL, queued integer NOT NULL, started timestamptz NOT NULL
);
CREATE INDEX empire_foundry_owner_idx ON empire_foundry_state (empire_entity_id);
CREATE TABLE empire_siege_engine_state (
    entity_id bigint PRIMARY KEY, building_entity_id bigint NOT NULL
);
CREATE INDEX empire_engine_building_idx ON empire_siege_engine_state (building_entity_id);
CREATE TABLE empire_notification_state (
    entity_id bigint PRIMARY KEY, empire_entity_id bigint NOT NULL,
    notification_type smallint NOT NULL, text_replacement text[] NOT NULL, timestamp integer NOT NULL
);
CREATE INDEX empire_notification_feed_idx ON empire_notification_state (empire_entity_id, timestamp DESC);

CREATE TABLE empire_rank_desc (
    rank integer PRIMARY KEY, title text NOT NULL, max_count integer, permissions boolean[] NOT NULL
);
CREATE TABLE empire_color_desc (id integer PRIMARY KEY, color_argb bigint NOT NULL, color_2_argb bigint);
CREATE TABLE empire_icon_desc (id integer PRIMARY KEY, icon_unicode text NOT NULL, is_shape boolean NOT NULL);
CREATE TABLE empire_supplies_desc (cargo_id integer PRIMARY KEY, energy integer NOT NULL);
CREATE TABLE empire_territory_desc (
    id smallint PRIMARY KEY, chunks integer NOT NULL, ranks smallint[] NOT NULL, crown_collectible_id integer NOT NULL
);
CREATE TABLE empire_notification_desc (
    id integer PRIMARY KEY, notification_type smallint NOT NULL,
    priority integer NOT NULL, show_on_login boolean NOT NULL, text text NOT NULL
);
