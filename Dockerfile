# ── Planner Stage ────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS planner

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml crates/cli/
COPY src-tauri/Cargo.toml src-tauri/

# Create dummy source files (cargo metadata requires them)
RUN mkdir -p crates/core/src crates/cli/src src-tauri/src && \
    echo '' > crates/core/src/lib.rs && \
    echo 'fn main() {}' > crates/cli/src/main.rs && \
    echo 'fn main() {}' > src-tauri/src/main.rs

RUN cargo chef prepare --recipe-path recipe.json

# ── Build Stage ──────────────────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim-bookworm AS builder

ARG GIT_VERSION=v0.0.0-dev
ENV GIT_VERSION=${GIT_VERSION}

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies from recipe (cached unless Cargo.toml/Cargo.lock change)
COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml crates/cli/
COPY src-tauri/Cargo.toml src-tauri/
RUN mkdir -p crates/core/src crates/cli/src src-tauri/src && \
    echo '' > crates/core/src/lib.rs && \
    echo 'fn main() {}' > crates/cli/src/main.rs && \
    echo 'fn main() {}' > src-tauri/src/main.rs
RUN cargo chef cook --release --recipe-path recipe.json -p trending-agent-cli

# Copy real source and build final binary
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
