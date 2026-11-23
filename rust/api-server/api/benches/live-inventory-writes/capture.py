"""Private fixture from a steady-state PostgreSQL 18 inventory writer (no DB writes).
Usage: python3 capture.py CONTAINER DB SECONDS OUTPUT.json.gz
Uses physical WAL only to locate rows; fetches payloads via read-only CTID lookups.
"""
import collections
import gzip
import json
import os
import re
import subprocess
import sys
import time

container, db, seconds, out = sys.argv[1:]
cmd = ["docker", "exec", "-i", "-u", "postgres", container]
psql = cmd + ["psql", "-XqAt", "-v", "ON_ERROR_STOP=1", "-d", db]


def query(p, sql):
    p.stdin.write(sql + ";\n")
    p.stdin.flush()
    return json.loads(p.stdout.readline())


def rows(p, predicate):
    return query(p, "SELECT coalesce(json_agg(json_build_array(xmin::text,ctid::text,"
                 "row_to_json(t))),'[]') FROM public.inventory t WHERE " + predicate)


def records(wal, rel, lo, hi):
    text = subprocess.check_output(cmd + ["pg_waldump", "-p", wal, "-r", "Heap", "-R", rel,
                                          "-s", lo, "-e", hi], text=True)
    for line in text.splitlines():
        m = re.search(r"tx:\s*(\d+),.*desc: (INSERT|UPDATE|HOT_UPDATE)\b", line)
        if m:
            block = re.search(r"blkref #0: rel \S+ blk (\d+)", line)[1]
            offset = re.search(r"\b(?:new_off|off): (\d+)", line)[1]
            yield (int(m[1]), f"({block},{offset})"), m[2] == "INSERT"


with subprocess.Popen(psql, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as live, \
     subprocess.Popen(psql, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as seed:
    path, wal, cursor = query(live, "SELECT json_build_array(pg_relation_filepath('public.inventory'),"
                              "current_setting('data_directory')||'/pg_wal',pg_current_wal_flush_lsn())")
    match = re.fullmatch(r"base/(\d+)/(\d+)", path)
    assert match, "fixture expects a table in the default tablespace"
    rel = "1663/" + "/".join(match.groups())
    start_lsn = cursor
    first = query(seed, "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SELECT "
                  "pg_snapshot_xmax(pg_current_snapshot())::text::bigint")
    batches = collections.defaultdict(list)
    versions = {}
    waiting = set()
    end = time.monotonic() + int(seconds)
    while time.monotonic() < end:
        time.sleep(.4)
        upper = query(live, "SELECT to_json(pg_current_wal_flush_lsn())")
        if upper == cursor:
            continue
        changes = records(wal, rel, cursor, upper)
        cursor = upper
        for key, inserted in changes:
            if key[0] >= first:
                batches[key[0]].append((key, inserted))
                waiting.add(key)
        if waiting:
            tids = ",".join(f"'{tid}'::tid" for tid in {key[1] for key in waiting})
            for xid, tid, row in rows(live, f"ctid IN ({tids})"):
                key = (int(xid), tid)
                if key in waiting:
                    versions[key] = row
                    waiting.remove(key)
    # The last transaction can straddle the WAL cutoff; earlier flushes by this
    # serial writer must all be complete. Never substitute a later tuple version.
    last = max(batches)
    del batches[last]
    assert batches and not any(xid != last for xid, _ in waiting), "missing committed versions"
    # Independent continuous pass catches missing WAL records at poll boundaries.
    checked = collections.defaultdict(list)
    for key, inserted in records(wal, rel, start_lsn, cursor):
        if first <= key[0] < last:
            checked[key[0]].append((key, inserted))
    assert checked == batches, "incomplete WAL transaction"
    steps = []
    touched = set()
    for xid, records in batches.items():
        batch = [versions[key] for key, _ in records]
        ids = [r["entity_id"] for r in batch]
        assert len(set(ids)) == len(ids)
        touched.update(ids)
        steps.append({"rows": batch, "inserted": [r["entity_id"] for r, (_, ins) in zip(batch, records) if ins]})
    initial = []
    ids = sorted(touched)
    for i in range(0, len(ids), 1000):
        predicate = "entity_id IN (" + ",".join(map(str, ids[i:i+1000])) + ")"
        initial.extend(r for _, _, r in rows(seed, predicate))
    for p in (seed, live):
        p.stdin.close()
        assert p.wait() == 0

os.umask(0o077)
with gzip.open(out, "xt") as f:
    json.dump({"seed": initial, "steps": steps}, f, separators=(",", ":"))
print("flushes:", len(steps), "rows:", sum(len(s["rows"]) for s in steps), "seed:", len(initial))
