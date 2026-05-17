.PHONY: build run run-web test fmt lint check dev-frontend build-downloads deploy deploy-cli deploy-gateway deploy-web clean help

# ─── Debug ────────────────────────────────────────────

build:
	cargo build

run:
	cargo run -p trending-agent-cli

run-web:
	@lsof -ti:8080 | xargs kill -9 2>/dev/null || true
	cargo run -p trending-agent-cli -- gateway run --web-only

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
	@mkdir -p downloads
	@echo ""
	@echo "🔨 Building CLI (all platforms)..."
	@# Linux x86_64
	rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
	cargo build --release -p trending-agent-cli --target x86_64-unknown-linux-gnu && \
		cp target/x86_64-unknown-linux-gnu/release/openatrading downloads/openatrading-x86_64-unknown-linux-gnu && \
		echo "✅ downloads/openatrading-x86_64-unknown-linux-gnu" || echo "⚠ CLI x86_64 skipped"
	@# Linux aarch64
	rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
	cargo build --release -p trending-agent-cli --target aarch64-unknown-linux-gnu && \
		cp target/aarch64-unknown-linux-gnu/release/openatrading downloads/openatrading-aarch64-unknown-linux-gnu && \
		echo "✅ downloads/openatrading-aarch64-unknown-linux-gnu" || echo "⚠ CLI aarch64 skipped (need gcc-aarch64-linux-gnu)"
	@echo ""
	@echo "🔨 Building Tauri (macOS only, other platforms use CI)..."
	@if command -v cargo-tauri &>/dev/null; then \
		cargo tauri build --target x86_64-apple-darwin 2>&1 | tail -1 || true; \
		cargo tauri build --target aarch64-apple-darwin 2>&1 | tail -1 || true; \
	else \
		echo "⚠ cargo-tauri not installed, skip. Install: cargo install tauri-cli"; \
	fi
	@echo ""
	@ls -lh downloads/

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
	@echo "  deploy-web     deploy frontend only"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
