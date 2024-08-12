FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /usr/src/api-server

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /usr/src/api-server/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin bitcraft-hub-api


FROM debian:bookworm-slim

WORKDIR /usr/src/api-server

COPY --from=builder /usr/src/api-server/target/release/bitcraft-hub-api .

RUN apt update && apt install libssl-dev ca-certificates -y && apt clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

CMD ["./bitcraft-hub-api"]
