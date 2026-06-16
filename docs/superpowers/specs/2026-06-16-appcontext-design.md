# AppContext 统一上下文设计

## 目标

将 7 个入口点中重复的 `LlmClient`/`ApiClient`/`build_tools` 构造代码统一为 `AppContext::build()`，同时将 `MemoryStore`/`TradeManager`/`NotificationSender` 纳入统一上下文，消除 `CronContext`/`SchedulerContext` 等临时结构体。

## 原则

- llm + api + tools 由 `build()` 统一构造
- memory + trade + social 由调用方构造后注入（trait object，可替换实现）
- `NotificationSender` 解耦业务类型，搬到 `channels` crate，用 blanket impl 自动桥接

---

## 1. NotificationSender 重构（前置步骤）

### 1.1 解耦业务类型

所有 `notify_*` 方法参数改为 `&str`，去掉对 `crate::cron` 的依赖：

```rust
// channels/src/lib.rs

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_text(&self, text: &str) -> Result<()>;

    async fn notify_collect(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_decision(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_trade(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
    async fn notify_news(&self, text: &str) -> Result<()> {
        self.send_text(text).await
    }
}
```

### 1.2 Blanket impl：所有 Channel 自动获得 NotificationSender

```rust
// channels/src/lib.rs

#[async_trait]
impl<T: Channel + Send + Sync> NotificationSender for T {
    async fn send_text(&self, text: &str) -> Result<()> {
        Channel::send_text(self, text).await
    }
}
```

新增平台只需 `impl Channel`，零桥接代码。

### 1.3 core::social 精简

`core::social` 只保留业务逻辑类型：
- `FilteredSender`（按事件类型过滤）
- `NoopSender` / `StdoutSender`（基础实现）
- `TelegramBot`
- `pub use openatrading_channels::*`（re-export）

---

## 2. AppContext 定义

```rust
// crates/core/src/context.rs

use std::sync::Arc;
use crate::api::Client as ApiClient;
use crate::llm::{build_tools, DynTool, LlmClient, McpServerConfig, ToolConfig};
use crate::memory::MemoryStore;
use crate::trade::TradeManager;
use crate::social::NotificationSender;

pub struct AppContext {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub tools: Vec<Arc<DynTool>>,
    pub memory: Arc<dyn MemoryStore>,
    pub trade: Arc<dyn TradeManager>,
    pub social: Arc<dyn NotificationSender>,
}

impl AppContext {
    pub async fn build(
        provider: Option<&str>,
        model: Option<&str>,
        mcp_servers: &[McpServerConfig],
        memory: Arc<dyn MemoryStore>,
        trade: Arc<dyn TradeManager>,
        social: Arc<dyn NotificationSender>,
    ) -> Self {
        let llm = LlmClient::from_config(provider, model);
        let api = ApiClient::new();
        let tool_cfg = ToolConfig { mcp_servers: mcp_servers.to_vec() };
        let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
        Self { llm, api, tools, memory, trade, social }
    }
}
```

Location: `crates/core/src/context.rs`，通过 `crates/core/src/lib.rs` 的 `pub mod context;` 导出。

---

## 3. 各入口变更

### 3.1 CLI `cmd_chat`

```diff
- let llm = LlmClient::from_config(provider, model);
- let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
+ let ctx = AppContext::build(provider, model, &mcp_servers, memory, trade, social).await;

- ChatArgs { llm, tools, system_prompt }
+ ChatArgs { llm: ctx.llm.clone(), tools: ctx.tools.clone(), system_prompt }
```

### 3.2 CLI `cmd_decision`

```diff
- let llm = LlmClient::from_config(provider, model);
- let api = Client::new();
- let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
+ let ctx = AppContext::build(provider, model, &mcp_servers, memory, trade, social).await;

- run_decision(&cron_ctx, &roles, &tools).await
+ run_decision(&ctx, &roles).await  // 内部从 ctx.tools 取 tools
```

### 3.3 TUI `run_chat`

`ChatArgs` 改为持有 `AppContext` 引用或克隆所需字段。

### 3.4 Cron 调度器

删除 `CronContext` 和 `SchedulerContext`：

```diff
- pub struct CronContext { ... }
- pub struct SchedulerContext { ... }

- pub async fn start_scheduler(ctx: SchedulerContext) -> Result<()>
+ pub async fn start_scheduler(ctx: &AppContext, config: CronConfig, roles: Vec<RoleEntry>) -> Result<()>
```

内部：
- `ctx.llm` / `ctx.api` / `ctx.tools` / `ctx.memory` / `ctx.trade` / `ctx.social` 直接使用
- 删除 `build_tools` 调用（line 136-142），改用 `ctx.tools.clone()`
- `run_decision` 签名改为 `(&AppContext, &[RoleEntry])`

通知调用改为自行拼接消息：

```diff
- let _ = ctx.social.notify_collect(&summary).await;
+ let _ = ctx.social.notify_collect(&format!("📊 采集完成\n...")).await;
```

### 3.5 Gateway Web 服务

`AppState` 内嵌 `AppContext`：

```rust
pub struct AppState {
    pub ctx: AppContext,
    pub roles: Arc<RwLock<Vec<RoleEntry>>>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub cache: Arc<AppCache>,
}
```

```diff
- Gateway::start(self, config)
+ Gateway::start(self, config, ctx: AppContext)

  // 内部
- let state = Arc::new(AppState { llm: LlmClient::from_config(...), api: ApiClient::new(), ... });
+ let state = Arc::new(AppState { ctx, roles: ..., mcp_servers: ..., cache: ... });
```

### 3.6 Web chat handler

保持 per-request `build_tools`（MCP 配置变更无需重启）：

```rust
// 不变：每次请求从 state.mcp_servers 构建 tools
let tool_cfg = openatrading_core::llm::ToolConfig {
    mcp_servers: state.mcp_servers.clone(),
};
let tools = openatrading_core::llm::build_tools(&tool_cfg).await.all_tools().to_vec();
```

### 3.7 TelegramBot

```diff
- TelegramBot::new(config, roles, memory, trade)
+ TelegramBot::new(config, roles, ctx: &AppContext)

  // 内部删除 LlmClient::from_config 和 build_tools 调用
  // 改用 ctx.llm.clone(), ctx.api.clone(), ctx.tools.clone(), ctx.memory.clone(), ctx.trade.clone()
```

### 3.8 测试

`crates/tui/tests/chat_e2e.rs`：

```diff
- llm: LlmClient::from_config(None, None),
+ ... AppContext::build(None, None, &[], memory, trade, Arc::new(NoopSender)).await ...
```

---

## 4. 不在本次范围

- `LlmClient` 单例化——当前 clone 成本已足够低
- `ApiClient` 连接池改造
- MCP tools 动态热加载

---

## 5. 变更文件清单

| 文件 | 变更类型 |
|------|----------|
| `crates/channels/src/lib.rs` | `NotificationSender` trait 移入 + blanket impl |
| `crates/core/src/social/mod.rs` | 精简为 re-export + 业务类型，删除依赖 `cron` 的 trait 默认实现 |
| `crates/core/src/social/telegram_bot.rs` | 构造函数接收 `&AppContext` |
| `crates/core/src/context.rs` | **新建**，`AppContext` 定义 + `build()` |
| `crates/core/src/lib.rs` | 添加 `pub mod context;` |
| `crates/core/src/cron/mod.rs` | 删除 `CronContext`/`SchedulerContext`，签名改为 `&AppContext`，消息格式化移到调用方 |
| `crates/cli/src/cli/mod.rs` | `cmd_chat` + `cmd_decision` 使用 `AppContext::build()` |
| `crates/cli/src/cli/gateway_cmd.rs` | cron 入口构造 `AppContext` 传入 `start_scheduler` |
| `crates/web/src/lib.rs` | `AppState` 内嵌 `AppContext` |
| `crates/web/src/gateway.rs` | `Gateway::start` 接收 `AppContext` |
| `crates/web/src/chat.rs` | 从 `state.ctx` 取 llm/api（tools 保持 per-request） |
| `crates/tui/tests/chat_e2e.rs` | 使用 `AppContext::build()` |
