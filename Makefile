include .env
export
.PHONY: build run run-web watch-web test test-tui fmt lint check run-frontend run-tauri build-tauri build-downloads deploy deploy-gateway deploy-web clean help release release-patch release-minor release-major release-set version chat decision run-mcp-http run-mcp-stdio release-tauri-local

# ─── Debug ────────────────────────────────────────────

build:
	cargo build

run:
	cargo run -p openatrading-cli
doctor:
	cargo run -p openatrading-cli -- doctor
watch-web:
	@lsof -ti:8080 | xargs kill -9 2>/dev/null || true
	cargo watch -x "run -p openatrading-cli -- gateway run"
run-web:
	@lsof -ti:8080 | xargs kill -9 2>/dev/null || true
	cargo run -p openatrading-cli -- gateway run
run-frontend:
	cd src && bun run dev

chat:
	cargo run -p openatrading-cli -- chat

decision:
	cargo run -p openatrading-cli -- decision

decision-dry-run:
	cargo run -p openatrading-cli -- decision --dry-run

run-tauri:
	cd src-tauri && cargo tauri dev

build-tauri:
	cd src-tauri && cargo tauri build

# ─── MCP Server ──────────────────────────────────────

run-mcp-http:
	@lsof -ti:8082 | xargs kill -9 2>/dev/null || true
	cargo run -p openatrading-cli -- mcp serve

run-mcp-stdio:
	cargo run -p openatrading-cli -- mcp serve --transport stdio

# ─── Quality ──────────────────────────────────────────

test:
	cargo test -p openatrading-api -p openatrading-core -p openatrading-cli -p openatrading-channels -p openatrading-provider -p openatrading-web -p openatrading-tui

test-tui:
	cargo test -p openatrading-tui

fmt:
	cargo fmt --all

lint:
	cargo clippy -p openatrading-api -p openatrading-core -p openatrading-cli -p openatrading-channels -p openatrading-provider -p openatrading-web -p openatrading-tui -- -D warnings

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

VERSION_FILES := crates/core/Cargo.toml crates/cli/Cargo.toml crates/api/Cargo.toml crates/channels/Cargo.toml crates/provider/Cargo.toml crates/web/Cargo.toml crates/agent/Cargo.toml crates/tui/Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json

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
	cargo generate-lockfile && \
	git add -A && git commit -m "release $$tag"; \
	git tag "$$tag"; \
	git push origin main --tags; \
	echo "✅ 已推送 $$tag，等待 GitHub Actions 构建"

# ─── Release Tauri (local build + gh upload) ─────────

release-tauri-local: build-tauri
	@if ! which gh >/dev/null 2>&1; then echo "❌ 需要 gh CLI（brew install gh）"; exit 1; fi
	$(eval V := $(shell grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'))
	$(eval TAG := v$(V))
	@echo "🚀 发布 OpenATrading $(V) → 公开 tap 仓库"
	@gh release create "$(TAG)" \
		--repo oouxx/homebrew-openatrading \
		--title "$(TAG)" \
		--notes "OpenATrading $(V)" \
		--verify-tag 2>/dev/null || echo "⚠ Release $(TAG) 已存在"
	@for f in src-tauri/target/release/bundle/dmg/*.dmg \
	          src-tauri/target/release/bundle/deb/*.deb \
	          src-tauri/target/release/bundle/appimage/*.AppImage \
	          src-tauri/target/release/bundle/msi/*.msi \
	          src-tauri/target/release/bundle/nsis/*.exe; do \
		[ -f "$$f" ] || continue; \
		gh release upload "$(TAG)" "$$f" --repo oouxx/homebrew-openatrading --clobber; \
	done
	@echo "✅ 已上传本机构建产物到 oouxx/homebrew-openatrading"

# ─── Help ────────────────────────────────────────────

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Debug:"
	@echo "  build          dev build"
	@echo "  run            CLI test run"
	@echo "  chat           CLI AI chat TUI"
	@echo "  decision       手动触发一次所有角色的每日决策"
	@echo "  decision-dry-run 决策 dry-run 模式（仅输出到终端）"
	@echo "  run-web        Web server"
	@echo "  run-mcp-http   MCP server (HTTP, :8082)"
	@echo "  run-mcp-stdio  MCP server (stdio)"
	@echo "  run-frontend   Vue dev server"
	@echo "  dev-tauri      Tauri desktop dev"
	@echo "  build-tauri    Tauri desktop build"
	@echo ""
	@echo "Quality:"
	@echo "  test           cargo test (all crates)"
	@echo "  test-tui       cargo test (tui only)"
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
	@echo "Release Tauri:"
	@echo "  release-tauri-local  本地编译 Tauri 并 gh 上传到 GitHub Release"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy         deploy all (gateway + web)"
	@echo "  deploy-gateway deploy gateway only"
	@echo "  deploy-web     deploy frontend only"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
