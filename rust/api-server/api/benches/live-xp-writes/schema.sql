CREATE TABLE experience_state (entity_id bigint NOT NULL, skill_id integer NOT NULL,
    experience integer NOT NULL, region smallint NOT NULL, PRIMARY KEY(entity_id, skill_id));
CREATE INDEX ON experience_state(entity_id, skill_id);
CREATE INDEX ON experience_state(region);
CREATE TABLE player_experience_history (recorded_at timestamptz NOT NULL, player_id bigint NOT NULL,
    skill_id integer NOT NULL, previous_experience integer, experience integer NOT NULL,
    region smallint NOT NULL, location_status text, claim_id bigint, attributed_experience bigint,
    legacy_claim_week_archived boolean NOT NULL DEFAULT false,
    CHECK ((location_status IS NULL AND claim_id IS NULL) OR
           (location_status IN ('unknown','unclaimed') AND claim_id IS NULL) OR
           (location_status='claim' AND claim_id IS NOT NULL)));
SELECT create_hypertable('player_experience_history', by_range('recorded_at', INTERVAL '1 day'));
CREATE INDEX ON player_experience_history(player_id, recorded_at DESC, skill_id);
CREATE FUNCTION record_player_experience_history() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' OR OLD.experience IS DISTINCT FROM NEW.experience THEN
        INSERT INTO player_experience_history (recorded_at, player_id, skill_id, previous_experience, experience, region)
        VALUES (clock_timestamp(), NEW.entity_id, NEW.skill_id, NULL, NEW.experience, NEW.region);
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER experience_history_record AFTER INSERT OR UPDATE ON experience_state
    FOR EACH ROW EXECUTE FUNCTION record_player_experience_history();
