.PHONY: build run run-web test fmt lint check dev-frontend build-downloads deploy deploy-cli deploy-gateway deploy-web clean help

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

# ─── Build downloads ─────────────────────────────────

build-downloads:
	@echo "🔨 Building CLI binaries..."
	@mkdir -p downloads
	cargo build --release -p trending-agent-cli
	@cp target/release/openatrading downloads/openatrading-linux-amd64
	@echo "✅ downloads/openatrading-linux-amd64"
	@if command -v cargo-tauri &>/dev/null; then \
		echo "🔨 Building Tauri desktop..."; \
		cd src-tauri && cargo tauri build 2>&1 | tail -5; \
		echo "✅ Tauri build complete (check src-tauri/target/release/bundle/)"; \
	else \
		echo "⚠ cargo-tauri not installed, skip Tauri. Install: cargo install tauri-cli"; \
	fi

# ─── Deploy ──────────────────────────────────────────

deploy:
	@echo "🚀 Deploying all (gateway + web)..."
	@bash deploy.sh all

deploy-cli:
	@echo "🚀 Deploying CLI gateway..."
	@bash deploy.sh cli

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
	@echo "Build:"
	@echo "  build-downloads  Build CLI + Tauri → downloads/"
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
