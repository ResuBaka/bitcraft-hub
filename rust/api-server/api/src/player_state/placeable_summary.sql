WITH RECURSIVE stages AS (
    SELECT p.placeable_id, t.entity_id IS NOT NULL AS growing, count(*) AS quantity,
           min(t.ends_at_ms) AS ends_at_ms
    FROM placeable_state p
    LEFT JOIN placeable_growth_timer t ON t.entity_id = p.entity_id
    LEFT JOIN placeable_desc d ON d.id = p.placeable_id
    WHERE p.owner_entity_id = $1 AND (NOT $2 OR d.tag IN ('Farmed Crop','Rice Farming'))
    GROUP BY p.placeable_id, t.entity_id IS NOT NULL
), edges AS (
    SELECT placeable_id, unnest(next_ids) AS dest FROM placeable_growth_desc
    UNION ALL
    SELECT placeable_id, unnest(next_ids) FROM placeable_interaction_desc
    WHERE NOT (verb_phrase = ANY($3::text[]))
), successor AS (
    -- Do not promise a particular outcome for random or branching transitions.
    SELECT placeable_id, min(dest) AS dest FROM edges GROUP BY placeable_id
    HAVING min(dest) = max(dest) AND min(dest) <> 0
), walk(stage_id, dest) AS (
    SELECT placeable_id, placeable_id FROM stages
    UNION
    SELECT w.stage_id, s.dest FROM walk w JOIN successor s ON s.placeable_id = w.dest
), outcome AS (
    -- UNION terminates even on cycles. A cycle with no terminal descriptor
    -- falls back to the observed stage, as does incomplete descriptor data.
    SELECT w.stage_id, w.dest FROM walk w
    JOIN placeable_desc d ON d.id = w.dest
    WHERE NOT EXISTS (SELECT 1 FROM successor s WHERE s.placeable_id = w.dest)
), details AS (
    SELECT s.*, d.name, d.tier, o.dest,
           ARRAY(SELECT DISTINCT verb_phrase FROM placeable_interaction_desc i
                 WHERE i.placeable_id = s.placeable_id AND i.verb_phrase <> ''
                 ORDER BY verb_phrase) AS interactions
    FROM stages s LEFT JOIN placeable_desc d ON d.id = s.placeable_id
    LEFT JOIN outcome o ON o.stage_id = s.placeable_id
)
SELECT $1::bigint::text AS owner_id, coalesce(dest, placeable_id) AS outcome_id,
       coalesce(goal.name, s.name, 'Placeable #' || placeable_id) AS outcome_name,
       coalesce(goal.tier, s.tier) AS outcome_tier, placeable_id AS stage_id,
       coalesce(s.name, 'Placeable #' || placeable_id) AS name,
       quantity, growing, ends_at_ms, interactions,
       -- An empty net may allow "Collect" but only return the net. Earlier
       -- stages count as ready if collection already yields the outcome's
       -- reward (e.g. a choppable birch that could also grow to maturity).
       CASE WHEN $2 THEN 'Harvest' = ANY(interactions) ELSE EXISTS (
           SELECT 1 FROM placeable_interaction_desc i
           WHERE i.placeable_id = s.placeable_id AND i.verb_phrase = ANY($3::text[])
             AND (s.dest IS NULL OR s.dest = s.placeable_id OR EXISTS (
                 SELECT 1 FROM placeable_interaction_desc final
                 WHERE final.placeable_id = s.dest AND final.verb_phrase = ANY($3::text[])
                   AND i.reward_keys && final.reward_keys
             ))
       ) END AS ready
FROM details s LEFT JOIN placeable_desc goal ON goal.id = s.dest
ORDER BY outcome_name, outcome_id, ready DESC, ends_at_ms NULLS LAST, stage_id, growing
