FROM rust:1.85-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .env .

RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

RUN rm -rf src

COPY . .
RUN touch -r /app/target/release/trending-agent /app/src/main.rs
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/trending-agent .
COPY --from=builder /app/.env .

ENV RUST_BACKTRACE=1

CMD ["./trending-agent"]