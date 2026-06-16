# AppContext 统一构建设计

## 目标

消除 5 个入口点重复的 `LlmClient`/`ApiClient`/`build_tools` 构造代码，统一为 `AppContext::build()` 单次构造。

## 原则

- 只统一 LLM + API + Tools 三个基础设施依赖
- Gateway 依赖（memory、trade、social）保持按需注入
- 不改变函数行为，不引入新依赖

## AppContext 定义

```rust
// crates/core/src/context.rs

pub struct AppContext {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub tools: Vec<Arc<DynTool>>,
}

impl AppContext {
    pub async fn build(
        provider: Option<&str>,
        model: Option<&str>,
        mcp_servers: &[McpServerConfig],
    ) -> Self {
        let llm = LlmClient::from_config(provider, model);
        let api = ApiClient::new();
        let tool_cfg = ToolConfig { mcp_servers: mcp_servers.to_vec() };
        let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
        Self { llm, api, tools }
    }
}
```

Location: `crates/core/src/context.rs`，通过 `crates/core/src/lib.rs` 的 `pub mod context;` 导出。

## 入口变更

### cmd_decision

```diff
- let llm = LlmClient::from_config(provider, model);
- let api = Client::new();
- let tool_cfg = ToolConfig { mcp_servers: app_config.mcp_servers.clone() };
- let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
+ let ctx = AppContext::build(
+     app_config.llm.provider.as_deref(),
+     app_config.llm.model.as_deref(),
+     &app_config.mcp_servers,
+ ).await;

- run_decision(memory, trade, social, &llm, &api, &roles, &tools)
+ run_decision(
+     &CronContext { llm: ctx.llm, api: ctx.api, memory, trade, social },
+     &roles,
+     &ctx.tools,
+ )
```

### cmd_chat → TUI

```diff
- let llm = LlmClient::from_config(provider, model);
- let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
+ let ctx = AppContext::build(provider, model, &mcp_servers).await;

- ChatArgs { llm, tools, system_prompt }
+ ChatArgs { llm: ctx.llm, tools: ctx.tools, system_prompt }
```

### gateway run — cron 调度器

`SchedulerContext` 的构造改为使用 `ctx.llm` / `ctx.api`，`start_scheduler` 内部用 `ctx.tools` 替代原本的 `build_tools` 调用。

### gateway run — Web 服务

`Gateway::start()` 不再内部构造 LlmClient/ApiClient，改为外部传入：

```diff
- Gateway::start(self, config)
+ Gateway::start(self, config, ctx: &AppContext)

  // 内部
- let state = Arc::new(AppState { llm: LlmClient::from_config(...), api: ApiClient::new(), ... });
+ let state = Arc::new(AppState { llm: ctx.llm.clone(), api: ctx.api.clone(), ... });
```

### gateway run — TelegramBot

```diff
- TelegramBot::new(config, roles, memory, trade)
+ TelegramBot::new(config, roles, memory, trade, llm: LlmClient, api: ApiClient)

  // 内部不再构造 LlmClient/ApiClient
```

### Web chat handler

`AppState` 增加 `tools` 字段，启动时从 `AppContext` 注入：

```diff
- let tool_cfg = ToolConfig { mcp_servers: state.mcp_servers.clone() };
- let tools = build_tools(&tool_cfg).await.all_tools().to_vec();
+ let tools = state.tools.clone();  // 从启动时注入的 AppState 获取
```

## 不在本次范围

- MemoryStore/TradeManager/Social 的进一步抽象——它们已经在 gateway run 中通过 Arc 共享
- LlmClient 单例化——当前 clone 成本已足够低，统一构造即可
- ApiClient 连接池改造
