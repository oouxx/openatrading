# MCP Server: 将 market_tools 暴露为标准 MCP 接口

**日期**: 2026-06-14
**状态**: Approved (brainstorming → writing-plans)

## 背景

`crates/core/src/llm/tools.rs::ToolRegistry::market_tools()` 已经聚合了 13 个行情类工具（涨停/跌停/市场概览/K 线/财联社等），底层是 `Arc<DynTool>`(`pi-agent-core::AgentTool`)。目前这些工具仅在 agent loop 内被调用，**没有对外的标准协议**。

需求：把行情工具按 [Model Context Protocol](https://modelcontextprotocol.io/) 标准暴露出去，让 Claude Desktop、Cursor、远程客户端等可以直接消费；同时与现有 gateway（web + cron + telegram）一并启动。

依赖：`rmcp 1.7`，目前已用其 client 能力（`crates/core/src/llm/mcp_tools.rs`），需要补 server 与 transport-io / transport-streamable-http-server feature。

## 目标

1. 提供一个标准 MCP server，暴露 17 个公开行情工具（13 个 market + field_items + 3 个 lhb）。
2. 同时支持 **streamable HTTP** 和 **stdio** 两种传输。
3. 通过 `trading-agent mcp serve` 独立启动，也能随 `trading-agent gateway run` 与 web/cron/telegram 一起跑。
4. 复用现有 `DynTool`，不重写任何工具的 schema 与执行逻辑。

## 非目标

- 不暴露 `submit_trading_decision`（agent 终止器）。
- 不暴露 `get_current_positions` / `get_historical_context`（依赖运行时 trade/memory 状态）。
- 不实现 MCP resources / prompts，仅工具。
- 不实现客户端鉴权 / TLS（HTTP 由上游网关或 reverse proxy 负责）。

## 总体架构

```
                 ┌────────────────────────────────────────┐
                 │  trading-agent gateway run             │
                 │  ┌──────────┐ ┌────────┐ ┌──────────┐  │
                 │  │ web :8080│ │ cron   │ │ telegram │  │
                 │  └──────────┘ └────────┘ └──────────┘  │
                 │  ┌──────────────────────────────────┐  │
                 │  │ mcp HTTP :8082  (新增)           │  │
                 │  └──────────────────────────────────┘  │
                 └────────────────────────────────────────┘
                              │
                              ▼
       crates/core/src/llm/mcp_server.rs
       MarketToolsServer { tools: Vec<Arc<DynTool>> }
       impl rmcp::ServerHandler
              │
              ▼
       Vec<Arc<DynTool>>  ←  tools.rs 工厂函数 (复用)
```

也可独立运行：

```
trading-agent mcp serve --transport stdio        # Claude Desktop / Cursor
trading-agent mcp serve --transport http --bind  # 单独部署
```

## 模块边界

### 新增：`crates/core/src/llm/mcp_server.rs`

```rust
pub struct MarketToolsServer { tools: Vec<Arc<DynTool>> }

impl MarketToolsServer {
    pub fn new(tools: Vec<Arc<DynTool>>) -> Self;
}

// rmcp ServerHandler 实现：list_tools / call_tool

pub async fn serve_stdio(
    tools: Vec<Arc<DynTool>>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()>;

pub async fn serve_http(
    tools: Vec<Arc<DynTool>>,
    bind: SocketAddr,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()>;
```

只导出 `MarketToolsServer` 和两个 `serve_*` 入口；rmcp 类型不外漏。`shutdown` 用 `watch<bool>` + `tokio::select!` 协同退出。

### 修改：`crates/cli/src/cli/mcp_cmd.rs`

新增 `pub async fn cmd_mcp_serve(transport: McpTransport, bind: String) -> anyhow::Result<()>`：
1. 调用 `tools.rs` 工厂函数构造 17 个 `Arc<DynTool>`。
2. 创建 `watch<bool>(false)`，spawn `tokio::signal::ctrl_c()` 任务在收到信号时置 true。
3. 根据 transport 调用 `serve_stdio` / `serve_http`。

### 修改：`crates/cli/src/cli/gateway_cmd.rs::cmd_gateway_run`

在现有 cron / web / telegram spawn 之后插入：

```rust
if do_mcp {
    let mcp_bind: SocketAddr = app_config.mcp_server.bind.parse()?;
    let tools = build_market_mcp_tools();  // 复用 mcp_cmd 中的工厂
    let shutdown_rx = ...;                 // 同 web 共享或单独 watch
    handles.push(tokio::spawn(async move {
        if let Err(e) = serve_http(tools, mcp_bind, shutdown_rx).await {
            tracing::error!("[gateway] mcp error: {e}");
        }
    }));
}
```

`do_mcp = !cron_only && !web_only && !no_mcp && app_config.mcp_server.enabled`。

### 修改：`crates/cli/src/cli/app.rs`

```rust
#[derive(Subcommand)]
pub enum McpAction {
    List   { ... }   // 已存在
    Status { ... }   // 已存在
    Call   { ... }   // 已存在
    /// 启动 MCP 服务端，对外暴露行情工具
    Serve {
        #[arg(long, value_enum, default_value_t = McpTransport::Http)]
        transport: McpTransport,
        #[arg(long, default_value = "0.0.0.0:8082")]
        bind: String,
    },
}

#[derive(Copy, Clone, ValueEnum)]
pub enum McpTransport { Http, Stdio }

#[derive(Subcommand)]
pub enum GatewayAction {
    Run {
        #[arg(long)] cron_only: bool,
        #[arg(long)] web_only: bool,
        #[arg(long)] no_mcp: bool,    // 新增
    },
    ...
}
```

### 修改：`crates/cli/src/cli/config.rs`

```rust
pub struct AppConfig {
    ...
    pub mcp_server: McpServerTomlConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerTomlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mcp_bind")]
    pub bind: String,
}

fn default_mcp_bind() -> String { "0.0.0.0:8082".into() }
```

`config.example.toml` 新增：

```toml
[mcp_server]
enabled = true
bind    = "0.0.0.0:8082"
```

> 命名注意：`mcp_server`（单数，本服务暴露的 server）与现有 `mcp_servers`（client 端连接的远端 servers 列表）刻意区分。

## 工具列表（17 个）

直接复用 `tools.rs` 工厂函数：

| 来源模块 | 工具 |
|---|---|
| XuanguBao | get_limit_up, get_limit_down, get_super_stock, get_market_overview, get_yesterday_limit_up, get_dapan_abnormal, get_realtime_quote, get_plates_by_stock, get_market_trend, get_plate_abnormal, get_kline |
| CLS       | get_telegraph, get_telegraph_allday |
| 韭研公社  | get_field_items |
| Eastmoney | get_lhb_detail, get_lhb_combined, get_lhb_active_yyb |

## 适配关键点

### Schema

`DynTool.parameters_schema` 已是合法 JSON Schema (`type: object`)，直接转 rmcp `Tool::input_schema`（`Arc<serde_json::Map>`）。

### 调用

`call_tool(name, args)`：
1. 在 `Vec<Arc<DynTool>>` 按 name 查；找不到返回 rmcp `ErrorCode::METHOD_NOT_FOUND`。
2. 执行 `tool.execute("mcp".into(), args, None, None).await`。
3. `AgentToolResult.content: Vec<ContentBlock>` → `Vec<rmcp::Content>`：`Text` 直接映射；其它变体（当前不会出现）按 `to_string` 退化。
4. 失败：`Box<dyn Error>` → `ErrorData::internal_error(format!("{e}"), None)`，不暴露内部细节。

### 优雅停机

- HTTP：`axum::serve(...).with_graceful_shutdown(async move { let _ = rx.changed().await; })`。
- stdio：rmcp transport 在 stdin EOF 时自然退出；额外 `tokio::select!` 监听 shutdown_rx 强制结束。

## 依赖

`crates/core/Cargo.toml`：

```toml
rmcp = { version = "1.7", features = [
    "client",
    "server",
    "transport-streamable-http-client-reqwest",
    "transport-streamable-http-server",
    "transport-io",
] }
```

> feature 名以 rmcp 1.7 实际暴露为准，构建时按编译错误调整。

## 错误处理

- `MarketToolsServer::call_tool` 内部用 `match` 分类：not-found → `METHOD_NOT_FOUND`；执行失败 → `internal_error(Display)`。
- `serve_http` / `serve_stdio` 返回 `anyhow::Result<()>`，上层 spawn 的任务用 `tracing::error!` 记录失败但不让 gateway 整体崩溃。
- 不在生产路径使用 `unwrap()` / `expect()`；schema 转换中如有不可能 panic 的不变量需附注释。

## 测试矩阵

| 类型 | 用例 |
|---|---|
| Happy | `MarketToolsServer::list_tools()` 返回 17 个，每项 schema `type=object` |
| Happy | `call_tool("get_market_overview", {})` 返回非空 text content（mock 底层 HTTP） |
| Happy | `call_tool("get_realtime_quote", {"symbol":"600519.SS"})` 转发底层并返回 |
| Edge | `call_tool("non_exist", {})` → METHOD_NOT_FOUND |
| Edge | `call_tool("get_kline", {"symbol":"x","period":"bad","count":"20"})` → internal_error |
| Edge | HTTP 并发 5 个 list_tools 无死锁 |
| Failure | 底层 HTTP 超时（mock）→ 错误正确传播为 internal_error |
| Integration | `gateway run` 同时起 web(8080) + mcp(8082)，互不干扰；Ctrl+C 双双优雅退出 |

集成测试位置：`crates/core/tests/mcp_server.rs`（in-memory transport，不打外部网络）。

## CLI 用法

```bash
# 独立 mcp（Claude Desktop / Cursor，stdio）
trading-agent mcp serve --transport stdio

# 独立 mcp（HTTP）
trading-agent mcp serve --transport http --bind 127.0.0.1:9000

# Gateway 全家桶
trading-agent gateway run                       # web + cron + telegram + mcp
trading-agent gateway run --no-mcp              # 仅关 mcp
trading-agent gateway run --cron-only           # 兼容旧行为，只跑 cron（mcp 自动关）
```

stdio 模式下所有日志走 stderr（已用 `tracing`），不污染 stdout 协议帧。

## 文档同步

落地时需要更新：
- `README.md`：新增 MCP server 一节，含 stdio / HTTP 两种用法。
- `config.example.toml`：新增 `[mcp_server]` 段。
- `.claude/context/`（如有架构图）：补 mcp 节点。
