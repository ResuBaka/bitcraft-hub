CREATE TABLE watchtower_history (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tower_id bigint NOT NULL,
    observed_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    source text NOT NULL,
    event text NOT NULL,
    before_state jsonb,
    after_state jsonb
);
-- No FK to the current mirror: a destroyed tower still has a timeline.
CREATE INDEX watchtower_history_timeline ON watchtower_history (tower_id, id DESC);
CREATE INDEX empire_watchtower_reports ON empire_notification_state
    ((text_replacement[array_length(text_replacement, 1)]), timestamp DESC)
    WHERE notification_type IN (2,3,4,5,6,7,8,10);
CREATE INDEX watchtower_history_departures ON watchtower_history
    ((before_state->>'empire_entity_id'), tower_id, id DESC)
    WHERE source = 'tower' AND before_state->>'empire_entity_id' IS DISTINCT FROM after_state->>'empire_entity_id';

CREATE FUNCTION record_watchtower_observation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    previous jsonb;
    current_row jsonb;
BEGIN
    IF TG_OP <> 'INSERT' THEN previous := to_jsonb(OLD); END IF;
    IF TG_OP <> 'DELETE' THEN current_row := to_jsonb(NEW); END IF;
    IF previous IS DISTINCT FROM current_row THEN
        INSERT INTO watchtower_history (tower_id, source, event, before_state, after_state)
        VALUES ((coalesce(current_row, previous)->>TG_ARGV[1])::bigint, TG_ARGV[0],
                CASE TG_OP WHEN 'INSERT' THEN 'seen' WHEN 'UPDATE' THEN 'changed' ELSE 'removed' END,
                previous, current_row);
    END IF;
    RETURN NULL;
END $$;

-- Installed and seeded together in the migration transaction. These triggers
-- capture both live writes and real differences reconciled after a disconnect.
CREATE TRIGGER watchtower_observation AFTER INSERT OR UPDATE OR DELETE ON empire_node_state
    FOR EACH ROW EXECUTE FUNCTION record_watchtower_observation('tower', 'entity_id');
CREATE TRIGGER watchtower_observation AFTER INSERT OR UPDATE OR DELETE ON empire_node_siege_state
    FOR EACH ROW EXECUTE FUNCTION record_watchtower_observation('siege', 'building_entity_id');
INSERT INTO watchtower_history (tower_id, source, event, after_state)
    SELECT entity_id, 'tower', 'baseline', to_jsonb(n) FROM empire_node_state n;
INSERT INTO watchtower_history (tower_id, source, event, after_state)
    SELECT building_entity_id, 'siege', 'baseline', to_jsonb(s) FROM empire_node_siege_state s;
