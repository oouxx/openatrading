# OpenATrading

A 股短线行情与舆情工作台。聚合选股宝、韭研公社、财联社等五个数据源，agent 在对话里做行情与舆情分析，模拟盘交易由你在 Web 面板手动记账。

## Features

- 多源数据聚合：选股宝（涨停跌停/板块涨跌榜/大盘异动/K 线/分时）、韭研公社时间线、财联社电报、东财龙虎榜、新浪集合竞价
- 项目 Skill：韭研板块异动与市场情报由 agent 按需获取，动态 token 和浏览器侧凭证不进入 Rust 运行链路
- 模拟盘手动记账：在 Web 面板按角色记录买卖，你填成交价与股数，系统只记账不撮合、不计手续费，持仓按实时行情估值
- 定时自动化：内置 cron 调度器，盘中采集数据、推送财联社 A/B 级电报
- 多平台通知：Telegram / Discord / Slack / Lark 单向推送，双向对话仅 Telegram / Discord
- MCP 双向：作为客户端连接外部数据源，也作为 server 对外暴露行情工具

## Project Skills

仓库内置两个按需执行的韭研公社 Skill：

- `/skill:jiuyan-sector-movements`：盘中板块异动、题材轮动与资金主线
- `/skill:jiuyan-market-intelligence`：社区文章、热点股票与讨论情绪

Skill 的源文件放在 `.agents/skills`，可被 pi-rs 以及其他支持 Agent Skills 的工具直接读取。OpenATrading 只把标准路径交给 pi-rs，Skill 的发现、解析、系统提示注入和 `/skill:` 展开均由 pi-rs 完成。

Skill 在运行时自行处理 `timestamp/token` 和 `SESSION`，Rust Gateway 不再提供这两个模块的数据接口。动态 token 通常约 10 分钟失效，执行 Skill 时应按需刷新。

## Architecture

```
crates/
├── api/         第三方数据源客户端（选股宝 / 韭研公社 / 财联社 / 东财 / 新浪）
├── core/        核心业务：配置、交易、记忆、角色、技能、自动化
├── extension/   行情工具扩展（注册进 pi-rs 扩展系统）
├── agent/       pi-rs 会话生命周期与事件协议
├── web/         Axum REST + SSE + WebSocket
├── channels/    通知渠道（Telegram / Discord / Slack / Lark）
└── cli/         CLI 入口与 gateway 生命周期
src/            React 19 + Vite 前端
src-tauri/      Tauri 桌面端
```

所有交付方式（CLI / Web / Desktop）都是对 `core` 的薄封装层。LLM 调用、会话管理、MCP 客户端、技能加载等能力复用 [pi-rs](https://github.com/oouxx/pi-rs)。

## Quick Start

### 安装

CLI 与服务端二进制从 [GitHub Releases](https://github.com/oouxx/openatrading/releases/latest) 分发。

一行安装（Linux / macOS）：

```bash
curl -fsSL https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh | bash
```

不想走管道就先看脚本再执行（更稳妥）：

```bash
curl -fsSLO https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh
less install.sh && bash install.sh

INSTALL_DIR=~/.local/bin bash install.sh   # 指定安装目录
```

脚本默认从 Releases 取对应平台的二进制；要改用自建镜像源时
`DL_SERVER=https://<your-domain> bash install.sh`。

也可以直接下载二进制放进 `PATH`：

| 平台 | 资产名 |
|------|--------|
| Linux x86_64 | `openatrading-x86_64-unknown-linux-gnu` |
| Linux aarch64 | `openatrading-aarch64-unknown-linux-gnu` |
| macOS Intel | `openatrading-x86_64-apple-darwin` |
| macOS Apple Silicon | `openatrading-aarch64-apple-darwin` |

升级：`openatrading update`（同样走 Releases；`OPENATRADING_DL_SERVER` 可覆盖下载源）。

### 初始化

```bash
openatrading init      # 创建 ~/.atrading/ 与默认配置
```

LLM 配置不在 `config.toml`：在 `~/.atrading/agent/settings.json` 设置 `defaultProvider` / `defaultModel`，在 `auth.json` 填 API Key。

### CLI

```bash
openatrading doctor                  # 诊断环境与服务状态
openatrading gateway run             # 前台启动 Web + Cron + MCP
openatrading gateway run --web-only  # 仅 Web
openatrading gateway run --cron-only # 仅定时任务
openatrading gateway run --no-mcp    # 不启动 MCP server
openatrading gateway install         # 安装 systemd 服务
openatrading skills list             # 已安装技能
openatrading skills install --name my-skill ./SKILL.md
openatrading mcp list                # 列出 MCP 端点上的工具
openatrading mcp serve               # 对外暴露行情工具（MCP server）
openatrading tools doc               # 生成工具参考文档
openatrading config show             # 查看配置
openatrading channels list           # 通知渠道编译状态
openatrading update                  # 从 GitHub Releases 自更新
```

### 模拟盘

交易在 Web / 桌面端面板里手动记录。agent 只提供行情与舆情分析，不参与下单，也读不到持仓。

- 每个角色一个独立账户，初始资金在建角色时设定（默认 100 万），之后不可修改：它决定现金与权益曲线的起点，改它等于改写全部历史。要换本金请新建角色。
- 账户只有重置一条重来路径：清空全部持仓与成交流水，历史不可恢复（初始资金不变）
- 买卖由你填写成交价与股数，系统只记账：不做撮合（没有滑点，也不生成模拟成交价），不计手续费和印花税
- 同一标的多次买入合并为一行，按加权平均成本计；卖出支持部分卖出
- 持仓按实时行情估值，行情拿不到时估值留空并提示，不会用成本价冒充现价
- 现金由成交流水推导，不单独落库，因此不会与流水对不上

### Web 面板

`openatrading gateway run` 后访问 `:8080`。前端开发：

```bash
cd src && bun install && bun run dev
```

### Docker

```bash
cp .env.example .env      # 填 JIUYAN_* / 通知渠道等，至少 JIUYAN_SECRET
docker compose pull       # 从 GHCR 拉已发布镜像
docker compose up -d --no-build
```

本地改代码需要重新构建时用 `docker compose up -d --build`。

### 桌面端（Tauri）

从 Homebrew 安装（推荐）：

```bash
brew tap oouxx/openatrading
brew install --cask openatrading
```

cask 安装时会自动清除 macOS 隔离属性，装完直接双击打开即可。

不用 Homebrew 的话，直接到
[Releases](https://github.com/oouxx/openatrading/releases/latest) 下载对应平台的
`.dmg` / `.deb` / `.AppImage` / `.msi` / `.exe`。自行编译则在
`src-tauri/target/release/bundle/` 获取安装包。

> **从网页下载 DMG 的用户注意**：Release 页面的安装包是 ad-hoc 签名（未经 Apple 公证），首次打开会被 Gatekeeper 拦下。先清除隔离属性再打开：
>
> ```bash
> xattr -dr com.apple.quarantine /Applications/OpenATrading.app
> ```

## Configuration

配置位于 `~/.atrading/config.toml`（`$OPENATRADING_HOME` 可整体改根目录）。`~/.atrading` 是唯一品牌根，pi-rs 的 agent 目录内嵌为 `agent/`，用户只需记一个目录：

```
~/.atrading/
├── config.toml      openatrading 配置
├── data.db          记忆 / 交易 / 自动化
├── roles/  watchlist.json  .env
└── agent/           pi-rs 的家
    ├── settings.json  auth.json      LLM provider / model / API Key
    ├── sessions/  skills/  mcp.json  会话 / 技能 / 外部 MCP server
```

```toml
[service]
db_path  = "data.db"
web_bind = "0.0.0.0:8080"

[cron]
enabled             = true
collection_schedule = "0 */30 9-15 * * 1-5"   # 6 字段：秒 分 时 日 月 周
news_schedule       = "0 */5 9-15 * * 1-5"

[social]
notify_channel = "telegram"    # telegram / discord / slack / lark

[social.telegram]
enabled   = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id   = "${TELEGRAM_CHAT_ID}"

[mcp_server]                   # 本服务对外暴露的 MCP server
enabled = true
bind    = "0.0.0.0:8082"
```

环境变量按 `~/.atrading/.env` → `{cwd}/.atrading/.env` 顺序加载（后者覆盖前者），`config.toml` 中用 `${VAR}` 引用：

| 变量 | 用途 |
|------|------|
| `OPENATRADING_HOME` | 自定义品牌根目录 |
| `JIUYAN_PHONE` / `JIUYAN_PASSWORD` | 韭研公社登录 |
| `JIUYAN_SECRET` | 韭研公社接口签名密钥（必填，代码内不内置） |
| `JIUYAN_SESSION_TOKEN` | Skill 使用的韭研公社 `SESSION` token |
| `JIUYAN_TIMESTAMP` / `JIUYAN_TOKEN` | Skill 使用的动态请求签名 |
| `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` | Telegram |
| `DISCORD_BOT_TOKEN` / `DISCORD_CHANNEL_ID` | Discord |
| `SLACK_BOT_TOKEN` / `SLACK_CHANNEL` | Slack |
| `LARK_APP_ID` / `LARK_APP_SECRET` / `LARK_RECEIVE_ID` | Lark |
| `OPENATRADING_DL_SERVER` | 自建下载源（默认走 GitHub Releases） |

完整模板见 [`.env.example`](.env.example)，复制为 `.env` 后填写。

### 约定：仓库里不存个人/部署环境信息

下面这些一律走环境变量或本机配置，**不要写回仓库**：

| 项 | 怎么传 |
|----|--------|
| 部署 | 走 GitHub Actions，服务器路径用仓库变量 `ATRADING_*` 指定（见下） |
| 下载源 | 默认 GitHub Releases；自建镜像用 `DL_SERVER`（`install.sh`）或 `OPENATRADING_DL_SERVER`（CLI 自更新） |
| 对外域名 | 部署端 nginx/证书层决定，`nginx.conf` 默认 `server_name _` |
| 前端 API 地址 | 默认同源；需要跨域时 `--build-arg VITE_API_BASE_URL=...` 或面板里设置 |
| Vercel 反代目标 | 部署前把 `src/public/vercel.json` 的 `YOUR_API_HOST` 换成自己的 |

部署由 [`.github/workflows/deploy.yml`](.github/workflows/deploy.yml) 负责：

- **构建**跑在 GitHub 托管 runner 上，镜像推到 GHCR
- **部署**跑在 self-hosted runner（目标服务器）上，只做 `docker compose pull` + `up -d --no-build`
- 触发：push 到 `main` 自动部署；或 Actions → Deploy → Run workflow 选择 `all`/`gateway`/`web`
- 服务器侧状态放在 `/opt/openatrading/`（`.env`、`downloads/`、`atrading/`、`certs/`），
  路径可用仓库变量 `ATRADING_ENV_FILE` / `ATRADING_HOST_HOME` /
  `ATRADING_DOWNLOADS_DIR` / `ATRADING_CERTS_DIR` 覆盖

## Data Sources

| 数据源 | 内容 |
|--------|------|
| 选股宝 | 涨停 / 跌停 / 强势股、市场概览与涨跌分布、大盘/板块异动、板块涨跌榜与成分股、板块摘要/指数、实时行情、K 线、分时走势、个股所属板块、批量资金流 |
| 韭研公社 | Rust 提供时间线；板块异动和市场情报由 `.agents/skills` 中的项目 Skill 完成 |
| 财联社 | 电报快讯（A/B 级自动推送） |
| 东财龙虎榜 | 席位详情、个股席位、合并、活跃营业部、研报 |
| 新浪 | 集合竞价快照 |

## MCP

**客户端** — 连接外部 MCP server 的能力由 pi-rs 提供，配置为标准的 `mcpServers` 形状，同名时项目级覆盖全局：

| 作用域 | 路径 |
|--------|------|
| 全局 | `~/.atrading/agent/mcp.json`（Web 面板「设置 → MCP 服务器」写入） |
| 项目 | `{cwd}/.pi-rs/mcp.json`（界面中只读，需直接编辑） |

```json
{
  "mcpServers": {
    "akshare": { "url": "http://localhost:8081/mcp" },
    "legacy":  { "type": "sse", "url": "https://example.com/sse" },
    "fs":      { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "."] },
    "off":     { "command": "some-server", "enabled": false }
  }
}
```

省略 `type` 或写 `"http"` = streamable HTTP，`"sse"` = 旧版 HTTP+SSE，有 `command` = stdio。鉴权写 `headers`（值支持 `${VAR}` 与 `!命令`），`enabled: false` 保留配置但不连接。配置在每次创建会话时读取，改动后新建会话即生效。

**Server** — 把 29 个行情工具按 MCP 标准对外暴露，可被 Claude Desktop / Cursor 等 MCP 客户端消费：

```bash
openatrading mcp serve --transport stdio                     # 本地客户端
openatrading mcp serve --transport http --bind 0.0.0.0:8082  # 远程调用
```

随 gateway 启动由 `[mcp_server] enabled` 控制。

## 内置扩展

pi-rs 内置的三个扩展可以在 Web 面板「设置 → 扩展」里逐个开关，开关写入 pi-rs 的 `$PI_RS_HOME/agent/settings.json`（字段 `extensionsEnabled`）：

| 扩展 | id | 提供的能力 |
|------|----|-----------|
| Goal | `goal` | `goal_complete` / `goal_blocked` / `goal_wait` 工具与 `/goal` 命令 |
| 子代理 | `subagent` | `subagent` 工具（独立上下文的子任务） |
| 网页搜索 | `web_search` | `web_search` / `web_fetch` 工具 |

```json
{
  "extensionsEnabled": { "goal": true, "subagent": false, "web_search": true }
}
```

省略的 id 视为启用；本应用不管理的扩展 key 会原样保留。停用后 Agent 立即失去该扩展的工具，对应斜杠命令也从命令面板消失、直接发送会被拒绝并提示去设置里重新开启；已打开的会话不需要重启，下一轮就按新的工具集工作。

## Automation

对话内让 agent 创建定时任务（工具 `create_automation` / `list_automations` / `update_automation`），也可用 REST `/api/automations` 或桌面端面板。任务持久化在 `data.db`，由 gateway 进程内的 cron 执行；桌面端跨进程的增删改由调度器每 60s 指纹比对 reconcile。

- **调度类型** — `daily` / `weekly` / `every_minutes` / `cron`（6 字段）
- **运行范围** — `new_thread`（独立会话，跑完即弃）或 `existing_thread`（复用指定会话）
- **可控项** — `cwd`、`read_only`（禁用 bash/edit/write）、`timeout_secs`（默认 600s）
- **收件箱** — 有产出 → 未读 + 通知；产出为空 → 自动归档不打扰；失败 → 未读；运行记录保留 30 天

内置任务（随 gateway 启动，不可删除）：

| 任务 | 计划 | 说明 |
|------|------|------|
| 数据采集 | 交易日按 `collection_schedule` | 选股宝大盘 / 涨停 / 强势股快照落库 |
| 新闻推送（盘中） | 交易日按 `news_schedule` | 财联社新增 A/B 级电报推送 |
| 新闻推送（盘后） | 每日 16:00–次日 8:59 每 30 分钟 | 同上 |

## Development

```bash
cargo build
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test

cd src && bun install && bun run dev     # 前端
```

常用 Makefile 目标：`make check`（fmt + lint + test）、`make run-web`、`make run-frontend`、`make run-tauri`。

## License

MIT
