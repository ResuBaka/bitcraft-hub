-- Read-only export of a completed, WAL-counted interval. Widen the time
-- predicate around the desired xids to include the worker's earlier observations.
-- The trigger writes one unattributed row per changed state; attributed rows
-- are the original (uncoalesced) input to the explicit history append.
BEGIN READ ONLY;
SELECT json_build_object(
  'state', json_agg(json_build_object('entity_id', player_id, 'skill_id', skill_id,
             'experience', experience, 'region', region) ORDER BY recorded_at, ctid)
             FILTER (WHERE location_status IS NULL),
  'history', json_agg(json_build_array(recorded_at, player_id, skill_id, previous_experience,
             experience, region, location_status, claim_id) ORDER BY recorded_at, ctid)
             FILTER (WHERE location_status IS NOT NULL))
FROM public.player_experience_history
WHERE recorded_at BETWEEN :'lo'::timestamptz AND :'hi'::timestamptz
  AND xmin::text::bigint BETWEEN :first_xid AND :last_xid
GROUP BY xmin::text::bigint ORDER BY xmin::text::bigint;
COMMIT;
