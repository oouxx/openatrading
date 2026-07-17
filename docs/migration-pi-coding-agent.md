# 迁移报告：pi-agent-core → pi-coding-agent + 模块职责重划

## 状态：✅ 已完成

迁移于 2026-07-16 完成，全 workspace 编译通过。

---

## 目标

1. **依赖切换**：agent 核心从 `pi-agent-core` 切换到 `pi-coding-agent`
2. **职责重划**：`crates/agent` 提供完整 agent 能力，`crates/core` 专注于业务层

---

## 现状：职责混乱

```
crates/provider (openatrading-provider)
  ├── LlmClient          ← LLM 客户端 + Agent 构造
  ├── pi-ai re-exports   ← 流式 API、模型注册
  └── pi-agent-core      ← 直接依赖

crates/agent (openatrading-agent)
  └── 透传 re-export pi-agent-core  ← 几乎无实际代码

crates/core (openatrading-core)
  ├── llm/session.rs     ← AgentSession、invoke_agent（agent 调度）
  ├── llm/tools.rs       ← ToolRegistry、工具定义、decision tool
  ├── llm/mcp_tools.rs   ← McpClient、McpRegistry、build_tools
  ├── llm/mcp_server.rs  ← MCP HTTP/stdio server
  ├── llm/mod.rs         ← re-export 编排
  ├── roles/             ← 交易策略、prompt（业务）
  ├── pipelines/         ← 交易 pipeline（业务）
  ├── agents/            ← 交易 agent（业务）
  ├── trade/             ← 交易引擎（业务）
  ├── cron/              ← 定时任务（业务）
  ├── social/            ← Telegram（业务）
  └── ...
```

**问题**：agent 调度、工具系统、MCP 等通用 agent 能力散落在 `core` 中，`agent` 反而只是个空壳。

---

## 目标架构

```
crates/agent (openatrading-agent)  ← 提供完整 agent 能力
  ├── LlmClient                    ← LLM 客户端（从 provider 移入）
  ├── AgentSession                 ← agent 会话管理（从 core 移入）
  ├── ToolRegistry                 ← 工具注册表（从 core 移入）
  ├── McpRegistry / McpClient      ← MCP 工具发现（从 core 移入）
  ├── build_tools()                ← 工具构建入口（从 core 移入）
  ├── MCP server                   ← MCP 服务端（从 core 移入）
  ├── pi-coding-agent              ← 新主依赖
  │   └── pi-agent-core            ← 传递依赖
  └── pi-ai                        ← LLM 流式 API（从 provider 移入）

crates/provider (openatrading-provider)
  └── 精简为 pi-ai 的薄封装层，或被 agent 吸收后废弃

crates/core (openatrading-core)    ← 纯业务层
  ├── roles/                       ← 交易角色、策略 prompt
  ├── pipelines/                   ← 交易 pipeline
  ├── agents/                      ← 交易 agent（sentiment、technical）
  ├── trade/                       ← 交易引擎、paper trading
  ├── cron/                        ← 定时任务
  ├── social/                      ← Telegram bot
  ├── memory/                      ← 记忆存储
  ├── api/                         ← 外部 API 客户端
  ├── skills/                      ← 技能注册表
  ├── commands/                    ← CLI 命令
  ├── chat/                        ← 聊天逻辑
  ├── context/                     ← 应用上下文
  ├── types/                       ← 核心类型
  └── utils/                       ← 工具函数
  │
  └── 依赖 crates/agent 获取 agent 能力
      （不再直接引用 pi-agent-core / pi-coding-agent）

crates/cli / crates/tui / crates/web
  └── 依赖 crates/core（间接使用 agent）
```

---

## 迁移步骤

### Step 1: 统一 pi-rs 版本

`crates/provider/Cargo.toml` 当前用 `v1.78.14`，`crates/agent/Cargo.toml` 用 `v1.79.8`。统一到 `v1.79.8`。

### Step 2: 重构 `crates/agent` — 吸收 agent 能力

**Cargo.toml**：
```toml
[dependencies]
pi-coding-agent = { git = "https://github.com/oouxx/pi-rs", tag = "v1.79.8" }
pi-agent-core = { git = "https://github.com/oouxx/pi-rs", tag = "v1.79.8", features = ["extraction"] }
pi-ai = { git = "https://github.com/oouxx/pi-rs", tag = "v1.79.8" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
async-trait = { workspace = true }
thiserror = "2"
rmcp = { version = "1.7", features = [
    "client", "server",
    "transport-streamable-http-client-reqwest",
    "transport-streamable-http-server",
    "transport-io", "transport-child-process",
] }
futures = { workspace = true }
async-stream = "0.3"
schemars = { version = "0.8", features = ["derive"] }
tracing = "0.1"

[features]
default = []
mcp = ["pi-agent-core/mcp"]
```

**新增模块**（从 `crates/core` 和 `crates/provider` 移入）：

| 新路径 | 来源 | 说明 |
|--------|------|------|
| `src/llm.rs` | `crates/provider/src/llm.rs` | `LlmClient`、`StreamFn`、流式 API |
| `src/session.rs` | `crates/core/src/llm/session.rs` | `AgentSession`、`invoke_agent` |
| `src/tools.rs` | `crates/core/src/llm/tools.rs` | `ToolRegistry`、工具定义、`create_decision_tool` |
| `src/mcp_tools.rs` | `crates/core/src/llm/mcp_tools.rs` | `McpClient`、`McpRegistry`、`build_tools` |
| `src/mcp_server.rs` | `crates/core/src/llm/mcp_server.rs` | MCP HTTP/stdio server |

**`src/lib.rs`**：
```rust
pub mod llm;         // LlmClient, StreamFn, 流式 API
pub mod session;     // AgentSession, invoke_agent
pub mod tools;       // ToolRegistry, 工具定义
pub mod mcp_tools;   // McpClient, McpRegistry, build_tools
pub mod mcp_server;  // MCP server

// 透传 pi-agent-core 类型（上层大量使用）
pub use pi_agent_core::agent;
pub use pi_agent_core::agent_loop;
pub use pi_agent_core::extraction;
pub use pi_agent_core::harness;
#[cfg(feature = "mcp")]
pub use pi_agent_core::mcp;
pub use pi_agent_core::pi_ai_types;
pub use pi_agent_core::proxy;
pub use pi_agent_core::types;

// 暴露 pi-coding-agent 的高层 API
pub use pi_coding_agent::core::agent_session::AgentSession as CodingAgentSession;
pub use pi_coding_agent::core::sdk;
```

### Step 3: 精简 `crates/provider`

`crates/provider` 的 `LlmClient` 移入 `crates/agent` 后，provider 变为 pi-ai 的薄封装层：

```toml
[dependencies]
pi-ai = { git = "https://github.com/oouxx/pi-rs", tag = "v1.79.8" }
```

```rust
// src/lib.rs
pub use pi_ai::api_registry;
pub use pi_ai::env_api_keys;
pub use pi_ai::models;
pub use pi_ai::providers;
pub use pi_ai::stream;
pub use pi_ai::types as pi_types;
```

> 如果 provider 没有其他消费者，可考虑合并到 `crates/agent` 后废弃此 crate。

### Step 4: 精简 `crates/core` — 只保留业务逻辑

**Cargo.toml** 变更：
- 移除 `rmcp` 依赖（移到 agent）
- 移除 `schemars` 依赖（移到 agent，仅 `DecisionOutput` schema 需要）
- 移除 `openatrading-provider` 依赖（agent 能力通过 `openatrading-agent` 获取）
- 保留 `openatrading-agent` 依赖

**移除模块**（移到 `crates/agent`）：
- `src/llm/session.rs`
- `src/llm/tools.rs`
- `src/llm/mcp_tools.rs`
- `src/llm/mcp_server.rs`

**`src/llm/mod.rs`** 改为从 `crates/agent` re-export：
```rust
pub use openatrading_agent::llm::{
    assistant_event_to_stream_events, LlmClient, StreamEvent, StreamFn, StreamFnOptions,
    StreamResult, ToolCall, ToolCallFunction, ToolCallItem, ToolCallMessage, ToolChatChoice,
    ToolChatResponse, ToolStreamResult,
};
pub use openatrading_agent::mcp_tools::{
    build_tools, McpClient, McpRegistry, McpServerConfig, McpTransportConfig, ToolConfig,
};
pub use openatrading_agent::tools::ToolRegistry;
pub use openatrading_agent::mcp_server::{serve_http, serve_stdio, MarketToolsServer};
pub use openatrading_agent::session::AgentSession;
pub use openatrading_agent::agent::{Agent, AgentOptions};
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{...};
```

### Step 5: 更新消费者

**`crates/cli`**：
- `check_llm.rs`：`openatrading_provider::llm::LlmClient` → `openatrading_core::llm::LlmClient`
- `mcp_cmd.rs`、`gateway_cmd.rs`、`tools_cmd.rs`：import 路径不变（仍走 `openatrading_core::llm`）

**`crates/tui`**：
- `chat.rs`：import 路径不变（仍走 `openatrading_core::llm`）

---

## 影响范围

| 文件 | 变更类型 | 风险 |
|------|---------|------|
| `crates/agent/Cargo.toml` | 大幅扩展依赖 | 中 |
| `crates/agent/src/lib.rs` | 从透传层变为功能模块 | 中 |
| `crates/agent/src/llm.rs` | **新建**（从 provider 移入） | 高 |
| `crates/agent/src/session.rs` | **新建**（从 core 移入） | 高 |
| `crates/agent/src/tools.rs` | **新建**（从 core 移入） | 中 |
| `crates/agent/src/mcp_tools.rs` | **新建**（从 core 移入） | 中 |
| `crates/agent/src/mcp_server.rs` | **新建**（从 core 移入） | 中 |
| `crates/provider/Cargo.toml` | 精简依赖 | 低 |
| `crates/provider/src/lib.rs` | 移除 LlmClient | 低 |
| `crates/core/Cargo.toml` | 精简依赖 | 低 |
| `crates/core/src/llm/mod.rs` | 改为从 agent re-export | 低 |
| `crates/core/src/llm/session.rs` | **删除**（移到 agent） | — |
| `crates/core/src/llm/tools.rs` | **删除**（移到 agent） | — |
| `crates/core/src/llm/mcp_tools.rs` | **删除**（移到 agent） | — |
| `crates/core/src/llm/mcp_server.rs` | **删除**（移到 agent） | — |
| `crates/cli/src/cli/doctor/check_llm.rs` | import 路径更新 | 低 |
| `crates/tui/src/chat.rs` | import 路径更新 | 低 |

---

## 最终依赖图

```
crates/cli / crates/tui / crates/web
  └── crates/core (openatrading-core)  ← 纯业务
        ├── crates/agent (openatrading-agent)  ← 完整 agent 能力
        │     ├── pi-coding-agent
        │     │     └── pi-agent-core (传递)
        │     ├── pi-agent-core (直接，类型透传 + extraction + mcp)
        │     └── pi-ai
        ├── crates/api
        ├── crates/channels
        └── ...

crates/provider (openatrading-provider)
  └── 已废弃，无任何 crate 依赖。保留 workspace member 待后续移除。
```

---

## 完成记录

### Phase 1 — 代码搬迁 ✅

| 操作 | 文件 |
|------|------|
| 统一版本 v1.79.8 | `crates/provider/Cargo.toml` |
| 新建 LlmClient | `crates/agent/src/llm.rs`（从 provider 移入） |
| 新建 invoke_agent | `crates/agent/src/session.rs`（从 core 移入，不含交易决策逻辑） |
| 新建工具辅助函数 | `crates/agent/src/tools.rs`（从 core 移入） |
| 新建 MCP 基础设施 | `crates/agent/src/mcp_tools.rs`（从 core 移入） |
| 新建 MCP server | `crates/agent/src/mcp_server.rs`（从 core 移入） |

### Phase 2 — 依赖切换 ✅

| 操作 | 说明 |
|------|------|
| core 移除 rmcp 依赖 | 已移到 agent |
| core/llm/mod.rs 重写 | 所有 agent 能力从 `openatrading_agent` re-export |
| core/llm/session.rs 精简 | 使用 agent 的 `invoke_agent`，保留 `process_decision` |
| core/llm/tools.rs 精简 | 使用 agent 的工具辅助函数，保留业务工具 + ToolRegistry |
| core/llm/mcp_tools.rs | 改为 agent 透传层 |
| core/llm/mcp_server.rs | 改为 agent 透传层 |
| CLI import 更新 | `check_llm.rs` 改用 `openatrading_core::llm::LlmClient` |

### Phase 3 — 清理与优化 ✅

| 操作 | 说明 |
|------|------|
| 废弃 provider | 移除所有 crate 对 `openatrading-provider` 的依赖（core、web、cli） |
| 清理 agent re-export | 移除 `agent_loop`、`proxy`、`mcp`（feature gate）等未使用的透传 |
| 移除 agent 的 mcp feature | pi-agent-core 的 mcp feature 已直接启用，无需透传 |
| 评估 AgentSession 替换 | 决定不替换 — 两者 API 模型不匹配 |
| 更新文档 | 本文件标记为完成状态 |
