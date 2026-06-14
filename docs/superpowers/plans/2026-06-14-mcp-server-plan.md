# MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `ToolRegistry::market_tools()` 等 17 个公开行情工具用 rmcp 包成标准 MCP Server，支持 stdio + streamable HTTP 双传输；既能 `trading-agent mcp serve` 独立运行，也能随 `gateway run` 与 web/cron/telegram 一起启动。

**Architecture:** 在 `crates/core/src/llm/mcp_server.rs` 新增 `MarketToolsServer`，实现 rmcp `ServerHandler`：内部持有 `Vec<Arc<DynTool>>`，把 list/call 直接代理到现有 `DynTool.execute`，零重写 schema。CLI 层在 `mcp_cmd::cmd_mcp_serve` 与 `gateway_cmd::cmd_gateway_run` 共享同一组工具构造函数，用 `tokio::sync::watch<bool>` 协同 Ctrl+C 优雅停机。

**Tech Stack:** Rust, rmcp 1.7 (server feature), axum, tokio, clap, tracing。

**Spec:** `docs/superpowers/specs/2026-06-14-mcp-server-design.md`

## File Structure

| 路径 | 操作 | 责任 |
|---|---|---|
| `crates/core/Cargo.toml` | 修改 | 启用 rmcp server / transport-io / streamable-http-server features |
| `crates/core/src/llm/mcp_server.rs` | 新建 | `MarketToolsServer`、`serve_stdio`、`serve_http`、`build_market_tools()` |
| `crates/core/src/llm/mod.rs` | 修改 | `pub mod mcp_server;` 与 re-export |
| `crates/core/tests/mcp_server.rs` | 新建 | list/call/not-found/internal-error 集成测试（in-memory transport） |
| `crates/cli/src/cli/app.rs` | 修改 | `McpAction::Serve`、`McpTransport` enum、`GatewayAction::Run.no_mcp` |
| `crates/cli/src/cli/config.rs` | 修改 | `McpServerTomlConfig` + `AppConfig.mcp_server` |
| `crates/cli/src/cli/mcp_cmd.rs` | 修改 | `cmd_mcp_serve(transport, bind)` |
| `crates/cli/src/cli/gateway_cmd.rs` | 修改 | gateway run 中 spawn mcp http 任务 |
| `crates/cli/src/cli/mod.rs` | 修改 | dispatch `McpAction::Serve` |
| `config.example.toml` | 修改 | 新增 `[mcp_server]` 段 |
| `README.md` | 修改 | 新增 MCP server 用法章节 |

---

## Task 1: 启用 rmcp server features

**Files:**
- Modify: `crates/core/Cargo.toml`

- [ ] **Step 1: 改 rmcp features**

把 `crates/core/Cargo.toml` 中

```toml
rmcp = { version = "1.7", features = ["client", "transport-streamable-http-client-reqwest"] }
```

替换为：

```toml
rmcp = { version = "1.7", features = [
    "client",
    "server",
    "transport-streamable-http-client-reqwest",
    "transport-streamable-http-server",
    "transport-io",
] }
```

- [ ] **Step 2: 验证编译通过**

Run: `cargo check -p openatrading-core`
Expected: 编译成功，无新的 warning（如 feature 名变更，按 rmcp 1.7 实际暴露调整：先在 `~/.cargo/registry/src` 或 `cargo metadata --format-version 1 | jq '.packages[] | select(.name=="rmcp") | .features'` 查 feature 列表）。

- [ ] **Step 3: Commit**

```bash
git add crates/core/Cargo.toml
git commit -m "chore(core): enable rmcp server + stdio + streamable-http-server features"
```

---

## Task 2: 新建 mcp_server 骨架与 build_market_tools

**Files:**
- Create: `crates/core/src/llm/mcp_server.rs`
- Modify: `crates/core/src/llm/mod.rs`

- [ ] **Step 1: 新建 mcp_server.rs 骨架**

```rust
// crates/core/src/llm/mcp_server.rs
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use openatrading_agent::types::DynTool;
use tokio::sync::watch;

use super::tools;

/// 收集所有要对外暴露的行情类 DynTool。
pub fn build_market_tools() -> Vec<Arc<DynTool>> {
    vec![
        tools::get_limit_up(),
        tools::get_limit_down(),
        tools::get_super_stock(),
        tools::get_market_overview(),
        tools::get_yesterday_limit_up(),
        tools::get_dapan_abnormal(),
        tools::get_realtime_quote(),
        tools::get_plates_by_stock(),
        tools::get_market_trend(),
        tools::get_plate_abnormal(),
        tools::get_kline(),
        tools::get_telegraph(),
        tools::get_telegraph_allday(),
        tools::get_field_items(),
        tools::get_lhb_detail(),
        tools::get_lhb_combined(),
        tools::get_lhb_active_yyb(),
    ]
}

/// 持有工具集合，用作 rmcp ServerHandler 状态。
#[derive(Clone)]
pub struct MarketToolsServer {
    tools: Arc<Vec<Arc<DynTool>>>,
}

impl MarketToolsServer {
    pub fn new(tools: Vec<Arc<DynTool>>) -> Self {
        Self {
            tools: Arc::new(tools),
        }
    }

    pub fn tools(&self) -> &[Arc<DynTool>] {
        &self.tools
    }
}

pub async fn serve_stdio(
    _tools: Vec<Arc<DynTool>>,
    _shutdown: watch::Receiver<bool>,
) -> Result<()> {
    anyhow::bail!("not implemented yet")
}

pub async fn serve_http(
    _tools: Vec<Arc<DynTool>>,
    _bind: SocketAddr,
    _shutdown: watch::Receiver<bool>,
) -> Result<()> {
    anyhow::bail!("not implemented yet")
}
```

- [ ] **Step 2: 注册模块**

修改 `crates/core/src/llm/mod.rs`，在 `pub mod mcp_tools;` 下方加：

```rust
pub mod mcp_server;

pub use mcp_server::{build_market_tools, serve_http, serve_stdio, MarketToolsServer};
```

- [ ] **Step 3: 写最小测试验证 build_market_tools**

新建 `crates/core/tests/mcp_server.rs`：

```rust
use openatrading_core::llm::build_market_tools;

#[test]
fn build_market_tools_returns_seventeen() {
    let tools = build_market_tools();
    assert_eq!(tools.len(), 17, "expected 17 public market tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "get_limit_up",
        "get_market_overview",
        "get_kline",
        "get_telegraph",
        "get_field_items",
        "get_lhb_detail",
    ] {
        assert!(names.contains(&required), "missing tool: {required}");
    }
}
```

- [ ] **Step 4: 跑测试**

Run: `cargo test -p openatrading-core --test mcp_server build_market_tools_returns_seventeen`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/llm/mcp_server.rs crates/core/src/llm/mod.rs crates/core/tests/mcp_server.rs
git commit -m "feat(core): scaffold mcp_server module + build_market_tools()"
```

---

## Task 3: 实现 ServerHandler — list_tools

**Files:**
- Modify: `crates/core/src/llm/mcp_server.rs`
- Modify: `crates/core/tests/mcp_server.rs`

> 在动手前先 `grep -rn "ServerHandler\|tool_router" ~/.cargo/registry/src 2>/dev/null | grep rmcp- | head -20` 查 rmcp 1.7 实际 trait 形态，下方代码以 `ServerHandler::list_tools(&self, _: Option<...>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, ErrorData>` 为基准；如签名有出入，按编译错误调整字段名/返回类型，但保持下面的语义不变。

- [ ] **Step 1: 写失败测试**

在 `crates/core/tests/mcp_server.rs` 末尾追加：

```rust
use openatrading_core::llm::MarketToolsServer;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{PaginatedRequestParam, ServerCapabilities};
use rmcp::service::RequestContext;

#[tokio::test]
async fn list_tools_returns_all_seventeen() {
    let server = MarketToolsServer::new(openatrading_core::llm::build_market_tools());
    let info = server.get_info();
    assert!(info.capabilities.tools.is_some());

    // 直接调用 ServerHandler::list_tools，无需启动 transport。
    let result = server
        .list_tools(None, dummy_request_context())
        .await
        .expect("list_tools failed");
    assert_eq!(result.tools.len(), 17);
    for tool in &result.tools {
        let schema_type = tool
            .input_schema
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(schema_type, "object", "{} schema not object", tool.name);
        assert!(!tool.description.as_deref().unwrap_or("").is_empty());
    }
}

// dummy_request_context 在 Task 4 测试也要复用，集中放在文件顶部辅助区。
fn dummy_request_context() -> RequestContext<rmcp::RoleServer> {
    // rmcp 暴露的 helper 视版本而定；若不可直接构造，
    // 用 tokio_util::sync::CancellationToken::new() 作为 ct，其余字段按默认。
    unimplemented!("filled in implementation step")
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cargo test -p openatrading-core --test mcp_server list_tools_returns_all_seventeen`
Expected: FAIL（unimplemented 或 compile error）。

- [ ] **Step 3: 实现 ServerHandler::list_tools 与 get_info**

在 `crates/core/src/llm/mcp_server.rs` 顶部 `use` 后追加（rmcp 1.7 类型，按编译报错微调）：

```rust
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    Implementation, ListToolsResult, PaginatedRequestParam, ProtocolVersion,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};

impl ServerHandler for MarketToolsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "openatrading-market".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some("China A-share market data tools".into()),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .tools
            .iter()
            .map(|t| {
                let schema_obj = t
                    .parameters_schema
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                Tool {
                    name: t.name.clone().into(),
                    description: Some(t.description.clone().into()),
                    input_schema: Arc::new(schema_obj),
                    annotations: None,
                    output_schema: None,
                }
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }
}
```

并把测试里的 `dummy_request_context` 替换为可工作的实现：

```rust
fn dummy_request_context() -> RequestContext<rmcp::RoleServer> {
    use rmcp::service::RequestContext;
    use tokio_util::sync::CancellationToken;
    // RequestContext 字段在 rmcp 1.7 大约形如:
    // { ct, id, meta, extensions, peer }
    // 测试不依赖 peer/id，构造一个最小可编译的实例。
    // 若 rmcp 未导出 public 构造器，把测试改为通过 in-memory transport 走完整路径。
    RequestContext {
        ct: CancellationToken::new(),
        id: rmcp::model::NumberOrString::Number(0),
        meta: Default::default(),
        extensions: Default::default(),
        peer: panic!("peer not constructible in unit test — switch to integration test via transport"),
    }
}
```

> **若 RequestContext 不能手动构造**：删除 `dummy_request_context`，把测试改写成跑 in-memory transport——见 Task 5。本 Task 仅保证 `MarketToolsServer::get_info()` 编译通过、`list_tools` 内部逻辑正确（可加一个不走 trait 的 inherent helper 单测）。

替代方案：在 `MarketToolsServer` 上加一个 inherent 方法做单测：

```rust
impl MarketToolsServer {
    #[doc(hidden)]
    pub fn build_tool_descriptors(&self) -> Vec<Tool> {
        self.tools.iter().map(|t| {
            let schema_obj = t.parameters_schema.as_object().cloned().unwrap_or_default();
            Tool {
                name: t.name.clone().into(),
                description: Some(t.description.clone().into()),
                input_schema: Arc::new(schema_obj),
                annotations: None,
                output_schema: None,
            }
        }).collect()
    }
}
```

并在 `list_tools` 中调用它，避免重复。把测试改成断言 `server.build_tool_descriptors()`。

- [ ] **Step 4: 测试通过**

Run: `cargo test -p openatrading-core --test mcp_server list_tools_returns_all_seventeen`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/llm/mcp_server.rs crates/core/tests/mcp_server.rs
git commit -m "feat(core): MarketToolsServer.list_tools via DynTool.parameters_schema"
```

---

## Task 4: 实现 call_tool — 适配 DynTool

**Files:**
- Modify: `crates/core/src/llm/mcp_server.rs`
- Modify: `crates/core/tests/mcp_server.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/core/tests/mcp_server.rs` 末尾追加：

```rust
#[tokio::test]
async fn call_tool_routes_to_dyn_tool_and_serializes_text() {
    use openatrading_agent::pi_ai_types::ContentBlock;
    use openatrading_agent::types::{AgentTool, AgentToolResult};
    use std::sync::Arc;

    let echo: Arc<openatrading_agent::types::DynTool> = Arc::new(AgentTool {
        name: "echo".into(),
        description: "echo back".into(),
        label: "echo".into(),
        parameters_schema: serde_json::json!({"type":"object","properties":{},"required":[]}),
        execution_mode: None,
        prepare_arguments: None,
        execute: Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text("hello-mcp")],
                    details: serde_json::Value::Null,
                    terminate: None,
                })
            })
        }),
    });

    let server = MarketToolsServer::new(vec![echo]);
    let result = server
        .call_tool_inherent("echo", serde_json::json!({}))
        .await
        .expect("echo failed");
    assert_eq!(result.content.len(), 1);
    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text"),
    };
    assert_eq!(text, "hello-mcp");
}

#[tokio::test]
async fn call_tool_unknown_returns_method_not_found() {
    let server = MarketToolsServer::new(openatrading_core::llm::build_market_tools());
    let err = server
        .call_tool_inherent("does_not_exist", serde_json::json!({}))
        .await
        .expect_err("expected error");
    // ErrorData.code is i32 wrapper; -32601 == method not found
    assert_eq!(err.code.0, -32601);
}

#[tokio::test]
async fn call_tool_propagates_business_error_as_internal() {
    use openatrading_agent::types::AgentTool;
    use std::sync::Arc;

    let bad: Arc<openatrading_agent::types::DynTool> = Arc::new(AgentTool {
        name: "bad".into(),
        description: "always fails".into(),
        label: "bad".into(),
        parameters_schema: serde_json::json!({"type":"object","properties":{},"required":[]}),
        execution_mode: None,
        prepare_arguments: None,
        execute: Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move { Err("boom".into()) })
        }),
    });
    let server = MarketToolsServer::new(vec![bad]);
    let err = server
        .call_tool_inherent("bad", serde_json::json!({}))
        .await
        .expect_err("expected error");
    assert_eq!(err.code.0, -32603); // internal error
    assert!(err.message.contains("boom"));
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cargo test -p openatrading-core --test mcp_server call_tool`
Expected: FAIL（`call_tool_inherent` 未定义）。

- [ ] **Step 3: 实现 call_tool**

在 `mcp_server.rs` 中给 `MarketToolsServer` 加 inherent 方法（trait 实现复用它）：

```rust
use openatrading_agent::pi_ai_types::ContentBlock;
use rmcp::model::{CallToolRequestParam, CallToolResult, Content, RawContent};
use rmcp::ErrorCode;

impl MarketToolsServer {
    pub async fn call_tool_inherent(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ErrorData> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("unknown tool: {name}"),
                    None,
                )
            })?;

        let result = (tool.execute)("mcp".into(), arguments, None, None)
            .await
            .map_err(|e| ErrorData::internal_error(format!("{e}"), None))?;

        let contents: Vec<Content> = result
            .content
            .into_iter()
            .map(|cb| match cb {
                ContentBlock::Text(t) => Content::text(t.text),
                other => Content::text(format!("{:?}", other)),
            })
            .collect();

        Ok(CallToolResult {
            content: contents,
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }
}

impl ServerHandler for MarketToolsServer {
    // ... get_info / list_tools 已在 Task 3 实现 ...

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        self.call_tool_inherent(&request.name, args).await
    }
}
```

> **rmcp 1.7 字段差异**：`ContentBlock` 在 `pi-agent-core` 中可能是 `enum { Text(TextBlock), ... }`。先 `grep -rn "pub enum ContentBlock" ~/.cargo/git ~/.cargo/registry 2>/dev/null | head` 确认变体名（已知 `ContentBlock::text(...)` 工厂可用）。如果 `Text(t)` 解构不可行，用 `cb.as_text().map(|s| s.to_string())` 这类访问器；找不到对应 API 时退化为 `serde_json::to_string(&cb).unwrap_or_default()` 走文本通道。

- [ ] **Step 4: 测试通过**

Run: `cargo test -p openatrading-core --test mcp_server call_tool`
Expected: 三个测试全 PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/llm/mcp_server.rs crates/core/tests/mcp_server.rs
git commit -m "feat(core): MarketToolsServer.call_tool routes DynTool with proper error mapping"
```

---

## Task 5: 实现 serve_stdio

**Files:**
- Modify: `crates/core/src/llm/mcp_server.rs`

- [ ] **Step 1: 实现 serve_stdio**

替换 Task 2 中的 `serve_stdio` 占位：

```rust
use rmcp::transport::io::stdio;
use rmcp::ServiceExt;

pub async fn serve_stdio(
    tools: Vec<Arc<DynTool>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let server = MarketToolsServer::new(tools);
    let transport = stdio();
    let service = server
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("mcp stdio serve: {e}"))?;

    tokio::select! {
        res = service.waiting() => {
            res.map_err(|e| anyhow::anyhow!("mcp stdio waiting: {e}"))?;
        }
        _ = shutdown.changed() => {
            tracing::info!("[mcp] stdio shutdown signal received");
            // service drop 时会发 cancel；这里依赖 Drop。
            drop(service);
        }
    }
    Ok(())
}
```

> 若 `rmcp::transport::io::stdio()` 路径不一致，按 1.7 实际 API 调整（可能是 `rmcp::transport::stdio()` 或 `rmcp::transport::io::Stdio::new()`）。`ServiceExt::serve` 通常接收 transport 并返回 `RunningService`。

- [ ] **Step 2: 编译验证**

Run: `cargo check -p openatrading-core`
Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/llm/mcp_server.rs
git commit -m "feat(core): serve_stdio for MarketToolsServer"
```

---

## Task 6: 实现 serve_http

**Files:**
- Modify: `crates/core/src/llm/mcp_server.rs`

- [ ] **Step 1: 实现 serve_http**

替换 Task 2 中的 `serve_http` 占位：

```rust
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

pub async fn serve_http(
    tools: Vec<Arc<DynTool>>,
    bind: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let server_factory = {
        let tools = Arc::new(tools);
        move || Ok::<_, std::io::Error>(MarketToolsServer::new((*tools).clone()))
    };

    let service = StreamableHttpService::new(
        server_factory,
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| anyhow::anyhow!("bind {bind}: {e}"))?;

    tracing::info!("[mcp] http listening on http://{bind}/mcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.changed().await;
            tracing::info!("[mcp] http shutdown signal received");
        })
        .await
        .map_err(|e| anyhow::anyhow!("mcp http serve: {e}"))?;
    Ok(())
}
```

> rmcp 1.7 streamable HTTP server 的实际类型名以 `cargo doc -p rmcp --open` / `grep -rn "StreamableHttpService" ~/.cargo/registry/src/.../rmcp-1.7*/src` 为准；常见替代是 `rmcp::transport::streamable_http_server::StreamableHttpServerConfig`。signature 调整不影响调用语义。

- [ ] **Step 2: 加 axum 依赖（如未传递）**

检查 `crates/core/Cargo.toml` 是否已有 `axum`。当前 web crate 用 axum，但 core 没用。加：

```toml
axum = { version = "0.8", default-features = false, features = ["http1", "tokio"] }
```

> 版本对齐 web crate；`grep '^axum' crates/web/Cargo.toml` 查实际版本。

- [ ] **Step 3: 编译验证**

Run: `cargo check -p openatrading-core`
Expected: 编译通过。

- [ ] **Step 4: 集成测试 — 端到端 list_tools**

在 `crates/core/tests/mcp_server.rs` 追加：

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_end_to_end_list_tools() {
    use std::net::SocketAddr;
    use tokio::sync::watch;

    let bind: SocketAddr = "127.0.0.1:0".parse().unwrap();
    // 0 端口让 OS 分配；但 serve_http 内部 bind 后 listener.local_addr() 可读。
    // 简化:固定一个高位端口避免改动接口；如冲突可重跑。
    let bind: SocketAddr = "127.0.0.1:18923".parse().unwrap();
    let (tx, rx) = watch::channel(false);

    let tools = openatrading_core::llm::build_market_tools();
    let handle = tokio::spawn(async move {
        openatrading_core::llm::serve_http(tools, bind, rx).await
    });

    // 给 server 100ms 起来
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let client = openatrading_core::llm::McpClient::connect(format!("http://{bind}/mcp"))
        .await
        .expect("connect");
    let tools_json = client.list_tools().await.expect("list");
    let arr = tools_json["tools"].as_array().expect("tools array");
    assert_eq!(arr.len(), 17);

    // 关闭
    let _ = tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), handle).await;
}
```

> 如 `client.list_tools()` 返回的 JSON 顶层不是 `{tools: [...]}` 而是直接 array，按实际 shape 调整断言。

- [ ] **Step 5: 跑测试**

Run: `cargo test -p openatrading-core --test mcp_server http_end_to_end_list_tools -- --nocapture`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/llm/mcp_server.rs crates/core/tests/mcp_server.rs
git commit -m "feat(core): serve_http via rmcp streamable-http + axum graceful shutdown"
```

---

## Task 7: CLI 子命令 `mcp serve`

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Modify: `crates/cli/src/cli/mcp_cmd.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 给 McpAction 加 Serve 变体 + McpTransport 枚举**

修改 `crates/cli/src/cli/app.rs`：

文件顶部 `use clap::{...}` 加 `ValueEnum`：

```rust
use clap::{Parser, Subcommand, ValueEnum};
```

替换 `McpAction`：

```rust
#[derive(Subcommand)]
pub enum McpAction {
    List {
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    Status {
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    Call {
        name: String,
        #[arg(default_value = "{}")]
        args: String,
        #[arg(long, default_value = "http://localhost:8081/mcp")]
        endpoint: String,
    },
    /// 启动 MCP 服务端，对外暴露行情工具
    Serve {
        #[arg(long, value_enum, default_value_t = McpTransport::Http)]
        transport: McpTransport,
        #[arg(long, default_value = "0.0.0.0:8082")]
        bind: String,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum McpTransport {
    Http,
    Stdio,
}
```

- [ ] **Step 2: 实现 cmd_mcp_serve**

在 `crates/cli/src/cli/mcp_cmd.rs` 末尾追加：

```rust
use crate::cli::app::McpTransport;

pub async fn cmd_mcp_serve(transport: McpTransport, bind: &str) -> Result<()> {
    use openatrading_core::llm::{build_market_tools, serve_http, serve_stdio};
    use tokio::sync::watch;

    let tools = build_market_tools();
    let (tx, rx) = watch::channel(false);

    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = tx.send(true);
        }
    });

    let result = match transport {
        McpTransport::Http => {
            let bind: std::net::SocketAddr = bind
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid bind {bind}: {e}"))?;
            tracing::info!("[mcp] http server starting on http://{bind}/mcp");
            serve_http(tools, bind, rx).await
        },
        McpTransport::Stdio => {
            tracing::info!("[mcp] stdio server starting");
            serve_stdio(tools, rx).await
        },
    };

    signal_task.abort();
    result
}
```

- [ ] **Step 3: dispatch**

修改 `crates/cli/src/cli/mod.rs`，把现有 `McpAction` match 块（约 205-217 行）替换为：

```rust
if let Some(Commands::Mcp { action }) = cli.command {
    return match action {
        McpAction::List { endpoint } => cmd_mcp_list(&endpoint).await,
        McpAction::Status { endpoint } => cmd_mcp_status(&endpoint).await,
        McpAction::Call {
            name,
            args,
            endpoint,
        } => cmd_mcp_call(&endpoint, &name, &args).await,
        McpAction::Serve { transport, bind } => cmd_mcp_serve(transport, &bind).await,
    };
}
```

- [ ] **Step 4: 编译 + 烟测**

Run:
```
cargo build -p openatrading-cli
./target/debug/trading-agent mcp serve --help
```
Expected: 显示 `--transport <http|stdio>` `--bind <BIND>`。

```
./target/debug/trading-agent mcp serve --transport http --bind 127.0.0.1:18924 &
sleep 1
./target/debug/trading-agent mcp list --endpoint http://127.0.0.1:18924/mcp
kill %1
```
Expected: 第二条命令打印 17 个工具。

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/cli/app.rs crates/cli/src/cli/mcp_cmd.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): trading-agent mcp serve --transport http|stdio"
```

---

## Task 8: AppConfig.mcp_server 配置段

**Files:**
- Modify: `crates/cli/src/cli/config.rs`
- Modify: `config.example.toml`

- [ ] **Step 1: 加 McpServerTomlConfig**

修改 `crates/cli/src/cli/config.rs`，在 `ServiceConfig` 之后加：

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerTomlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mcp_bind")]
    pub bind: String,
}

impl Default for McpServerTomlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: default_mcp_bind(),
        }
    }
}

fn default_mcp_bind() -> String {
    "0.0.0.0:8082".to_string()
}
```

- [ ] **Step 2: 接入 AppConfig + Raw**

在 `AppConfig` 定义中加字段：

```rust
pub struct AppConfig {
    pub llm: LlmTomlConfig,
    pub mcp_servers: Vec<openatrading_core::llm::McpServerConfig>,
    pub service: ServiceConfig,
    pub mcp_server: McpServerTomlConfig,  // 新增
    pub cron: Option<CronTomlConfig>,
    pub social: Option<SocialConfig>,
}
```

raw deserialize struct（约 30 行附近）也加：

```rust
mcp_server: Option<McpServerTomlConfig>,
```

`load()` 把 raw → AppConfig 时（约 245 行附近）加：

```rust
mcp_server: raw.mcp_server.unwrap_or_default(),
```

`merge` 中（如果有合并逻辑）也加：

```rust
self.mcp_server = other.mcp_server;
```

`Default` 实现里加 `mcp_server: McpServerTomlConfig::default()`。

> 实际定位用：`grep -n "mcp_servers:\|service:\|cron:" crates/cli/src/cli/config.rs`，每处 `mcp_servers` 之后都补一行 `mcp_server`（注意拼写区别）。

- [ ] **Step 3: 加测试**

在 `crates/cli/src/cli/config.rs` 现有 `#[cfg(test)] mod tests` 中加（如无 tests 模块则新建）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_defaults() {
        let cfg = McpServerTomlConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.bind, "0.0.0.0:8082");
    }

    #[test]
    fn config_loads_without_mcp_server_section() {
        let toml_text = r#"
            [llm]
            provider = "anthropic"
            model = "claude-sonnet-4-6"

            [service]
            db_path = "data.db"
        "#;
        let raw: super::Raw = toml::from_str(toml_text).expect("parse");
        // 不直接用 load() 因要 IO；只验证 raw 字段是 Option<McpServerTomlConfig>::None
        assert!(raw.mcp_server.is_none());
    }
}
```

> Raw struct 私有，用 `pub(crate)` 或在测试中用 `super::` 访问。如果当前 raw 是 `Deserialize` 自由结构，调整可见性即可。

- [ ] **Step 4: 跑测试**

Run: `cargo test -p openatrading-cli config::tests::mcp_server`
Expected: 两个测试 PASS。

- [ ] **Step 5: 更新 config.example.toml**

在 `[service]` 段后追加：

```toml
[mcp_server]
enabled = true
bind    = "0.0.0.0:8082"
```

- [ ] **Step 6: Commit**

```bash
git add crates/cli/src/cli/config.rs config.example.toml
git commit -m "feat(cli): [mcp_server] config section with defaults"
```

---

## Task 9: gateway run 集成 mcp

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Modify: `crates/cli/src/cli/gateway_cmd.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 给 GatewayAction::Run 加 --no-mcp**

修改 `crates/cli/src/cli/app.rs`：

```rust
Run {
    #[arg(long)]
    cron_only: bool,
    #[arg(long)]
    web_only: bool,
    /// 不启动 MCP server（默认随 gateway 启动）
    #[arg(long)]
    no_mcp: bool,
}
```

- [ ] **Step 2: 调整 dispatch**

修改 `crates/cli/src/cli/mod.rs` 中 `GatewayAction::Run` 分支，把参数原样传过去：

```rust
GatewayAction::Run {
    cron_only,
    web_only,
    no_mcp,
} => cmd_gateway_run(cron_only, web_only, no_mcp).await,
```

- [ ] **Step 3: cmd_gateway_run 签名 + spawn mcp**

修改 `crates/cli/src/cli/gateway_cmd.rs::cmd_gateway_run`：

签名：

```rust
pub async fn cmd_gateway_run(cron_only: bool, web_only: bool, no_mcp: bool) -> Result<()> {
```

在 `do_cron` / `do_web` 计算附近加：

```rust
let do_mcp = !cron_only
    && !web_only
    && !no_mcp
    && app_config.mcp_server.enabled;
```

在 `// ── Telegram Bot` 之前 / web spawn 之后插入：

```rust
// ── MCP server (CLI layer) ─────────────────────
if do_mcp {
    let mcp_bind: std::net::SocketAddr = app_config
        .mcp_server
        .bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid mcp_server.bind {}: {e}", app_config.mcp_server.bind))?;
    let (mcp_tx, mcp_rx) = tokio::sync::watch::channel(false);
    // 把 stop sender 暂存到外层 Vec，Ctrl+C 时统一通知
    mcp_shutdown.replace(mcp_tx);

    let tools = openatrading_core::llm::build_market_tools();
    let mh = tokio::spawn(async move {
        if let Err(e) = openatrading_core::llm::serve_http(tools, mcp_bind, mcp_rx).await {
            tracing::error!("[gateway] mcp error: {e}");
        }
    });
    handles.push(mh);
    eprintln!("[gateway] mcp: enabled at http://{mcp_bind}/mcp");
}
```

在函数顶部声明：

```rust
let mut mcp_shutdown: Option<tokio::sync::watch::Sender<bool>> = None;
```

并在末尾 Ctrl+C 段后、`for h in handles { h.abort(); }` 之前加：

```rust
if let Some(tx) = mcp_shutdown.take() {
    let _ = tx.send(true);
}
```

> mcp_shutdown 用 `Option<Sender>` 占位，避免 do_mcp=false 时把任意值塞进去。

- [ ] **Step 4: 编译 + 手动验证**

Run:
```
cargo build -p openatrading-cli
./target/debug/trading-agent gateway run --no-mcp
```
Expected: 启动日志中无 `mcp: enabled`。

```
./target/debug/trading-agent gateway run
```
Expected: 启动日志中含 `[gateway] mcp: enabled at http://0.0.0.0:8082/mcp`，且 `curl -s -X POST http://127.0.0.1:8082/mcp -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'` 能拿到 17 个工具（如端口或路径不一致按实际调整）。

Ctrl+C 应同时干掉 web 和 mcp，shutdown 日志双双出现。

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/cli/app.rs crates/cli/src/cli/gateway_cmd.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): gateway run launches mcp http alongside web/cron/telegram"
```

---

## Task 10: README + 文档同步

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 加 MCP server 章节**

在 `README.md` 现有 "工具 / Tools" 或 "Gateway" 章节附近插入：

````markdown
## MCP Server

把 17 个公开行情工具按 [Model Context Protocol](https://modelcontextprotocol.io/) 标准对外暴露，可直接被 Claude Desktop、Cursor、远程 agent 等 MCP 客户端消费。

### 独立运行

```bash
# stdio：用于 Claude Desktop / Cursor 本地配置
trading-agent mcp serve --transport stdio

# HTTP：远程或脚本调用
trading-agent mcp serve --transport http --bind 0.0.0.0:8082
```

### 随 gateway 启动

```bash
trading-agent gateway run             # web + cron + telegram + mcp 全开
trading-agent gateway run --no-mcp    # 仅关 mcp
```

### 配置

```toml
[mcp_server]
enabled = true              # gateway run 时是否一并启动
bind    = "0.0.0.0:8082"    # mcp serve / gateway 的默认 bind
```

> 注意：`mcp_server`（单数）配置本服务暴露的 server；`mcp_servers`（复数）是连接的远端 server 列表。

### 支持的工具

涨停/跌停/强势股/市场概览/昨日涨停/大盘异动/实时行情/股票板块/分时走势/板块异动/K 线、财联社电报、韭研公社、东财龙虎榜（详情/合并/活跃营业部）。
````

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README MCP server usage"
```

---

## Self-Review

- **Spec coverage**：
  - 总体架构 → Task 2/5/6
  - 模块边界（mcp_server.rs / mcp_cmd / gateway_cmd / app / config）→ Task 2-9
  - 17 工具列表 → Task 2 `build_market_tools` 直接列出
  - schema/调用/错误 → Task 3/4
  - 优雅停机 → Task 5/6/9（watch + with_graceful_shutdown）
  - 依赖（rmcp features / axum）→ Task 1/6
  - 测试矩阵全部覆盖：list_tools/call_tool/not-found/internal-error 在 Task 3/4，HTTP e2e 在 Task 6，gateway 双跑在 Task 9 手动验证（自动化集成代价大、本轮跳过）
  - CLI 用法 → Task 7/9
  - 配置段 → Task 8
  - 文档同步 → Task 10
- **Placeholder scan**：每个 step 都有具体代码或命令；rmcp 1.7 类型名标注了 fallback 排错路径，不是 TBD。
- **Type consistency**：
  - `MarketToolsServer::call_tool_inherent(&str, serde_json::Value) -> Result<CallToolResult, ErrorData>` 在 Task 4 定义并被 Task 4 的 trait `call_tool` 调用，签名一致。
  - `serve_http(Vec<Arc<DynTool>>, SocketAddr, watch::Receiver<bool>)` 在 Task 2 声明、Task 6 实现、Task 7/9 调用，签名一致。
  - `build_market_tools() -> Vec<Arc<DynTool>>` 全程统一。

如果 rmcp 1.7 的 `ServerHandler` API 与本计划假设有出入，按编译错误调整字段名/枚举变体即可，**整体语义不变**。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-14-mcp-server-plan.md`. Two execution options:

1. **Subagent-Driven (recommended)** — 每个 task 派一个新的 subagent 执行，task 之间审阅，迭代快。
2. **Inline Execution** — 在当前会话里按 task 顺序执行，带 checkpoint。

哪种方式？
