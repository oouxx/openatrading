# 方案：AgentSession 替换为 pi-coding-agent AgentSession

## 现状

### 使用点

| 位置 | 用途 | 当前调用 |
|------|------|---------|
| `cron/mod.rs:343` | 定时交易决策 | `AgentSession::new(&llm, &s).with_tools(t).process_decision(&msg)` |
| `telegram_bot.rs:500` | Telegram 聊天 | `AgentSession::new(&llm, &s).with_tools(t).process_chat(&msg)` |
| `adapter_handler.rs:157` | Social adapter 聊天 | `AgentSession::new(&llm, &s).process_chat(question)` |

### 当前 Tool 体系

- **agent crate**: `ExecuteFn`、`tool()`、`empty_schema()`、`str_schema()`、`str_arg()`、`text_result()`、`to_box_err()`
- **core crate**: 16 个市场数据工具 + `ToolRegistry` + `create_decision_tool()` + `build_tools()`

### pi-coding-agent 的组件

| 组件 | 说明 | 交易场景是否可用 |
|------|------|----------------|
| `AgentSession` | 完整 session 管理 + 事件驱动 loop | ⚠️ 工具列表写死在构造中，无法注入交易工具 |
| `SessionManager` | 会话持久化 | ✅ 可复用，readonly 模式 |
| `EventBusController` | 事件总线 | ✅ 可复用 |
| `ModelRegistry` | 模型注册表 | ✅ 可复用 |
| `system_prompt::build_system_prompt()` | 构建 system prompt | ✅ 通过 `custom_prompt` 覆盖 |
| `skills::load_skills()` | 加载 skill | ✅ 可独立使用 |
| `tools::create_coding_tools()` | 创建 coding 工具 | ❌ 交易场景不需要 |

---

## 方案

### 核心问题

pi-coding-agent 的 `AgentSession::new()` 在内部创建工具列表，**不接受外部注入**：

```rust
// pi-coding-agent AgentSession::new() 内部
let mut tool_list = tools::create_coding_tools(&options.cwd, ...);
let tools: Vec<Arc<DynTool>> = tool_list.into_iter().map(Arc::new).collect();
let initial_state = AgentState { tools, ... };
let agent = Agent::new(AgentOptions { initial_state, ... });
```

交易工具无法加进去。所以不能直接使用 `AgentSession::new()`。

### 架构

分两层：

```
┌──────────────────────────────────────────────────┐
│  AgentRuntime (生命周期管理)                        │
│                                                    │
│  持有:                                              │
│    - session_manager: SessionManager  (全局唯一)    │
│    - event_bus: EventBusController   (全局唯一)     │
│    - model_registry: ModelRegistry   (全局唯一)     │
│    - sessions: HashMap<SessionId, AgentSession>     │
│                                                    │
│  create(system_prompt, tools, skills) → SessionId  │
│  destroy(session_id)                                │
│  get(session_id) → &AgentSession                    │
│  compact(session_id)   ← 消息数超限时压缩           │
│                                                    │
│  stream(session_id, msg, tx) → AgentResult          │
│  run(session_id, msg) → AgentResult                 │
│  decision(session_id, msg) → DecisionOutput         │
└──────────────────────┬───────────────────────────┘
                       │ 内部持有
              ┌────────▼────────┐
              │  AgentSession   │  (pi-coding-agent 的 AgentSession)
              │                 │
              │  + coding tools │
              │  + trading tools│
              │  + skills       │
              │  + event bus    │
              └─────────────────┘
```

- **`AgentRuntime`** = 公开 API，管理 session 创建/复用/销毁/压缩
- **`AgentSession`** = pi-coding-agent 的 AgentSession（或自己组装的等价物），实际执行 agent loop

### 详细设计

#### AgentRuntime

```rust
// crates/core/src/llm/agent_runtime.rs

pub struct AgentRuntime {
    session_manager: SessionManager,    // 全局唯一
    event_bus: EventBusController,      // 全局唯一
    model_registry: ModelRegistry,      // 全局唯一
    sessions: Mutex<HashMap<String, AgentSession>>,
}

impl AgentRuntime {
    pub fn new(
        session_manager: SessionManager,
        event_bus: EventBusController,
        model_registry: ModelRegistry,
    ) -> Self { ... }

    // ── 生命周期 ────────────────────────────────

    /// 创建一个新的 agent session，返回 session_id
    pub fn create(
        &self,
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
    ) -> String { ... }

    /// 销毁一个 session
    pub fn destroy(&self, session_id: &str) { ... }

    /// 获取已有 session（不存在则创建）
    fn get_or_create(
        &self,
        id: &str,
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
    ) -> AgentSession { ... }

    /// 压缩 session 消息历史
    pub fn compact(&self, session_id: &str) { ... }

    // ── 执行 ─────────────────────────────────────

    pub async fn stream(
        &self,
        session_id: &str,
        user_message: &str,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<AgentResult> { ... }

    pub async fn run(
        &self,
        session_id: &str,
        user_message: &str,
    ) -> Result<AgentResult> { ... }

    pub async fn decision(
        &self,
        session_id: &str,
        user_message: &str,
    ) -> Result<DecisionOutput> { ... }
}
```

#### AgentSession（自己组装，不使用 pi-coding-agent 的 new()）

```rust
// AgentRuntime 内部管理，不对外暴露
struct AgentSession {
    agent: Arc<Agent>,
    created_at: Instant,
    message_count: AtomicUsize,
}

impl AgentSession {
    fn new(
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
        session_manager: &SessionManager,
        event_bus: &EventBusController,
        model_registry: &ModelRegistry,
    ) -> Self {
        // 1. 用 pi-coding-agent 的 system_prompt builder
        let prompt = system_prompt::build_system_prompt(&BuildSystemPromptOptions {
            cwd: cwd.clone(),
            custom_prompt: Some(system_prompt.to_string()),
            skills: Some(skills),
            ..Default::default()
        });

        // 2. 合并 coding tools + trading tools
        let mut all_tools: Vec<Arc<DynTool>> = tools::create_coding_tools(&cwd, None)
            .into_iter().map(Arc::new).collect();
        all_tools.extend(trading_tools);

        // 3. 构造 Agent
        let agent = Agent::new(AgentOptions {
            initial_state: Some(AgentState {
                system_prompt: prompt,
                model: llm.model.clone(),
                tools: all_tools,
                ..Default::default()
            }),
            stream_fn: Some(llm.to_stream_fn()),
            session_id: Some(session_manager.get_session_id().to_string()),
            ..Default::default()
        });

        Self { agent: Arc::new(agent), created_at: Instant::now(), message_count: AtomicUsize::new(0) }
    }
}
```

#### 统一执行核心

```rust
// AgentRuntime 中的核心执行方法，stream 和 run 共用
async fn execute(
    agent: Arc<Agent>,
    user_message: &str,
    listener: Option<impl Fn(AgentEvent) + Send + Sync>,
) -> Result<AgentResult> {
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);

    // 注册事件监听
    let subscribe_listener = Arc::new(
        move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move { let _ = tx.send(event).await; })
                as Pin<Box<dyn Future<Output = ()> + Send>>
        },
    );
    let _handle = agent.subscribe(subscribe_listener).await;

    // 启动 agent loop
    let messages = vec![AgentMessage::User {
        content: vec![ContentBlock::text(user_message)],
        timestamp: 0,
    }];
    let agent_clone = agent.clone();
    let join = tokio::spawn(async move { agent_clone.process(messages).await });

    // 收集结果
    let mut final_text = String::new();
    let mut tool_output = None;

    while let Some(event) = rx.recv().await {
        if let Some(ref cb) = listener { cb(event.clone()); }
        match &event {
            AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. }, ..
            } => final_text.push_str(delta),
            AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::Done { message, .. }, ..
            } => {
                for block in &message.content {
                    if let ContentBlock::ToolCall { arguments, .. } = block {
                        tool_output = Some(arguments.clone());
                    }
                }
                break;
            }
            AgentEvent::Error { .. } => break,
            _ => {}
        }
    }

    // 等待后台任务完成
    let _ = join.await;
    Ok(AgentResult { final_text, tool_output })
}
```

### 生命周期管理

| 场景 | session_id 策略 | 生命周期 |
|------|----------------|---------|
| Cron 决策 | `"cron:{role_name}"` | 每次 `decision()` 调用时创建，用完后保留（下次可能复用消息历史）或销毁 |
| Telegram 聊天 | `"telegram:{chat_id}"` | 按 chat_id 缓存，长期存活。消息数超限时 compact |
| Adapter handler | `"adapter:{chat_id}"` | 一次性，用完即销毁 |

```rust
// cron 使用示例
let session_id = format!("cron:{}", role.name);
let decision = runtime.decision(&session_id, &market_user_message).await?;
// session 保留在 runtime 中，下次 cron tick 可复用（有历史上下文）
// 或每次 tick 用不同 session_id 实现隔离

// Telegram 使用示例
let session_id = format!("telegram:{}", chat_id);
let (tx, mut rx) = mpsc::channel(256);
let result = runtime.stream(&session_id, &user_msg, tx).await?;

// 消息数超限时压缩
if runtime.message_count(&session_id) > MAX_HISTORY {
    runtime.compact(&session_id);
}
```

### 迁移调用点

| 调用点 | 当前 | 迁移后 |
|--------|------|--------|
| `cron/mod.rs:343` | `AgentSession::new(&llm, &s).with_tools(t).process_decision(&msg)` | `runtime.decision("cron:{role}", &msg).await?` |
| `telegram_bot.rs:500` | `AgentSession::new(&llm, &s).with_tools(t).process_chat(&msg)` | `runtime.stream("telegram:{chat}", &msg, tx).await?` |
| `adapter_handler.rs:157` | `AgentSession::new(&llm, &s).process_chat(question)` | `runtime.stream("adapter:{id}", question, tx).await?` |

### 清理

- 删除 `crates/core/src/llm/session.rs`
- 删除 `crates/agent/src/session.rs`
- 更新 `crates/core/src/llm/mod.rs` re-export

### 风险

1. **AgentRuntime 全局唯一**：在 `AppContext` 中初始化，所有模块共享。需要 `Mutex` 保护 `sessions` HashMap，但执行路径本身无锁（channel 通信）。
2. **Session 膨胀**：长期运行的 Telegram session 会累积消息。通过 `compact()` 定期压缩，或设置最大消息数后自动重建 agent。
3. **后台任务取消**：`execute()` 使用 `tokio::spawn`，如果调用方取消（如前端断开），后台 agent loop 可能仍在运行。需要 `CancellationToken` 或 `AbortHandle` 传递取消信号。
4. **session_id 冲突**：不同场景用不同前缀（`cron:`、`telegram:`、`adapter:`），天然隔离。
