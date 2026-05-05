.PHONY: build release run test test-all fmt lint clean dev-frontend dev-tauri docker-up docker-down deploy-static help

# ─── Build ───────────────────────────────────────────

build:
	cargo build

release:
	cargo build --release

# ─── Run ─────────────────────────────────────────────

run:
	cargo run

run-ai:
	cargo run -- run --ai

run-web:
	cargo run -- run --web

run-all:
	cargo run -- run --ai --web --format both

# single data source
limit-up:
	cargo run -- limit-up

limit-down:
	cargo run -- limit-down

overview:
	cargo run -- overview

plate:
	cargo run -- plate-abnormal

field:
	cargo run -- field

timeline:
	cargo run -- timeline

community:
	cargo run -- community

agent:
	cargo run -- agent

# ─── AI Analysis ─────────────────────────────────────

sentiment:
	cargo run -- sentiment

technical:
	cargo run -- technical

# ─── Test ────────────────────────────────────────────

test:
	cargo test

test-all:
	cargo test -- --ignored

# ─── Lint & Format ───────────────────────────────────

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt lint test
	@echo "✅ All checks passed"

# ─── Frontend ────────────────────────────────────────

dev-frontend:
	cd src && npm run dev

# ─── Tauri ───────────────────────────────────────────

dev-tauri:
	cargo tauri dev

# ─── Static Deploy ───────────────────────────────────

deploy-static:
	cargo run -- run --format html --output ./dist/reports/
	cargo run -- index --dir ./dist/

# ─── Docker ──────────────────────────────────────────

docker-up:
	docker compose up -d

docker-down:
	docker compose down

# ─── Clean ───────────────────────────────────────────

clean:
	cargo clean

# ─── Help ────────────────────────────────────────────

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Build:"
	@echo "  build          dev build"
	@echo "  release        release build"
	@echo ""
	@echo "Run:"
	@echo "  run            default run (full report)"
	@echo "  run-ai         full report + AI analysis"
	@echo "  run-web        full report + Web UI"
	@echo "  run-all        full report + AI + Web + both formats"
	@echo ""
	@echo "Single source:"
	@echo "  limit-up       fetch limit-up data"
	@echo "  limit-down     fetch limit-down data"
	@echo "  overview       market overview"
	@echo "  plate          plate abnormal"
	@echo "  field          Jiuyan field data"
	@echo "  timeline       Jiuyan timeline"
	@echo "  community      community articles"
	@echo "  agent          interactive REPL"
	@echo ""
	@echo "AI:"
	@echo "  sentiment      sentiment analysis"
	@echo "  technical      technical analysis"
	@echo ""
	@echo "Test:"
	@echo "  test           unit tests"
	@echo "  test-all       all tests (including integration)"
	@echo ""
	@echo "Lint:"
	@echo "  fmt            cargo fmt"
	@echo "  lint           cargo clippy"
	@echo "  check          fmt + lint + test"
	@echo ""
	@echo "Dev:"
	@echo "  dev-frontend   Vue 3 dev server"
	@echo "  dev-tauri      Tauri desktop app"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy-static  generate static HTML to dist/"
	@echo "  docker-up      start via docker compose"
	@echo "  docker-down    stop docker compose"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
