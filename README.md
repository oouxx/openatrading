# Trending Agent

AI-native A 股短线交易决策工作台。「AI 情绪周期分析终端」

[预览](./docs/preview.png)

## 快速开始

```bash
# 安装 pre-commit hook
pre-commit install

# 构建
cargo build

# 运行（完整报告）
cargo run

# 完整报告 + AI 分析 + Web UI
cargo run -- run --ai --web --port 8080

# Vue 前端开发
cd src && bun install && bun run dev

# Docker
docker compose up -d
```

## 数据源

| 数据源 | 内容 | 认证 |
|--------|------|------|
| 选股宝 | 涨停/跌停、板块异动、市场概览 | 公开 API |
| 韭研公社 | 社群文章、时间线、情绪 | 手机号登录 |
| 淘股吧 | 用户帖子、游资观点 | HTML 爬虫 |

## AI 分析

多角色 AI 分析（游资操盘手 / 量化分析师 / 风控官 / 量价低吸派），通过 OpenRouter 调用 LLM。

```bash
cargo run -- sentiment    # 情绪分析
cargo run -- technical    # 技术分析
cargo run -- run --ai     # 完整报告 + AI
cargo run -- agent        # 交互模式
```

## 技术栈

| Layer | Tech |
|-------|------|
| 后端 | Rust (Cargo Workspace: core + cli + web + wasm) |
| 前端 | Vue 3, TypeScript, Vite, TailwindCSS, Pinia, lightweight-charts |
| 包管理 | Bun |
| LLM | OpenRouter (deepseek/deepseek-chat) |
| 部署 | Docker Compose, Vercel, RackNerd |

## 配置

在 `.env` 中配置环境变量：

| 变量 | 必需 | 用途 |
|------|------|------|
| `OPENROUTER_API_KEY` | AI 分析必需 | OpenRouter API key |
| `OPENROUTER_MODEL` | 可选 | 默认 `deepseek/deepseek-chat` |
| `JIUYAN_PHONE` | 韭研公社必需 | 登录手机号 |
| `JIUYAN_PASSWORD` | 可选 | 韭研公社密码 |
| `TAOGUBA_USER` | 可选 | 淘股吧用户名 |
| `TAOGUBA_PASSWORD` | 可选 | 淘股吧密码 |

## 常用命令

```bash
# 单数据源
cargo run -- limit-up        # 涨停
cargo run -- overview        # 市场概览
cargo run -- timeline        # 时间线

# 质量检查
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test -- --ignored      # 集成测试（需 JIUYAN_PHONE）

# 提交前检查
pre-commit run --all-files
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
