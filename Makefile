.PHONY: build run run-web test fmt lint check dev-frontend dev-tauri build-tauri build-downloads deploy deploy-gateway deploy-web clean help release release-patch release-minor release-major release-set version

# ─── Debug ────────────────────────────────────────────

build:
	cargo build

run:
	cargo run -p trading-agent-cli

run-web:
	@lsof -ti:8080 | xargs kill -9 2>/dev/null || true
	cargo run -p trading-agent-cli -- gateway run --web-only

run-frontend:
	cd src && bun run dev

dev-tauri:
	cd src-tauri && cargo tauri dev

build-tauri:
	cd src-tauri && cargo tauri build

# ─── Quality ──────────────────────────────────────────

test:
	cargo test -p trading-agent-api -p trading-agent-core -p trading-agent-cli -p trading-agent-channels -p trading-agent-provider -p trading-agent-web

fmt:
	cargo fmt --all

lint:
	cargo clippy -p trading-agent-api -p trading-agent-core -p trading-agent-cli -p trading-agent-channels -p trading-agent-provider -p trading-agent-web -- -D warnings

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

# ─── Release ──────────────────────────────────────────

VERSION_FILES := crates/core/Cargo.toml crates/cli/Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json

version:
	@echo "Current: $$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *\"\(.*\)\"/\1/')"

release-patch: check _bump-patch _release
release-minor: check _bump-minor _release
release-major: check _bump-major _release
release-set: check _bump-set _release
release: check _release

_bump-patch:
	@v=$$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$${major}.$${minor}.$$((patch + 1))"; \
	echo "⬆  $$v → $$new (patch)"; \
	$(call _set_version,$$new)

_bump-minor:
	@v=$$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$${major}.$$((minor + 1)).0"; \
	echo "⬆  $$v → $$new (minor)"; \
	$(call _set_version,$$new)

_bump-major:
	@v=$$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$$((major + 1)).0.0"; \
	echo "⬆  $$v → $$new (major)"; \
	$(call _set_version,$$new)

_bump-set:
	@if [ -z "$(VERSION)" ]; then echo "❌ 用法: make release-set VERSION=0.6.0"; exit 1; fi
	@v=$$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	echo "⬆  $$v → $(VERSION) (set)"; \
	$(call _set_version,$(VERSION))

define _set_version
	for f in $(VERSION_FILES); do \
		if [ "$$f" = "src-tauri/tauri.conf.json" ]; then \
			sed -i '' "s/\"version\": \"[0-9][^\"]*\"/\"version\": \"$(1)\"/" $$f; \
		else \
			sed -i '' "s/^version = \"[0-9][^\"]*\"/version = \"$(1)\"/" $$f; \
		fi; \
	done
endef

_release:
	@v=$$(grep '^version' crates/cli/Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	tag="v$$v"; \
	git tag -l "$$tag" | grep -q "$$tag" && { echo "❌ Tag $$tag 已存在"; exit 1; }; \
	echo "🚀 发版 $$tag"; \
	git add -A && git commit -m "release $$tag"; \
	git tag "$$tag"; \
	git push origin main --tags; \
	echo "✅ 已推送 $$tag，等待 GitHub Actions 构建"

# ─── Help ────────────────────────────────────────────

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Debug:"
	@echo "  build          dev build"
	@echo "  run            CLI test run"
	@echo "  run-web        Web server"
	@echo "  run-frontend   Vue dev server"
	@echo "  dev-tauri      Tauri desktop dev"
	@echo "  build-tauri    Tauri desktop build"
	@echo ""
	@echo "Quality:"
	@echo "  test           cargo test"
	@echo "  fmt            cargo fmt"
	@echo "  lint           cargo clippy"
	@echo "  check          fmt + lint + test"
	@echo ""
	@echo "Release:"
	@echo "  version        显示当前版本"
	@echo "  release-patch  发版 patch (0.1.0 -> 0.1.1)"
	@echo "  release-minor  发版 minor (0.1.0 -> 0.2.0)"
	@echo "  release-major  发版 major (0.1.0 -> 1.0.0)"
	@echo "  release-set    指定版本发版 (make release-set VERSION=0.6.0)"
	@echo "  release        用当前版本号发版 (不 bump)"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy         deploy all (gateway + web)"
	@echo "  deploy-gateway deploy gateway only"
	@echo "  deploy-web     deploy frontend only"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"