# Trending Agent

AI-native A 股短线交易决策工作台。「AI 情绪周期分析终端」

[预览](./docs/preview.png)

## 快速开始

```bash
pre-commit install   # 安装 hook
cargo build          # 构建

# CLI
cargo run                                # 全量数据，Markdown → stdout
cargo run -- --json                      # JSON → stdout
cargo run -- --data limit_up,overview    # 指定数据源
cargo run -- --ai                        # 全量 + AI 分析

# Web
cargo run --bin trending-agent-web       # API 服务 :8080
cd src && bun install && bun run dev     # Vue 前端 :5173

# Docker
docker compose up -d
```

## 数据源

| 数据源 | 内容 | 认证 |
|--------|------|------|
| 选股宝 | 涨停/跌停、板块异动、市场概览 | 公开 API |
| 韭研公社 | 社群文章、时间线、情绪 | 手机号登录 |
| 淘股吧 | 用户帖子、游资观点 | HTML 爬虫 |

## CLI 数据源

通过 `--data` 按需获取：

| 数据源 | 说明 |
|--------|------|
| `overview` | 市场概览（涨跌家数/炸板率） |
| `limit_up` | 涨停股票 |
| `limit_down` | 跌停股票 |
| `super_stock` | 强势股（未涨停但强） |
| `field_item` | 韭研公社异动板块 |
| `plate_abnormal` | 板块异动事件 |
| `timeline` | 韭研公社时间线 |
| `sentiment` | AI 舆情分析 |
| `cls_telegraph` | 财联社电报 |
| `dapan_abnormal` | 大盘异动 |
| `market_trend` | 市场趋势 |
| `community` | 社区文章 |

```bash
cargo run -- --data limit_up,sentiment           # 涨停+舆情
cargo run -- --data cls_telegraph --ai           # 电报+AI分析
cargo run -- --data field_item --json --output data.json
```

## 技术栈

| Layer | Tech |
|-------|------|
| 后端 | Rust (Cargo Workspace: core + cli + web + wasm) |
| 前端 | Vue 3, TypeScript, Vite, TailwindCSS, Pinia, lightweight-charts |
| 包管理 | Bun |
| LLM | DeepSeek (可切换 OpenAI-compatible API) |
| 部署 | Docker Compose, Vercel, RackNerd |

## 配置

在 `.env` 中配置环境变量：

| 变量 | 必需 | 用途 |
|------|------|------|
| `LLM_API_KEY` | AI 必需 | LLM API key（DeepSeek 或 OpenAI-compatible） |
| `LLM_MODEL` | 可选 | 默认 `deepseek-chat` |
| `LLM_API_ENDPOINT` | 可选 | 默认 DeepSeek API |
| `JIUYAN_PHONE` | 韭研公社必需 | 登录手机号 |
| `JIUYAN_PASSWORD` | 可选 | 韭研公社密码 |

## 常用命令

```bash
# CLI
cargo run                                   # 全量 Markdown
cargo run -- --json                         # 全量 JSON
cargo run -- --data limit_up,overview       # 按数据源
cargo run -- --ai                           # 全量 + AI

# Web
cargo run --bin trending-agent-web          # API 服务
cd src && bun run dev                       # Vue 前端

# 质量检查
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test -- --ignored                     # 集成测试（需 JIUYAN_PHONE）
```

## 架构

```
crates/core/    # 业务逻辑（API 客户端 / Agent / LLM / 报告 / 模板）
crates/cli/     # CLI 二进制
crates/web/     # Axum Web 服务
crates/wasm/    # WASM 绑定
src/            # Vue 3 前端
src-tauri/      # Tauri 桌面应用
tests/          # 集成测试
```

`crates/core` 不含框架依赖（无 Clap/Axum/Tauri），所有交付方式都是调用 `core` 的薄封装层。

更多细节 → `.claude/context/engineering.md`

## 部署

```bash
# Docker
docker compose up -d

# RackNerd
make deploy-racknerd

# Vercel 静态部署
[![Deploy with Vercel](https://vercel.com/button)](https://vercel.com/new/clone?repository-url=https%3A%2F%2Fgithub.com%2Foouxx%2Ftrending-agent)
```
