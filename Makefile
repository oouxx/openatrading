.PHONY: build run run-web test fmt lint check dev-frontend deploy deploy-gateway deploy-web clean help

# ─── Debug ────────────────────────────────────────────

build:
	cargo build

run:
	cargo run -p trending-agent-cli

run-web:
	cargo run -p trending-agent-web

dev-frontend:
	cd src && bun run dev

# ─── Quality ──────────────────────────────────────────

test:
	cargo test

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets -- -D warnings

check: fmt lint test
	@echo "✅ All checks passed"

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
	@echo "Debug:"
	@echo "  build          dev build"
	@echo "  run            CLI test run"
	@echo "  run-web        Web server"
	@echo "  dev-frontend   Vue dev server"
	@echo ""
	@echo "Quality:"
	@echo "  test           cargo test"
	@echo "  fmt            cargo fmt"
	@echo "  lint           cargo clippy"
	@echo "  check          fmt + lint + test"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy         deploy all (gateway + web)"
	@echo "  deploy-gateway deploy gateway only"
	@echo "  deploy-web     deploy web + frontend"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
