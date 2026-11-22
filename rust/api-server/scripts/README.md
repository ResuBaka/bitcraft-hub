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

### Investigating a rising API RSS in production

Use one instance throughout and compare windows **after initial sync**, at similar
client counts and traffic. For example, from a host that can reach the API:

```sh
python3 scripts/worker_metrics.py --url http://app-api-live:8000/metrics --rss --samples 120 --interval 30
```

| What to graph alongside `process_resident_memory_bytes` | What to look for |
| --- | --- |
| `websocket_broadcast_queue_after_receive_messages` | Number left in the unbounded shared queue immediately after the broadcaster takes up to 500 messages. A sustained rise points to fan-out falling behind. It is a last-dequeue sample; it cannot see new arrivals while the broadcaster is stalled. |
| `websocket_client_queue_messages`, `websocket_largest_client_queue_messages`, `websocket_clients_connected_total` | Total and largest outgoing queue at scrape time. One queue approaching 1,999 indicates a slow writer; many partly full queues can retain substantial memory before any client is disconnected. Counts exclude the message being written and do not measure bytes. |
| `websocket_client_subscriptions` | Counts memberships, including the same topic on different sockets. Growth with a flat client count points to increasing subscription sets. The count should fall as clients leave. |
| Worker sampler above, `database_connection_pool_idle`, `database_query_duration_seconds` | Growing input queues indicate a consumer falling behind. Four claim DB writers (local, member, tech state and tech desc) have separate unbounded queues of row batches: the input sampler can show zero while they wait on Postgres. Check DB latency and pool pressure in the same window. |
| `app_state_cache_size{cache=~"buy_order_state\|sell_order_state\|mobile_entity_state"}`, `bitcraft_database_last_connected_timestamp_seconds` | These workers upsert initial rows but don't remove IDs missing from a fresh regional snapshot. Deletes missed during disconnect can persist. If cache size steps up after reconnects, compare against the live upstream row count for the same set of regions. |
| `leaderboard_registered_players`, `leaderboard_buffer_capacity_bytes` | Player slots are deliberately never reused. Check whether newly seen players or increased board capacity explain the trend. |
| `rate(http_requests_total{path="/market/orders"}[5m])` and proxy response sizes | `/market/orders?return_all=true` clones the full buy and sell books per request. Overlapping large responses can raise the heap peak and leave RSS elevated after they finish. The API metric groups both filtered and full requests under the same route. |

The WebSocket queue and subscription gauges above are available in the build
with the portal-listener fix. On earlier builds, the disabled portal-dimension
consumer still had an active producer: each initial portal, live insert, and
destination-changing update queued a `u32` until the regional connection ended.
This is separate from the measured `portal_state` worker. After sync, the
following is an **upper bound on new entries**, since not every update changes
the destination (exclude windows with reconnects):

```promql
sum by (instance) (increase(game_message_events{table="portal_state",type=~"insert|update"}[1h]))
```

For scale, the archived nine-region native run had just 4 live inserts and
14 updates between its 40 s and 340 s scrapes (at most 18 additional dimension
IDs). At that rate the queue does not explain a rise of tens of MB per hour;
check the production rate rather than assuming the fix accounts for the slope.

If the queues drain and cache counts plateau while RSS stays high, compare
live allocations on a representative canary before calling it a leak.
The [existing heap profile](../api/benches/memory-profile-2026-11-22.md)
shows substantial allocator retention after ingestion, but did not exercise
downstream clients or a representative HTTP load.

Tests:

```sh
python3 -m unittest discover -s scripts -p 'test_worker_metrics.py'
```
