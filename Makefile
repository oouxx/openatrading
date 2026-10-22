-include .env
export
.PHONY: build run run-web watch-web test fmt lint check run-frontend run-tauri deploy deploy-gateway deploy-web clean help release release-patch release-minor release-major release-set version build-tauri release-tauri-local release-brew

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

# Install to /usr/local/bin
install: release
	cp target/release/openatrading /usr/local/bin/openatrading
	@echo "✓ Installed to /usr/local/bin/openatrading"

# Uninstall
uninstall:
	rm -f /usr/local/bin/openatrading
	@echo "✓ Removed /usr/local/bin/openatrading"

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


# ─── Release Tauri (local build + gh upload) ─────────

# gh 优先读 GH_TOKEN/GITHUB_TOKEN 环境变量；Makefile 顶部 `-include .env` + `export`
# 会把 .env 的 GITHUB_TOKEN（fine-grained PAT，无 release 权限）注入环境导致 403。
# 本地发布用 keyring 凭据（gh auth login）；CI 场景用 RELEASE_GH_TOKEN 显式指定。
GH_AUTH := if [ -n "$${RELEASE_GH_TOKEN:-}" ]; then export GH_TOKEN="$$RELEASE_GH_TOKEN"; unset GITHUB_TOKEN; else unset GH_TOKEN GITHUB_TOKEN; fi

build-tauri:
	cd src-tauri && cargo tauri build

release-tauri-local: build-tauri
	@if ! which gh >/dev/null 2>&1; then echo "❌ 需要 gh CLI（brew install gh）"; exit 1; fi
	$(eval V := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'))
	$(eval TAG := v$(V))
	@echo "🚀 发布 OpenATrading $(V) → 公开 tap 仓库"
	@$(GH_AUTH); gh release create "$(TAG)" \
		--repo oouxx/homebrew-openatrading \
		--title "$(TAG)" \
		--notes "OpenATrading $(V)" \
		--notes-start-tag "" 2>/dev/null || echo "⚠ Release $(TAG) 已存在"
	@found=0; \
	for f in $$(find target/release/bundle -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' -o -name '*.msi' -o -name '*.exe' \) ! -name 'rw.*' 2>/dev/null); do \
		found=1; \
		echo "📤 上传 $$(basename "$$f")"; \
		$(GH_AUTH); gh release upload "$(TAG)" "$$f" --repo oouxx/homebrew-openatrading --clobber; \
	done; \
	if [ "$$found" -eq 0 ]; then \
		echo "❌ 未找到编译产物，请先运行 make build-tauri"; \
		exit 1; \
	fi
	@echo "✅ 已上传本机构建产物到 oouxx/homebrew-openatrading"

release-brew: build-tauri
	@bash release-brew.sh

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
	@echo "Release Tauri:"
	@echo "  release-tauri-local  本地编译 Tauri 并 gh 上传到 GitHub Release"
	@echo "  release-brew         本地编译 DMG + gh 上传 GitHub Release + 更新 brew cask"
	@echo ""
	@echo "Deploy:"
	@echo "  deploy         deploy all (gateway + web)"
	@echo "  deploy-gateway deploy gateway only"
	@echo "  deploy-web     deploy frontend only"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
	@echo "  install        安装到 /usr/local/bin"
	@echo "  uninstall      从 /usr/local/bin 卸载"
