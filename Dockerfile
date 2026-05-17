# syntax=docker/dockerfile:1.7
# ── Planner Stage ────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS planner

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml  crates/cli/

RUN cargo chef prepare --recipe-path recipe.json


# ── Builder Stage ─────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS builder

# mold 在 bookworm 官方源里有，直接装
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        mold \
    && rm -rf /var/lib/apt/lists/*

ARG GIT_VERSION=v0.0.0-dev
ENV GIT_VERSION=${GIT_VERSION}

# 全局启用 mold linker
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"

WORKDIR /app

# ── 依赖编译层（Cargo.lock 不变就命中 Docker layer cache）─────────────────────
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p trending-agent-cli

# ── 业务代码编译层 ─────────────────────────────────────────────────────────────
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release -p trending-agent-cli && \
    cp target/release/openatrading /usr/local/bin/openatrading


# ── Runtime Stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-20240612-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/openatrading /usr/local/bin/openatrading

VOLUME /root/.atrading
VOLUME /app

WORKDIR /app

ENTRYPOINT ["openatrading"]
CMD ["gateway", "run"]