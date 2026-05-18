.PHONY: build run run-web test fmt lint check dev-frontend build-downloads deploy deploy-gateway deploy-web clean help

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

# Clean build artifacts
clean:
	cargo clean

# Install to /usr/local/bin
install: release
	cp target/release/openatrading /usr/local/bin/openatrading
	@echo "✓ Installed to /usr/local/bin/openatrading"

# Uninstall
uninstall:
	rm -f /usr/local/bin/openatrading
	@echo "✓ Removed /usr/local/bin/openatrading"


# ── Cross-compilation targets ──

release-mac-arm:
	cargo build --release --target aarch64-apple-darwin

release-mac-intel:
	cargo build --release --target x86_64-apple-darwin

release-linux:
	cross build --release --target x86_64-unknown-linux-musl

release-linux-arm:
	cross build --release --target aarch64-unknown-linux-musl

release-windows:
	cargo build --release --target x86_64-pc-windows-msvc

# Build all platforms (requires cross for ARM Linux)
release-all: release-mac-arm release-mac-intel release-linux release-linux-arm release-windows


# ── Packaging ──

TARGETS = aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-pc-windows-msvc

package:
	@mkdir -p dist
	@for target in $(TARGETS); do \
		if [ -f "target/$$target/release/openatrading" ]; then \
			echo "Packaging $$target..."; \
			tar czf "dist/openatrading-$$target.tar.gz" -C "target/$$target/release" openatrading; \
		elif [ -f "target/$$target/release/openatrading.exe" ]; then \
			echo "Packaging $$target..."; \
			cd "target/$$target/release" && zip "../../../dist/openatrading-$$target.zip" openatrading.exe && cd ../../..; \
		fi \
	done
	@cd dist && sha256sum openatrading-* > SHA256SUMS.txt 2>/dev/null || shasum -a 256 openatrading-* > SHA256SUMS.txt
	@echo "✓ Packages in dist/"
	@ls -lh dist/


# ─── Deploy ──────────────────────────────────────────

deploy:
	@echo "🚀 Deploying all (gateway + web)..."
	@bash deploy.sh all

deploy-gateway:
	@echo "🚀 Deploying gateway..."
	@bash deploy.sh gateway

deploy-web:
	@echo "🚀 Deploying web (frontend only)..."
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
