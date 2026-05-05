.PHONY: build release run test test-all fmt lint clean dev-frontend dev-tauri docker-up docker-down deploy-static help

# ─── Build ───────────────────────────────────────────

build:
	cargo build

release:
	cargo build --release

# ─── Run ─────────────────────────────────────────────

run:
	cargo run --bin trending-agent

run-ai:
	cargo run --bin trending-agent -- run --ai

run-web:
	@pkill -f trending-agent-web 2>/dev/null; sleep 1; cargo run --bin trending-agent-web

run-all:
	cargo run --bin trending-agent -- run --ai --web --format both

# single data source
limit-up:
	cargo run --bin trending-agent -- limit-up

limit-down:
	cargo run --bin trending-agent -- limit-down

overview:
	cargo run --bin trending-agent -- overview

plate:
	cargo run --bin trending-agent -- plate-abnormal

field:
	cargo run --bin trending-agent -- field

timeline:
	cargo run --bin trending-agent -- timeline

community:
	cargo run --bin trending-agent -- community

agent:
	cargo run --bin trending-agent -- agent

# ─── AI Analysis ─────────────────────────────────────

sentiment:
	cargo run --bin trending-agent -- sentiment

technical:
	cargo run --bin trending-agent -- technical

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

# ─── JSON ────────────────────────────────────────────

json:
	cargo run --bin trending-agent -- run --format json --output ./src/public/
	@latest=$$(ls -t ./src/public/report_*.json 2>/dev/null | head -1); \
	if [ -n "$$latest" ]; then cp "$$latest" ./src/public/data/report.json && echo "✅ report.json ready"; fi

# ─── Static Deploy ───────────────────────────────────

# TAOGUBA_USERS: comma-separated user IDs, e.g. TAOGUBA_USERS=2577911,123456
TAOGUBA_USERS ?= 2577911

deploy-static:
	@echo "📡 生成静态数据 report.json..."
	@cargo run --bin trending-agent -- run --format json --output ./src/public/
	@latest=$$(ls -t ./src/public/report_*.json 2>/dev/null | head -1); \
	if [ -n "$$latest" ]; then cp "$$latest" ./src/public/data/report.json && echo "✅ report.json ready"; fi
	@echo "📦 构建 Vue 前端..."
	@cd src && npm run build
	@echo ""
	@echo "✅ 部署文件已生成到 src/dist/"
	@echo ""
	@echo "Vercel 配置说明："
	@echo "  默认: VITE_DATA_SOURCE=json (读取内置 report.json)"
	@echo "  动态: 在 Vercel 环境变量设置:"
	@echo "    VITE_DATA_SOURCE=api"
	@echo "    VITE_API_BASE_URL=https://你的后端地址"

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
	@echo "  deploy-static  generate report.json + build Vue → src/dist/"
	@echo "                 Vercel default: VITE_DATA_SOURCE=json"
	@echo "                 Set VITE_API_BASE_URL in Vercel dashboard for API mode"
	@echo "  docker-up      start via docker compose"
	@echo "  docker-down    stop docker compose"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
