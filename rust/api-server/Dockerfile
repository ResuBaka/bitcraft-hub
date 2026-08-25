FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /usr/src/api-server

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /usr/src/api-server/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN apt update && apt install cmake binutils -y
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN --mount=type=cache,target=/usr/src/api-server/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release --bin bitcraft-hub-api && \
    objcopy --compress-debug-sections=zlib target/release/bitcraft-hub-api && \
    mv target/release/bitcraft-hub-api .


FROM debian:trixie-slim

WORKDIR /usr/src/api-server

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt update && apt install --no-install-recommends libssl3t64 ca-certificates -y

COPY --from=builder /usr/src/api-server/bitcraft-hub-api .

CMD ["./bitcraft-hub-api"]
