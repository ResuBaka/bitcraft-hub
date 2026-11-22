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

#### Upstream WebSocket TLS

The native SpacetimeDB SDK uses `tokio-websockets`. For `wss://`, the default
uses native TLS with a 16 KiB TCP read buffer below TLS. The optional Rustls
connector uses ring and the platform certificate verifier over raw TCP.
Plain `ws://` connections use raw TCP in either build. From `rust/api-server`:

```bash
cargo build --release --locked
cargo build --release --locked --features spacetimedb-tokio-websockets-rustls
```
