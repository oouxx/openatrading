-include .env
export
.PHONY: build run run-web watch-web test fmt lint check run-frontend run-tauri deploy deploy-gateway deploy-web clean help release release-patch release-minor release-major release-set version build-tauri

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
	@echo $(PI_RS_HOME)
	cargo run -p openatrading-cli -- gateway run

run-frontend:
	cd src && bun run dev

run-tauri:
	cd src-tauri && cargo tauri dev


# ─── Quality ──────────────────────────────────────────

test:
	cargo test -p openatrading-core -p openatrading-api

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

check: fmt lint test
	@echo "✅ All checks passed"

# Clean build artifacts
clean:
	cargo clean

# ─── 本机安装（装当前工作区的桌面端，不走发版流程）────────────────
#
# 只装桌面应用：`cargo tauri build` 出的 .app → /Applications，直接覆盖。
# 以前这里是 `install: release` + `cp` 一个 CLI 二进制，两个问题：`release` 依赖会
# 先发一次版（tag 已存在就直接失败），/usr/local/bin 是 root 的、`cp` 写不进去。
APP_PATH ?= /Applications/OpenATrading.app
# tauri 的 productName 是小写的 openatrading，产物在这里；装到上面的显示名路径
APP_BUNDLE := target/release/bundle/macos/openatrading.app
SUDO := $(shell [ -w "$(dir $(APP_PATH))" ] || echo sudo)
SUDO_BIN := $(shell [ -w /usr/local/bin ] || echo sudo)
# 本地只出 .app：不打 DMG，也不生成更新包（更新包需要 TAURI_SIGNING_PRIVATE_KEY，
# 那是发版流程的事，本地没有那把钥匙）
TAURI_BUILD := cd src-tauri && cargo tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'

install:
	$(TAURI_BUILD)
	@if pgrep -f "$(APP_PATH)/Contents/MacOS" >/dev/null; then \
		echo "→ 正在运行的 $(notdir $(APP_PATH)) 先退出"; \
		osascript -e 'quit app "$(notdir $(basename $(APP_PATH)))"' >/dev/null 2>&1 || true; \
		sleep 1; \
	fi
	$(SUDO) rm -rf "$(APP_PATH)"
	$(SUDO) ditto "$(APP_BUNDLE)" "$(APP_PATH)"
	@/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$(APP_PATH)/Contents/Info.plist" | sed 's/^/  /'
	@echo "✓ 已安装到 $(APP_PATH)"

# 卸掉桌面端和（如果还在的）CLI 二进制
uninstall:
	$(SUDO) rm -rf "$(APP_PATH)"
	$(SUDO_BIN) rm -f /usr/local/bin/openatrading
	@echo "✓ 已移除 $(APP_PATH) 与 /usr/local/bin/openatrading"

# ─── Deploy ──────────────────────────────────────────

# 部署已改为 GitHub Actions：build 在托管 runner 出镜像，self-hosted runner 只 pull+up。
# 这些 target 只是触发 workflow（需要 gh CLI 且已 gh auth login）。
deploy:
	@echo "🚀 Triggering deploy: all (gateway + frontend + akshare-mcp)..."
	@gh workflow run deploy.yml -f service=all
	@gh run list -w Deploy -L 1

deploy-gateway:
	@echo "🚀 Triggering deploy: gateway..."
	@gh workflow run deploy.yml -f service=gateway
	@gh run list -w Deploy -L 1

deploy-web:
	@echo "🚀 Triggering deploy: frontend..."
	@gh workflow run deploy.yml -f service=web
	@gh run list -w Deploy -L 1

# ─── Release ──────────────────────────────────────────

version:
	@echo "Current: $$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *\"\(.*\)\"/\1/')"

release-patch: check _bump-patch _release
release-minor: check _bump-minor _release
release-major: check _bump-major _release
release-set: check _bump-set _release
release: check _release

_bump-patch:
	@v=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$${major}.$${minor}.$$((patch + 1))"; \
	echo "⬆  $$v → $$new (patch)"; \
	$(call _set_version,$$new)

_bump-minor:
	@v=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$${major}.$$((minor + 1)).0"; \
	echo "⬆  $$v → $$new (minor)"; \
	$(call _set_version,$$new)

_bump-major:
	@v=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	IFS='.' read -r major minor patch <<< "$$v"; \
	new="$$((major + 1)).0.0"; \
	echo "⬆  $$v → $$new (major)"; \
	$(call _set_version,$$new)

_bump-set:
	@if [ -z "$(VERSION)" ]; then echo "❌ 用法: make release-set VERSION=0.6.0"; exit 1; fi
	@v=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	echo "⬆  $$v → $(VERSION) (set)"; \
	$(call _set_version,$(VERSION))

define _set_version
	new_ver="$(1)"; \
	sed -i '' '/^\[workspace\.package\]/,/^\[/{ s/^version = "[0-9][^"]*"/version = "'"$$new_ver"'"/; }' Cargo.toml; \
	echo "✓ version $$new_ver"
endef

_release:
	@v=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/'); \
	tag="v$$v"; \
	git tag -l "$$tag" | grep -q "$$tag" && { echo "❌ Tag $$tag 已存在"; exit 1; }; \
	echo "🚀 发版 $$tag"; \
	cargo generate-lockfile && \
	git add -A && git commit -m "release $$tag"; \
	git tag "$$tag"; \
	BRANCH=$$(git branch --show-current); \
	git push origin "$$BRANCH" --tags || { echo "❌ 推送失败：$$tag 已本地提交并打标，但未到远端"; exit 1; }; \
	echo "✅ 已推送 $$tag 到 $$BRANCH，等待 GitHub Actions 构建"


# ─── Tauri ───────────────────────────────────────────

build-tauri:
	cd src-tauri && cargo tauri build

# ─── Help ────────────────────────────────────────────

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Debug:"
	@echo "  build          dev build"
	@echo "  run            CLI run"
	@echo "  doctor         CLI doctor check"
	@echo "  run-web        Web server (gateway)"
	@echo "  watch-web      Web server with auto-reload"
	@echo "  run-frontend   Vue dev server"
	@echo "  run-tauri      Tauri desktop dev"
	@echo "  build-tauri    Tauri desktop bundle"
	@echo ""
	@echo "Quality:"
	@echo "  test           cargo test (all crates)"
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
	@echo "  install        构建桌面端并装到 /Applications/OpenATrading.app"
	@echo "  uninstall      卸载桌面端（以及遗留的 /usr/local/bin CLI）"
