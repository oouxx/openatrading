# AgentRuntime 统一计划

## 核心原则

**所有 LLM 调用都是流式的。** 没有 one-shot chat，没有 `invoke_agent`。统一走 `create_agent_session` + `subscribe` + `add_user_text`。

## 当前问题

3 条并行 LLM 路径，各自创建 session，各自管理工具：

| 路径               | 入口                  | 流式 | 持久化 | 工具注入           |
| ------------------ | --------------------- | ---- | ------ | ------------------ |
| `invoke_agent`     | cron/telegram/adapter | ❌   | ❌     | custom_tools       |
| `process_messages` | web/tauri             | ✅   | ✅     | extension_registry |
| `LlmClient.chat`   | doctor/replay         | ❌   | ❌     | 无                 |

## 目标架构

```
AppContext
  └── runtime: AgentRuntime          ← 替代 llm: LlmClient
        ├── create_session() → SessionHandle
        └── LlmClient (仅 doctor/extractor 用)
```

### AgentRuntime

```rust
// crates/core/src/llm/runtime.rs

pub struct AgentRuntime {
    ext_registry_factory: fn() -> ExtensionRegistry,  // 每次 create_session 时调用
}

pub struct SessionHandle {
    session: AgentSession,       // pi-coding-agent 的 AgentSession
    session_id: String,          // JSONL file stem
}

impl AgentRuntime {
    pub fn new() -> Self;

    /// 创建新会话（所有 LLM 调用的唯一入口）
    /// - system_prompt: 系统提示词
    /// - custom_tools: 额外工具（positions, historical_context 等）
    /// - persist: 是否持久化到 JSONL
    /// - session_id: 恢复已有会话时传入
    pub async fn create_session(
        &self,
        system_prompt: &str,
        custom_tools: Vec<ToolDefinition>,
        persist: bool,
        session_id: Option<&str>,
    ) -> Result<SessionHandle>;
}

impl SessionHandle {
    /// 订阅事件流（必须在 send 之前调用）
    pub async fn subscribe(&mut self, tx: mpsc::Sender<AgentEvent>) -> UnsubscribeHandle;

    /// 发送用户消息，触发 agent loop（流式事件通过 subscribe 的 tx 推送）
    pub async fn send(&mut self, message: &str);

    /// 获取当前所有消息
    pub async fn get_messages(&self) -> Vec<AgentMessage>;

    /// 中止当前运行
    pub async fn abort(&self);

    /// 获取 session_id
    pub fn session_id(&self) -> &str;
}
```

### 调用方式变化

#### cron / telegram / adapter（原来用 invoke_agent，无流式）

```rust
// 之前
let result = invoke_agent(system_prompt, &msg, custom_tools).await?;

// 之后：创建 session → subscribe → send → 收集结果
let mut handle = ctx.runtime.create_session(&system, custom_tools, false, None).await?;
let (tx, mut rx) = mpsc::channel(256);
handle.subscribe(tx).await;
handle.send(&msg).await;
// 从 rx 收集事件，提取 final_text / tool_output
let result = collect_result(rx).await;
```

#### web / tauri（原来用 process_messages，已有流式）

```rust
// 之前
let result = process_messages(&system_prompt, tools, pi_messages, tx, sid).await;

// 之后：直接用 SessionHandle
let mut handle = ctx.runtime.create_session(&system, custom_tools, true, sid.as_deref()).await?;
handle.subscribe(tx).await;
handle.send(&msg).await;
```

#### doctor / extract_decision_output（原来用 LlmClient.chat）

```rust
// 保留 LlmClient，不走 AgentRuntime（不需要工具、不需要流式）
// 或者也走 runtime，用空 custom_tools
```

## 实现步骤

### Step 1: 创建 `runtime.rs`

- 新文件 `crates/core/src/llm/runtime.rs`
- 实现 `AgentRuntime` 和 `SessionHandle`
- `AgentRuntime::new()` 使用 `openatrading_extension::create_registry` 作为工厂
- `create_session()` 内部调用 `sdk::create_agent_session`，传入 extension_registry + custom_tools

### Step 2: 更新 `AppContext`

- `crates/core/src/context.rs`
- 替换 `llm: LlmClient` 为 `runtime: AgentRuntime`
- 保留 `LlmClient` 的创建能力（doctor/extractor 仍需要 model + api_key）
- 可以在 AppContext 上加一个 `llm_client()` 方法按需创建

### Step 3: 更新 `AgentSession`（trading-specific）

- `crates/core/src/llm/session.rs`
- `AgentSession::process_decision()` 和 `process_chat()` 改为使用 `AgentRuntime::create_session()`
- 内部创建 session → subscribe → send → 收集结果
- 提取公共的 `collect_result(rx)` 辅助函数

### Step 4: 更新 `process_messages` 调用者

- `crates/web/src/chat.rs` — 用 `runtime.create_session()` 替代 `process_messages()`
- `src-tauri/src/main.rs` — 同上
- 删除 `agent_runtime.rs` 中的 `process_messages()` 函数

### Step 5: 更新 cron / telegram / adapter

- `crates/core/src/cron/mod.rs` — 用 `runtime.create_session()` 替代 `AgentSession::new()`
- `crates/core/src/social/telegram_bot.rs` — 同上
- `crates/core/src/social/adapter_handler.rs` — 同上

### Step 6: 清理

- 删除 `crates/agent/src/session.rs` 中的 `invoke_agent()` — 不再需要
- 删除 `crates/core/src/llm/agent_runtime.rs` 中的 `process_messages()` — 被 runtime 替代
- 保留 `create_session()` 函数（仅创建空 session 的工具函数）
- `LlmClient` 保留在 `crates/agent/src/llm.rs`，仅用于 doctor 和 extractor

## 关键文件

| 文件                                        | 操作                                       |
| ------------------------------------------- | ------------------------------------------ |
| `crates/core/src/llm/runtime.rs`            | **新建** — AgentRuntime + SessionHandle    |
| `crates/core/src/llm/session.rs`            | 重写 — 使用 runtime                        |
| `crates/core/src/llm/agent_runtime.rs`      | 删除 process_messages，保留 create_session |
| `crates/core/src/llm/mod.rs`                | 更新 re-exports                            |
| `crates/core/src/context.rs`                | 替换 llm → runtime                         |
| `crates/core/src/cron/mod.rs`               | 使用 runtime                               |
| `crates/core/src/social/telegram_bot.rs`    | 使用 runtime                               |
| `crates/core/src/social/adapter_handler.rs` | 使用 runtime                               |
| `crates/web/src/chat.rs`                    | 使用 runtime                               |
| `src-tauri/src/main.rs`                     | 使用 runtime                               |
| `crates/agent/src/session.rs`               | 删除 invoke_agent                          |

## 验证

1. `cargo check` 编译通过
2. cron 决策流程：runtime.create_session → subscribe → send → 收集 DecisionOutput
3. web SSE：runtime.create_session(persist=true) → subscribe → send → 事件流推送
4. telegram 对话：runtime.create_session → subscribe → send → 收集文本回复
5. doctor 检查：仍用 LlmClient.chat()（不走 runtime）
