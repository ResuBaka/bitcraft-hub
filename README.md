# Bitcraft Hub

> Not affiliated with Clockwork Labs
 
> This project is a work in progress and is not yet ready for production use. On that note we currently do not have an easy setup for contributing to this project. As you need to have all the game database state.

## Service needed to run

### Client

* Api (rust/api-server)

### Server
* Bitcraft Online (SpacetimeDB)
* TimescaleDB (Postgress)

# Setup

## Frontend

We are using **Bun** for the frontend part which can be found in the frontend folder.

Make sure to install the dependencies:

```bash
bun install
```

### Development Server

Start the development server on `http://localhost:3000`:

```bash
bun run dev
```

Nuxt DevTools are off by default to keep local browser profiling representative.
Run `NUXT_DEVTOOLS_ENABLED=true bun run dev` if you need the DevTools overlay.

### Production

Build the application for production:

```bash
bun run build
```

Start the production server:

```bash
bun run .output/server/index.mjs
```

## Backend

We are using **Rust** for the frontend part which can be found in the rust/api-server folder.

The PostgreSQL migrations install the `pg_trgm` extension for search indexes.
The database user needs `CREATE` permission on the database, or an administrator
must install the extension beforehand with `CREATE EXTENSION IF NOT EXISTS pg_trgm`.

To get the default config you can run the command bellow.

```bash
cargo run -- print-config --format toml --show-default
```

With this you need to create the **config** folder and create the **config.toml** with the content from the command above.  
In there you need to change the following values:
```toml
[database]
url = "" 

[spacetimedb]
domain = "localhost"
protocol = "https://"
# Optional: add the region number to this base port. If omitted, no port is appended.
# base_port = 3000
databases = []
password = ""
websocket_protocol = "wss://"
cleanup = false
```

The optional base port can also be set with the environment variable
`BITCRAFT_HUB_API__SPACETIMEDB__BASE_PORT`.

### Development Server

Start the development server on `http://localhost:8080`:

```bash
cargo run -- serve
```

### Production

Build the application for production:

```bash
cargo build --release
```

#### Database worker batching

Live ingestion workers flush pending database writes after **2 seconds** from
the first row, or earlier when the worker's batch is full. Configure the
window in milliseconds with `BITCRAFT_HUB_API__DATABASE__WORKER_FLUSH_MS`.
Graceful shutdown drains accepted rows and flushes partial batches without
waiting for that timer. A crash can still lose queued rows. Craft and claim
event writers expose `worker_db_flush_rows` to measure the resulting SQL batch sizes.

Inventory **snapshot** upserts flush at the existing 3,000-row comparison
boundary or earlier when the next changed row would put the estimated bind
payload over **1 MiB**. Configure the byte budget with
`BITCRAFT_HUB_API__DATABASE__INVENTORY_SNAPSHOT_WRITE_BYTES` (positive integer).
A single oversized inventory is written alone and counted in
`inventory_snapshot_oversize_rows_total`. This is a per-statement budget, not
a limit on snapshot read buffers or live-update queues. Live inventory upserts
use the same `UNNEST` statement and byte budget. If a live flush needs multiple
statements, they commit together or all roll back.

#### Upstream WebSocket TLS

The SpacetimeDB SDK uses `tokio-websockets`. For `wss://`, the API defaults
to Rustls with ring and the platform certificate verifier over raw TCP.
Disable the default feature to use native TLS with a 16 KiB TCP read buffer.
Plain `ws://` connections use raw TCP in either build. From `rust/api-server`:

```bash
cargo build --release --locked
cargo build --release --locked --no-default-features
```

Run only one ingestion process against the benchmark database at a time.

Upstream SpacetimeDB message compression defaults to `gzip`. On a fast link,
set `BITCRAFT_HUB_API__SPACETIMEDB__COMPRESSION=none` to trade bandwidth for
lower decoder CPU (`gzip`, `none`, and `brotli` are accepted). This applies
to global and regional connections, including reconnects; TLS is independent.
The [region-14 decoder replay](rust/api-server/api/benches/sdk-decode/README.md)
measured 1.90× the application payload bytes with `none`.

For a BSATN replay fixture, create a private directory (`mkdir -m 700`) and
set `BITCRAFT_HUB_API__SPACETIMEDB__REGION_CAPTURE_DIR` to its absolute path.
Each regional connection writes a separate `region-<number>-*.capture` file
(512 MiB limit); the global connection is excluded. Unset the setting before
timing the live API, since recording adds decompression and disk writes.
Files contain decompressed BSATN and can include authentication tokens. Keep
them private. See `sdks/rust/BSATN_CAPTURE.md` in the SDK fork for the format.

### WebSocket traffic metrics

The client-facing `/websocket` negotiates `permessage-deflate` with clients
that offer it. Both JSON and MessagePack are supported, with no change to the
application message format. Clients without compression support can still
connect. Reverse proxies should use an HTTP/1.1 upgrade to the API for this
route.

The API's `/metrics` endpoint exposes `websocket_outbound_messages_total` and
`websocket_outbound_payload_bytes_total` for the client-facing `/websocket`.
Both are labeled by `message_type` (the protocol's `t` tag) and `encoding`
(`json` or `messagepack`). Metrics count each successful write once per
recipient, including direct replies. A series appears once a message of that
type and encoding has been written. Byte counts cover the serialized application
payload **before compression**, not WebSocket framing, control frames, TLS, or
TCP overhead. Use network counters to measure wire savings. The upstream
SpacetimeDB connection is not included.

In Prometheus, rank message types by payload bytes per second to find the
largest contributors:

```promql
topk(10, sum by (message_type) (rate(websocket_outbound_payload_bytes_total[5m])))
```

To tell large payloads apart from frequent messages, compare that with average
bytes per message by type and encoding:

```promql
sum by (message_type, encoding) (rate(websocket_outbound_payload_bytes_total[5m]))
/
sum by (message_type, encoding) (rate(websocket_outbound_messages_total[5m]))
```

### Live leaderboards

The leaderboard page loads the menu from `/leaderboard/boards` and requests
only the selected board from `/leaderboard/boards/{id}`. Its WebSocket
subscription is `leaderboard_delta.<id>`. `LeaderboardBoardDelta` carries only
changed rows, plus the new order if ranks or membership change. The client
checks the epoch and base revision before applying it. `SyncLeaderboardBoard`
requests a catch-up delta using a short, bounded history or a full
`LeaderboardBoardReset` snapshot when the base is missing or a snapshot is
smaller. The legacy `/leaderboard` endpoint and the `leaderboard` and
`leaderboard_board.<id>` WebSocket topics remain available for older clients.

### Live player presence

The claim, player, and account overview pages use `player_presence.<player-id>`
for live sign-in changes. `PlayerPresenceChanged` contains just the player ID
and online status. Playtime is read from the HTTP snapshot, and the legacy
`player_state` topic remains available for older clients.
