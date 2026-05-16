.PHONY: build release run run-json run-html run-ai run-web limit-up super-stock overview field timeline sentiment cls plate test test-all fmt lint check clean dev-frontend json deploy-static docker-build docker-up docker-down docker-logs docker-restart docker-rebuild docker-shell deploy deploy-gateway deploy-web help

# ─── Build ───────────────────────────────────────────

build:
	cargo build

release:
	cargo build --release

# ─── Run ─────────────────────────────────────────────

run:
	cargo run --bin trending-agent

run-json:
	cargo run --bin trending-agent -- --json

run-html:
	cargo run --bin trending-agent -- --html --output report.html

run-ai:
	cargo run --bin trending-agent -- --ai

run-web:
	@pkill -f trending-agent-web 2>/dev/null; sleep 1; cargo run --bin trending-agent-web

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
	cd src && bun run dev

# ─── Tauri ───────────────────────────────────────────

dev-tauri:
	cargo tauri dev

# ─── JSON ────────────────────────────────────────────

json:
	cargo run --bin trending-agent -- --json --output ./src/public/data/report.json
	@echo "✅ report.json ready"

# ─── Deploy ──────────────────────────────────────────

deploy:
	@echo "🚀 Deploying all (gateway + web)..."
	@bash deploy.sh all

deploy-gateway:
	@echo "🚀 Deploying gateway..."
	@bash deploy.sh gateway

deploy-web:
	@echo "🚀 Deploying web (backend + frontend)..."
	@bash deploy.sh web

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
	@echo "  run            full data → Markdown stdout"
	@echo "  run-json       full data → JSON stdout"
	@echo "  run-html       full data → HTML file"
	@echo "  run-ai         full data + AI analysis"
	@echo "  run-web        Web API server (:8080)"
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
	@echo "  dev-frontend   Vue 3 dev server (bun)"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy         deploy all (gateway + web)"
	@echo "  deploy-gateway deploy openatrading gateway"
	@echo "  deploy-web     deploy web backend + frontend"
	@echo "  json           generate report.json for static frontend"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
