FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Dedicated stage: compile dioxus-cli from source so it matches the builder's glibc.
# This layer is cached until the version changes.
FROM chef AS dx-builder
RUN cargo install dioxus-cli --version "0.7.7" --locked

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY --from=dx-builder /usr/local/cargo/bin/dx /usr/local/cargo/bin/dx

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

ENV SQLX_OFFLINE=true

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    dx build --release --package jogga --features=wasm-split --wasm-split && \
    cp -r /app/target/dx/jogga/release/web /app/web

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/web/ /usr/local/app

ENV JOGGA__SERVER__HOST=0.0.0.0
ENV JOGGA__SERVER__PORT=80

EXPOSE 80

WORKDIR /usr/local/app
ENTRYPOINT ["/usr/local/app/server"]
