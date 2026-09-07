# OpenATrading

AI-native A 股短线交易决策工作台。集成实时行情、多源舆情、AI 决策引擎，支持 CLI / Web / TUI / 桌面端四种交互方式。

## Features

- **多源数据聚合** — 选股宝（涨停跌停/板块异动/大盘异动）、韭研公社（社区情绪/时间线）、财联社电报、龙虎榜
- **AI 驱动决策** — 接入 LLM，结合持仓分析、市场情绪、策略技能，生成每日买卖建议
- **定时自动化** — 内置 cron 调度器：盘中数据采集、盘后决策、新闻推送
- **灵活部署** — CLI 直接运行、Web 面板、TUI 终端、Tauri 桌面端、Docker 容器化
- **MCP 协议** — 支持标准 MCP 工具调用，扩展数据源和分析能力
- **多角色策略** — 支持同时运行多个交易角色，每个角色独立策略和仓位管理
- **多平台通知** — Telegram / Discord / Slack / Lark 推送

## Architecture

```
openatrading
├── crates/
│   ├── core/        # 核心业务逻辑：API 客户端、LLM、cron、角色、技能
│   ├── cli/         # CLI 二进制入口，gateway 生命周期管理
│   ├── web/         # Axum Web 服务，提供 REST API
│   ├── tui/         # 终端 TUI 界面（pi-tui）
│   ├── api/         # 第三方 API 封装（选股宝、韭研公社、财联社等）
│   ├── channels/    # 通知渠道抽象（Telegram / Discord / Slack / Lark）
│   ├── agent/       # Agent 相关逻辑
│   ├── provider/    # 数据 provider
│   └── channels/    # 通知渠道实现
├── src/             # Vue 3 + Vite 前端
├── src-tauri/       # Tauri 桌面端
├── opendesign/      # 原型设计稿
└── Dockerfile*      # 容器化部署
```

所有交付方式（CLI / Web / TUI / Desktop）都是对 `core` 的薄封装层。

## Quick Start

### 初始化

```bash
openatrading init              # 创建 ~/.atrading/ 和默认配置
```

### CLI

```bash
openatrading                   # 全量数据 → Markdown
openatrading --json            # JSON 输出
openatrading --data limit_up,overview   # 指定数据源
openatrading --ai              # 全量 + AI 分析
openatrading decision          # 手动触发决策
openatrading decision --dry-run --role zarili   # 指定角色试运行
openatrading doctor            # 诊断服务状态
openatrading skills list       # 列出可安装技能
openatrading mcp list          # 列出 MCP 工具
openatrading mcp serve         # 启动 MCP 行情工具服务器
openatrading tools doc         # 生成工具参考文档
openatrading trade add --role zarili --symbol 000001 --price 10.5 --shares 100
openatrading trade list        # 持仓列表
openatrading trade close --id 1 --price 11.0
```

### Web 面板

```bash
openatrading gateway run          # 启动 Web + Cron + Telegram Bot（前台）
openatrading gateway run --web-only   # 仅启动 Web
openatrading gateway run --cron-only  # 仅启动定时任务
```

Web 服务默认监听 `:8080`，配套 Vue 3 前端：

```bash
cd src && bun install && bun run dev
```

### TUI 终端

```bash
openatrading chat              # AI 对话界面
```

### Docker

```bash
docker compose up -d
```

### 桌面端（Tauri）

编译后在 `src-tauri/target/release/bundle/` 获取安装包。

## Configuration

配置文件位于 `~/.atrading/config.toml`（可通过 `$OPENATRADING_HOME` 环境变量自定义路径）。
LLM 模型/Provider 配置已移交 pi-rs，在 `$PI_RS_HOME/agent/settings.json` 中设置 `defaultProvider` / `defaultModel`。

```toml
[[mcp_servers]]
name = "akshare"
transport = { type = "streamable-http", url = "http://akshare-mcp:8081/mcp" }
enabled = true

[service]
db_path = "data.db"

[cron]
enabled = true
collection_schedule = "0 0 18 * * 1-5"   # 交易日 18:00(6 字段:秒 分 时 日 月 周)
news_schedule = "0 */5 9-15 * * 1-5"     # 盘中每 5 分钟

[social]
notify_channel = "telegram"
[social.telegram]
enabled = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id = "${TELEGRAM_CHAT_ID}"
```

环境变量（也支持从 `~/.atrading/.env` 加载）：

| 变量 | 必需 | 用途 |
|------|------|------|
| `LLM_API_KEY` | AI 功能必需 | LLM API key |
| `TELEGRAM_BOT_TOKEN` | Telegram 通知 | Bot token |
| `TELEGRAM_CHAT_ID` | Telegram 通知 | 聊天 ID |
| `JIUYAN_PHONE` | 韭研公社 | 登录手机号 |
| `OPENATRADING_HOME` | 可选 | 自定义配置目录 |

## Data Sources

| 数据源 | 内容 | 访问方式 |
|--------|------|---------|
| 选股宝 | 涨停/跌停/板块异动/大盘异动/市场趋势 | HTTP API |
| 韭研公社 | 社区文章/时间线/异动 | HTTP API（需登录） |
| 财联社 | 电报快讯（A/B 级自动推送） | HTTP API |
| 龙虎榜 | 席位详情/个股上榜 | HTTP API |
| MCP 工具 | akshare 等数据扩展 | Streamable HTTP |

## MCP Server

把 17 个公开行情工具按 [Model Context Protocol](https://modelcontextprotocol.io/) 标准对外暴露，可直接被 Claude Desktop、Cursor、远程 agent 等 MCP 客户端消费。

### 独立运行

```bash
# stdio：用于 Claude Desktop / Cursor 本地配置
openatrading mcp serve --transport stdio

# HTTP：远程或脚本调用
openatrading mcp serve --transport http --bind 0.0.0.0:8082
```

### 随 gateway 启动

```bash
openatrading gateway run             # web + cron + telegram + mcp 全开
openatrading gateway run --no-mcp    # 仅关 mcp
```

### 配置

```toml
[mcp_server]
enabled = true              # gateway run 时是否一并启动
bind    = "0.0.0.0:8082"    # mcp serve / gateway 的默认 bind
```

> 注意：`mcp_server`（单数）配置本服务暴露的 server；`mcp_servers`（复数）是连接的远端 server 列表。

### 支持的工具

涨停/跌停/强势股/市场概览/昨日涨停/大盘异动/实时行情/股票板块/分时走势/板块异动/K 线/财联社电报/韭研公社/东财龙虎榜（详情/合并/活跃营业部）。

## Strategies（技能）

策略以 `SKILL.md` 形式存储在 `~/.atrading/skills/{name}/`，使用 YAML frontmatter + Markdown 描述。

内置策略（`openatrading skills list` 查看）：
- **zarili** — 量价低吸派，市场温度择时 + 涨停股池 + 分时低吸

## Automation

定时自动化复刻自 Codex 桌面端 Scheduled Tasks,**收件箱语义与其对齐,但实现与使用方式有以下差异**:

| | Codex 原版 | 本项目 |
|---|---|---|
| 存储 | 桌面端本地任务文件 | SQLite(与 memory/trade 同库),CLI / Web / 桌面端共享同一份数据 |
| 创建入口 | 桌面端 UI 表单 | 对话内让 agent 创建(工具 `create_automation` / `list_automations` / `update_automation`)、REST API `/api/automations`、桌面端面板 |
| 调度器 | 桌面端进程托管 | gateway 进程内 cron(6 字段表达式 + Asia/Shanghai);桌面端跨进程的增删改由调度器每 60s 指纹比对 reconcile |
| 执行环境 | 起新会话执行 prompt | 同为独立 agent 会话,但可控 `cwd`、`read_only`(禁用 bash/edit/write 与交易决策)、`timeout_secs`(默认 600s,上限 24h) |

- **调度类型** — `daily` / `weekly` / `every_minutes` / `cron`(6 字段「秒 分 时 日 月 周」)
- **运行范围** — `new_thread`(独立会话,跑完即弃)或 `existing_thread`(复用指定会话)
- **收件箱** — 运行有产出 → 未读 + 通知;有产出为空 → 自动归档不打扰;失败 → 未读;运行记录保留 30 天

### 对话创建示例

直接跟 agent 说:「每个交易日早上 8:30 给我一份盘前新闻早报」。agent 会先调 `list_automations` 去重,再通过 `create_automation` 落库并热更新调度器。

### 内置任务(随 gateway 启动,不可删除)

| 任务 | 计划 | 说明 |
|------|------|------|
| 数据采集 | 交易日 18:00(`collection_schedule` 可配置) | 选股宝大盘/涨停/强势股快照落库 |
| 新闻推送(盘中) | 交易日 9:00–15:59 每 5 分钟(`news_schedule` 可配置) | 财联社新增 A/B 级电报推送 |
| 新闻推送(盘后) | 每日 16:00–次日 8:59 每 30 分钟(固定) | 同上 |

## Desktop App

Tauri 桌面端提供原生体验，支持 macOS / Linux / Windows。可通过 GitHub Releases 下载安装包。

## Development

```bash
# 构建
cargo build

# 运行 CLI
cargo run -- --ai

# 前端开发
cd src && bun install && bun run dev

# 质量检查
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

MIT
