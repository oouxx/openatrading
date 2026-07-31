# syntax=docker/dockerfile:1.7
# ── Planner Stage ────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS planner

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/api/Cargo.toml crates/api/
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml  crates/cli/
COPY crates/channels/Cargo.toml crates/channels/
COPY crates/web/Cargo.toml crates/web/
COPY crates/agent/Cargo.toml crates/agent/
COPY crates/extension/Cargo.toml crates/extension/
COPY src-tauri/Cargo.toml src-tauri/

# 为所有 workspace member 创建 dummy src，让 cargo metadata 能解析
RUN mkdir -p crates/api/src crates/core/src crates/cli/src crates/channels/src crates/web/src crates/agent/src crates/extension/src src-tauri/src && \
    echo '' > crates/api/src/lib.rs && \
    echo '' > crates/core/src/lib.rs && \
    echo 'fn main() {}' > crates/cli/src/main.rs && \
    echo '' > crates/channels/src/lib.rs && \
    echo '' > crates/web/src/lib.rs && \
    echo '' > crates/agent/src/lib.rs && \
    echo '' > crates/extension/src/lib.rs && \
    echo 'fn main() {}' > src-tauri/src/main.rs

RUN cargo chef prepare --recipe-path recipe.json


# ── Builder Stage ─────────────────────────────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        mold \
    && rm -rf /var/lib/apt/lists/*

# Default is unset — build.rs will read the version from Cargo.toml.
# Override with: docker build --build-arg GIT_VERSION=v0.x.x ...
ARG GIT_VERSION

ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"

WORKDIR /app

# ── 依赖编译层（Cargo.lock 不变就命中 Docker layer cache）─────────────────────
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p openatrading-cli

# ── 业务代码编译层 ─────────────────────────────────────────────────────────────
COPY Cargo.toml Cargo.lock ./
# config.example.toml is include_str!'d at compile time by cmd_init / ConfigCheck,
# so it must exist in the build context alongside the source.
COPY config.example.toml ./
COPY crates/ crates/
# src-tauri is a workspace member; its manifest must be present for workspace
# resolution, but we never build it here (only openatrading-cli).
COPY src-tauri/Cargo.toml src-tauri/

RUN cargo build --release -p openatrading-cli && \
    cp target/release/openatrading /usr/local/bin/openatrading


# ── Runtime Stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-20240612-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Asia/Shanghai
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

COPY --from=builder /usr/local/bin/openatrading /usr/local/bin/openatrading

VOLUME /root/.atrading
VOLUME /app

WORKDIR /app

ENTRYPOINT ["openatrading"]
CMD ["gateway", "run"]
