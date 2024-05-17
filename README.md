# Bitcraft Hub

> Not affiliated with Clockwork Labs
 
> This project is a work in progress and is not yet ready for production use. On that note we currently do not have an easy setup for contributing to this project. As you need to have all the game database state.

## Setup

We are using Bun for this project.

Make sure to install the dependencies:

```bash
bun install
# or
just install
```

## Development Server

Start the development server on `http://localhost:3000`:

```bash
bun run dev
# or
just dev
```

## Production

Build the application for production:

```bash
bun run build
# or
just build
```

Start the production server:

```bash
bun run .output/server/index.mjs
# or
just start
```
