# ── Planner Stage ────────────────────────────────────
FROM rust:1.88-slim-bookworm AS planner

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src-tauri/ src-tauri/

RUN cargo chef prepare --recipe-path recipe.json

# ── Build Stage ──────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder

ARG GIT_VERSION=v0.0.0-dev
ENV GIT_VERSION=${GIT_VERSION}

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

WORKDIR /app

# Pre-build deps from recipe (cached unless Cargo.toml/Cargo.lock change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p trending-agent-cli

# Copy real source and build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src-tauri/ src-tauri/

RUN cargo build --release -p trending-agent-cli && \
    cp target/release/openatrading /usr/local/bin/openatrading

# ── Runtime Stage ────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/openatrading /usr/local/bin/openatrading

VOLUME /root/.atrading
VOLUME /app

WORKDIR /app

ENTRYPOINT ["openatrading"]
CMD ["gateway", "run"]
