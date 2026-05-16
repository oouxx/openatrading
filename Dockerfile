# ── Build Stage ───────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder

ARG GIT_VERSION=v0.0.0-dev
ENV GIT_VERSION=${GIT_VERSION}

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/cli/Cargo.toml crates/cli/
COPY crates/web/Cargo.toml crates/web/
COPY src-tauri/Cargo.toml src-tauri/

# Pre-build deps layer
RUN mkdir -p crates/core/src crates/cli/src crates/web/src src-tauri/src && \
    echo '' > crates/core/src/lib.rs && \
    echo 'fn main() {}' > crates/cli/src/main.rs && \
    echo 'fn main() {}' > crates/web/src/main.rs && \
    echo 'fn main() {}' > src-tauri/src/main.rs && \
    cargo build --release -p trending-agent-cli && \
    rm -rf crates/cli/src crates/core/src crates/web/src src-tauri/src

# Copy real source
COPY crates/ crates/
COPY src-tauri/ src-tauri/

RUN cargo build --release -p trending-agent-cli && \
    cp target/release/openatrading /usr/local/bin/openatrading

# ── Runtime Stage ─────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/openatrading /usr/local/bin/openatrading

# Mount points
VOLUME /root/.atrading
VOLUME /app

WORKDIR /app

ENTRYPOINT ["openatrading"]
CMD ["gateway", "run"]
