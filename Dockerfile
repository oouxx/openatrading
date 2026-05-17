# syntax=docker/dockerfile:1.7
# ── Planner Stage ────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS planner

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml  crates/cli/
COPY src-tauri/Cargo.toml   src-tauri/

# 为所有 workspace member 创建 dummy src，让 cargo metadata 能解析
RUN mkdir -p crates/core/src crates/cli/src src-tauri/src && \
    echo '' > crates/core/src/lib.rs && \
    echo 'fn main() {}' > crates/cli/src/main.rs && \
    echo 'fn main() {}' > src-tauri/src/main.rs

RUN cargo chef prepare --recipe-path recipe.json


# ── Builder Stage ─────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        mold \
    && rm -rf /var/lib/apt/lists/*

ARG GIT_VERSION=v0.0.0-dev
ENV GIT_VERSION=${GIT_VERSION}

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