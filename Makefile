.PHONY: build release run run-json run-html run-ai run-web limit-up super-stock overview field timeline sentiment cls plate test test-all fmt lint check clean dev-frontend json deploy-static docker-build docker-up docker-down docker-logs docker-restart docker-rebuild docker-shell deploy-racknerd deploy-racknerd-backend deploy-racknerd-frontend help

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

# single data source
limit-up:
	cargo run --bin trending-agent -- --data limit_up

super-stock:
	cargo run --bin trending-agent -- --data super_stock

overview:
	cargo run --bin trending-agent -- --data overview

field:
	cargo run --bin trending-agent -- --data field_item

timeline:
	cargo run --bin trending-agent -- --data timeline

sentiment:
	cargo run --bin trending-agent -- --data sentiment

cls:
	cargo run --bin trending-agent -- --data cls_telegraph

plate:
	cargo run --bin trending-agent -- --data plate_abnormal

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

# ─── Static Deploy ───────────────────────────────────

# TAOGUBA_USERS: comma-separated user IDs, e.g. TAOGUBA_USERS=2577911,123456
TAOGUBA_USERS ?= 2577911

deploy-static:
	@echo "📦 构建 Vue 前端..."
	@cd src && bun run build
	@echo ""
	@echo "✅ 部署文件已生成到 src/dist/"
	@echo ""
	@echo "Vercel 配置说明："
	@echo "  动态: 在 Vercel 环境变量设置:"
	@echo "    VITE_API_BASE_URL=https://你的后端地址"

# ─── Docker ──────────────────────────────────────────

docker-build:
	docker compose build

docker-up:
	docker compose up -d

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f

docker-restart:
	docker compose restart

docker-rebuild:
	docker compose build --no-cache
	docker compose up -d

docker-shell:
	docker compose exec trending-agent bash

# ─── Deploy (RackNerd) ──────────────────────────────

define deploy-racknerd-help
	cargo run --bin trending-agent -- --json --output ./src/public/data/report.json && echo "  ✅ report.json ready"
endef
deploy-racknerd:
	@echo "🚀 Deploying all (backend + frontend) to RackNerd..."
	@bash deploy-racknerd.sh all

deploy-racknerd-backend:
	@echo "🚀 Deploying backend to RackNerd..."
	@bash deploy-racknerd.sh backend

deploy-racknerd-frontend:
	@echo "🚀 Deploying frontend to RackNerd..."
	@bash deploy-racknerd.sh frontend

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
	@echo "Data source:"
	@echo "  limit-up       --data limit_up"
	@echo "  super-stock    --data super_stock"
	@echo "  overview       --data overview"
	@echo "  field          --data field_item"
	@echo "  timeline       --data timeline"
	@echo "  sentiment      --data sentiment"
	@echo "  cls            --data cls_telegraph"
	@echo "  plate          --data plate_abnormal"
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
	@echo "  json           generate report.json for static frontend"
	@echo "  deploy-static  build Vue → src/dist/"
	@echo "  docker-up      start via docker compose (:8080)"
	@echo "  docker-down    stop docker compose"
	@echo ""
	@echo "Other:"
	@echo "  clean          cargo clean"
