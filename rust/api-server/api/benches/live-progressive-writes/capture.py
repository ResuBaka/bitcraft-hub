"""Capture post-coalescing progressive-action transactions from PostgreSQL 18.

Usage: python capture.py OUTPUT.json.gz SECONDS -- psql -XqAt DB
Only a session-local temporary table is written. Run against steady live traffic.
"""
import gzip
import json
import subprocess
import sys
import time
from itertools import groupby

out, seconds, sep, *cmd = sys.argv[1:]
assert sep == "--" and cmd
poll = """
WITH delta AS (
  MERGE INTO seen AS old
  USING (SELECT xmin AS version, t AS row FROM progressive_action_state t) AS new
  ON old.id = (new.row).entity_id
  WHEN MATCHED AND old.version <> new.version THEN UPDATE SET version = new.version
  WHEN NOT MATCHED BY TARGET THEN INSERT VALUES ((new.row).entity_id, new.version)
  WHEN NOT MATCHED BY SOURCE THEN DELETE
  RETURNING merge_action() AS action, old.id, new.version::text AS xid, new.row
) SELECT coalesce(jsonb_agg(to_jsonb(delta)), '[]') FROM delta;
"""
# Relabel IDs consistently across the seed, batches, deletes and secondary keys.
ids = {}


def alias(key):
    return ids.setdefault(key, len(ids) + 1)


def row(change):
    r = change["row"]
    for field in ("entity_id", "owner_entity_id", "building_entity_id"):
        r[field] = alias(r[field])
    return r


with subprocess.Popen(cmd + ["-v", "ON_ERROR_STOP=1"], stdin=subprocess.PIPE,
                      stdout=subprocess.PIPE, text=True) as p:
    def query(sql):
        p.stdin.write(sql + "\n")
        p.stdin.flush()
        return json.loads(p.stdout.readline())

    seed = query("CREATE TEMP TABLE seen(id bigint PRIMARY KEY, version xid NOT NULL);" + poll)
    capture = {"seed": [row(c) for c in seed], "steps": []}
    end = time.monotonic() + float(seconds)
    transactions = {}
    while time.monotonic() < end:
        time.sleep(0.25)
        changes = query(poll)
        deletes = [alias(c["id"]) for c in changes if c["action"] == "DELETE"]
        writes = sorted((c for c in changes if c["row"] is not None),
                        key=lambda c: (int(c["xid"]), c["id"]))
        for xid, group in groupby(writes, key=lambda c: c["xid"]):
            batch = list(group)
            assert xid not in transactions
            transactions[xid] = (len(batch), sum(c["action"] == "UPDATE" for c in batch))
            capture["steps"].append({"upsert": [row(c) for c in batch]})
        if deletes:
            capture["steps"].append({"delete": deletes})
    p.stdin.close()
    assert p.wait() == 0

with gzip.open(out, "wt") as f:
    json.dump(capture, f, separators=(",", ":"))
# Used to check batch completeness against the table's insert/update WAL records.
for xid, (size, updates) in transactions.items():
    print(xid, size, updates)
