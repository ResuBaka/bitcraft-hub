# Worker-message metrics

Run from `rust/api-server`, with the API server running:

```sh
python3 scripts/worker_metrics.py
python3 scripts/worker_metrics.py --samples 3 --interval 5
python3 scripts/worker_metrics.py --worker inventory_state --worker experience_state --samples 3
python3 scripts/worker_metrics.py --worker inventory_state --worker vault_state --rss --samples 3
```

The helper streams `http://localhost:8000/metrics` and parses only worker
metric families. It prints sent/received/error counts, approximate outstanding
input-channel messages, the last receive batch size, and sampled rates. Use
`--url` for another endpoint. It needs no external Python packages.
`--rss` additionally reads only `process_resident_memory_bytes` and prints it in
MiB and decimal GB, when exposed by the server.

Important limits:

- `queued~ = sent - send_errors - received` is approximate, not an atomic queue
  gauge. Small discrepancies can occur during scraping.
- A received message can still be executing. A whole initial snapshot counts as
  **one message**, not its number of rows or bytes.
- These metrics do **not** measure database-write queues, allocator retention,
  or application/SDK caches. Empty input queues do not establish that writes are
  finished or explain process RSS.
- Missing counters show `-`, never an assumed zero. Negative queue estimates
  show `!labels`; check instrumentation/reset mismatches before interpreting them.
- Experience receives previously used the `player_state` label, and deployable
  sends/receives used different names. Source labels are corrected; restart the
  server to get clean counters. Old running binaries still expose bad estimates.

When inventory/vault initial chunking is enabled, a second table shows their
private initial-batch subqueues: sent, received, discarded and approximate queued
batches, plus completed input markers read. These are separate from the main
worker queue, where each entire snapshot still counts as one envelope. `end read`
does not guarantee database persistence has finished.

Tests:

```sh
python3 -m unittest discover -s scripts -p 'test_worker_metrics.py'
```
