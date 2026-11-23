"""Check snapshot polling didn't miss or combine live upserts (PostgreSQL 18).

Args: capture.py stdout, pg_waldump -r Heap -R TABLESPACE/DB/RELFILE output
spanning the capture. The latter must come from the same table on the source.
"""
from collections import Counter, defaultdict
import re
import sys

with open(sys.argv[1]) as f:
    batches = [tuple(map(int, line.split())) for line in f]
assert batches, "capture had no upserts"
first, last = batches[0][0], batches[-1][0]
wal = defaultdict(Counter)
with open(sys.argv[2]) as f:
    for line in f:
        m = re.search(r"tx:\s*(\d+),.*desc: (INSERT|HOT_UPDATE|UPDATE)\b", line)
        if m and first <= int(m[1]) <= last:
            wal[int(m[1])][m[2]] += 1
expected = {xid: (n, conflicts) for xid, n, conflicts in batches}
actual = {xid: (c.total(), c["UPDATE"] + c["HOT_UPDATE"]) for xid, c in wal.items()}
if expected != actual:
    raise ValueError(f"incomplete capture: {actual.items() ^ expected.items()}")
print(f"Verified {len(actual)} upsert transactions and their insert/conflict counts")
