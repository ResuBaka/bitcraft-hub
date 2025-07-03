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
databases = []
password = ""
websocket_protocol = "wss://"
cleanup = false
```

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
