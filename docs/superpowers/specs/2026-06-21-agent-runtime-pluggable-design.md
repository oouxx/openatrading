# Agent Runtime 可插拔抽象

**状态：** 待决策（尚未确定是否实施）
**日期：** 2026-06-21
**关联：** `crates/agent`、`crates/provider`、`crates/core`、`crates/web`、`crates/tui`、`crates/cli`

> 本文档是**架构决策文档**，目的在于让维护者判断"是否值得改动"，而非可直接执行的实现计划。若决定推进，再据此进入实现计划阶段。

---

## 1. 背景与现状

### 1.1 当前的两套抽象

代码库里存在两层都被称作 "LLM/agent" 的东西：

**第一层 —— agent loop（`crates/agent/src/runner.rs`）**
封装 `pi-agent-core` 的 `Agent::process`。这是被 web 的 `ai_chat`、tui 聊天、以及 `AgentSession`/`agent_loop`（cron/决策路径）使用的运行时。语义丰富：`AgentMessage` 历史、`AgentEvent` 渐进式事件（TextDelta / ThinkingDelta / ToolCallStart/Delta/End / MessageEnd / AgentEnd）、steering、follow_up、tool 执行循环。其 LLM 契约是 `StreamFn → Future<StreamResponse>`，`StreamResponse = Box<dyn Stream<Item = AssistantMessageEvent>>`。

**第二层 —— LLM provider（`crates/provider/src/llm.rs` 的 `LlmClient`）**
封装 `pi-ai` 的 `stream::stream`/`complete`。提供简单的 `chat()` / `chat_stream_with_messages()`（被 `ai_replay_analyze` 和 doctor 的 `check_llm` 使用），以及 `create_stream()`（第一层的 `StreamFn` 会调用它）。这里有一个 `ChatStreamProvider` trait，但它**只覆盖简单的文本流路径**——第一层（agent loop）没有抽象，直接依赖 `LlmClient` + pi-agent-core。

### 1.2 关键耦合点

`pi-agent-core` 的整个接口都与 `pi-ai` 的类型**绑定**：`AgentMessage`、`ContentBlock`、`AssistantMessageEvent`、`Context`、`Model`、`Tool`、`Usage`、`StopReason`。`StreamFn` 产出 `AssistantMessageEvent`；`ConvertToLlmFn` 返回 `Vec<pi_ai::Message>`。

upper layer 对这些类型的依赖实测范围：

- `AgentEvent` 在 web/tui/core 被 match 的变体：5 个（MessageUpdate / MessageEnd / AgentEnd / ToolExecutionStart / ToolExecutionEnd）
- `AssistantMessageEvent` 被 match 的变体：6 个（TextDelta / ThinkingDelta / ToolCallDelta / ToolCallEnd / Error 等）
- `ContentBlock` 被 match 的变体：4 个（Text / Image / text() 构造 / ToolCall）
- `pi_ai_types` 在 upper layer（非 mod.rs）的 import 点：9 处

### 1.3 现存半成品 `rig.rs`

`crates/provider/src/rig.rs` 是一个未完成的 rig 适配尝试，存在三个缺口：

1. **非流式**：`chat_stream_with_messages` 把单次回复用 `futures::stream::once` 包装，并非真流式。
2. **丢历史**：只取最后一条 user 消息发出，丢弃对话历史。
3. **无 tool 事件**：只实现 `ChatStreamProvider`（第二层简单路径），未实现 agent loop 所需的 `StreamFn`/`create_stream`，因此实际上无法驱动 web/tui/决策路径。

### 1.4 后端真实驱动模型（关键事实）

三个目标后端的事件驱动模型各不相同，这是抽象设计的核心约束：

| 后端 | 驱动模型 | 事件类型 | 事件颗粒 |
|---|---|---|---|
| **pi-agent** | **push**：`subscribe(listener)` 回调 | `AgentEvent { AssistantMessageEvent }` 两层嵌套 | 细粒度增量：TextDelta / ThinkingDelta / ToolCallStart / Delta / End |
| **rig** | **pull**：`stream_prompt().into_future()` → `Stream<MultiTurnStreamItem>` | `MultiTurnStreamItem` | 混合：StreamAssistantItem（文本/tool-call 增量）/ CompletionCall（usage） |
| **adk-rust** | **pull**：`Runner::run() -> EventStream`（`Stream<Item=Result<Event>>`） | `Event`（struct，非 enum） | **粗粒度**：一个 Event = 一整轮 LlmResponse 快照（含 Content/Part），无 delta 增量；附带 state_delta / transfer_to_agent / tool_confirmation |

> adk 的事件是"一整轮 LlmResponse 的快照"，不是流式增量——这是其后端固有限制，任何适配都无法凭空造出它没有的增量。

---

## 2. 目标与非目标

### 目标

- 将 agent runtime 抽象为可插拔层，通过 feature flag + 运行时配置在 `pi-agent` / `rig` / `adk-rust` 间切换。
- 切换 runtime **无需重新编译**：运行时分发，同一二进制可支持多个后端。
- upper layer（web / tui / cli / core）**不再直接依赖** `pi_ai_types`，仅依赖规范类型层。
- 抽象遵循"最小公共 trait + 各后端暴露专属能力"原则：公共层只定义所有后端都具备的能力；steering / continue_run / 中断 / thinking 等 pi 独有能力作为**借用**而非强制接口。
- 修复现存 `rig.rs` 半成品（非流式 / 丢历史 / 无 tool 事件）。

### 非目标

- 不重写 pi-agent-core 的 agent loop 本身；pi 路径行为应保持不变。
- 不追求三后端能力完全对等（rig/adk 不具备的能力不伪造，诚实降级）。
- v1 不把 adk 的 state_delta / transfer_to_agent / tool_confirmation 等独有语义纳入公共层（归专属借用）。

---

## 3. 设计

### 3.1 规范类型层（canonical types）

在 `openatrading-agent` 新增一组最小规范类型，upper layer 只 import 这些。

**最小原则**：只纳入 upper layer 实际使用 + agent loop 必需的概念。pi 独有的 `BashExecution` / `Custom` / `BranchSummary` / `CompactionSummary` 不进规范层，留在 pi-adapter 内部。

```rust
// crates/agent/src/runtime/types.rs

pub enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String },
    ToolCall { id: String, name: String, arguments: serde_json::Value },
    Image { data: String, media_type: String },
}

pub struct Usage { pub input: u64, pub output: u64, pub cache_read: u64, pub cache_write: u64, pub total: u64 }

pub enum StopReason { Stop, Length, ToolUse, ContentFilter, Error }

/// 只保留 upper layer 实际使用的三种角色。
pub enum AgentMessage {
    User { content: Vec<ContentBlock>, timestamp: i64 },
    Assistant { content: Vec<ContentBlock>, model: String, usage: Usage,
                stop_reason: Option<StopReason>, error_message: Option<String>, timestamp: i64 },
    ToolResult { tool_call_id: String, tool_name: String, content: Vec<ContentBlock>,
                 is_error: bool, timestamp: i64 },
}

pub struct ToolResult { pub content: Vec<ContentBlock>, pub details: serde_json::Value, pub terminate: Option<bool> }

/// 一个 tool call 的标识 + 参数（用于 RuntimeEvent::ToolCallStart/Delta/End）。
pub struct ToolCall { pub id: String, pub name: String, pub arguments: serde_json::Value }

/// 跨后端工具 trait。core 构建 Arc<dyn Tool>，各 adapter 各自包装。
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> &serde_json::Value;
    async fn execute(&self, arguments: serde_json::Value) -> ToolResult;
}
pub type DynTool = Arc<dyn Tool>;
```

**事件流刻意拍平**：pi 现在是两层 `AgentEvent { AssistantMessageEvent }`，upper layer 要嵌套 match。规范层改为单层 `RuntimeEvent`，adapter 直接 yield、consumer 直接 match——对 web/tui 的 SSE 代码是简化：

```rust
pub enum RuntimeEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolCallStart(ToolCall),
    ToolCallDelta { tool_call_id: String, arguments_delta: String },
    ToolCallEnd(ToolCall),
    ToolExecutionStart { tool_call_id: String, name: String },
    ToolExecutionEnd { tool_call_id: String, is_error: bool },
    TurnEnd(AgentMessage),
    RunEnd { messages: Vec<AgentMessage>, aborted: bool },
    Error(RuntimeError),
}
```

**有意的取舍**：

- 不设规范 `Model` struct——factory 直接吃 `(provider, model_id)` 字符串，各 adapter 自己做 model 解析（pi 走 `get_model` 注册表，rig 走 per-provider client 构造，adk 走其 model builder）。避免把 pi 的 `Model` 字段集强加给 rig/adk。
- `on_payload` / `on_response` 观测回调不进公共层——production 路径没接，若某后端调试需要，作为专属扩展后续再加。
- `ToolCallDelta` 可缺失——rig/adk 若不流式吐 tool 参数增量，adapter 只发 `ToolCallEnd`，consumer 容忍 absence。

### 3.2 公共接口：双层枚举 + 最小公共方法

采用双层枚举（镜像今天的 `LlmClient → Agent`），公共方法克制到最小：`Runtime` 暴露 `build` + `new_session`，`Session` 暴露 `subscribe` + `process`。专属能力由各后端具体类型暴露、业务代码 `if let` 借用。

```rust
// crates/agent/src/runtime/mod.rs

/// 长生命周期 client（替代 LlmClient）。config.toml 的 runtime 字段决定变体。
#[derive(Clone)]
pub enum Runtime {
    #[cfg(feature = "runtime-pi")]  Pi(Arc<PiClient>),
    #[cfg(feature = "runtime-rig")] Rig(Arc<RigClient>),
    #[cfg(feature = "runtime-adk")] Adk(Arc<AdkClient>),
}

impl Runtime {
    pub fn build(cfg: &RuntimeConfig) -> Result<Self>;          // 运行时分派
    pub fn new_session(&self, system_prompt: String, tools: Vec<DynTool>) -> Session;
}

/// per-chat 会话（替代 pi Agent）。不可跨 chat 复用。
#[derive(Clone)]
pub enum Session {
    #[cfg(feature = "runtime-pi")]  Pi(Arc<PiSession>),
    #[cfg(feature = "runtime-rig")] Rig(Arc<RigSession>),
    #[cfg(feature = "runtime-adk")] Adk(Arc<AdkSession>),
}

impl Session {
    pub async fn subscribe(&self, listener: AgentEventListener) -> UnsubscribeHandle;
    pub async fn process(&self, messages: Vec<AgentMessage>) -> Result<Vec<AgentMessage>, RuntimeError>;
}

pub type AgentEventListener = Arc<
    dyn Fn(RuntimeEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
>;

pub struct UnsubscribeHandle { /* backend-specific private token */ }
impl UnsubscribeHandle { pub async fn unsubscribe(self) { /* delegate */ } }
```

**设计要点**：

- `subscribe` 是**唯一的公共事件入口**（push 模型）。rig / adk 原生是 pull，各自在 adapter 内部做一次 pull→push 桥接。
- `process` 返回 `Vec<AgentMessage>`：web/tui/cli 都要从结果取 final_text / tool_output / usage，这是跨后端共同需求，属"基础功能"。
- `abort` 不进公共层（实测无人调用），留在 pi 具体类型上。
- delegation 是少量 match arm，机械、一次性。`Arc` 内嵌让 `Clone` 廉价，web 的 `agent_clone = Arc::clone` 模式原样保留。

### 3.3 专属能力的暴露与"按需借用"

专属能力 = 各后端具体类型上的真方法，**不进枚举、不逼对端实现**：

```rust
// pi 独有：中途插队 / 续跑 / 中断 / thinking。rig/adk 没有 → 不实现。
// 注意 ThinkingLevel 不在规范类型层（§3.1），它是 pi 专属类型，
// 随 PiSession 一起暴露、由借用方自行处理。
impl PiSession {
    pub async fn steer(&self, msg: AgentMessage);
    pub async fn follow_up(&self, msg: AgentMessage);
    pub async fn continue_run(&self) -> Result<Vec<AgentMessage>, RuntimeError>;
    pub async fn abort(&self);
    pub async fn set_thinking_level(&self, level: pi_agent_core::ThinkingLevel);
}

// pi / adk / rig 各自的构造期旋钮：各自 builder，形状不同。
impl PiClient  { pub fn builder() -> PiClientBuilder; }
impl RigClient { pub fn builder() -> RigClientBuilder; }
impl AdkClient { pub fn builder() -> AdkClientBuilder; }
```

**业务代码"按需借用"**：`if let` 即借用，编译期类型安全，rig/adk 上自然落空：

```rust
let session = ctx.runtime.new_session(system_prompt, tools);
let h = session.subscribe(listener).await;
session.process(messages).await?;            // 公共能力：所有 runtime 都有

if let Session::Pi(pi) = &session {          // 借用 pi 专属能力
    pi.steer(interrupt_msg).await;
}
```

`if let Session::Pi` 就是"借用"——编译器知道 `pi: &Arc<PiSession>`，steer 是真方法；rig/adk 从不被迫实现 steer，也不需要 `Unsupported` 哑返回。这正是"不逼所有 runtime 实现同一份接口"。

**一次性问答/流式 = 自由函数**（doctor 的 `chat()`、replay_analyze 的流式本质是"无工具单轮"= `new_session(无工具) + process`）：

```rust
pub async fn chat_one(rt: &Runtime, prompt: &str) -> Result<String, RuntimeError>;
pub async fn stream_one(rt: &Runtime, messages: Vec<AgentMessage>) -> Result<StreamResult, RuntimeError>;
```

### 3.4 三 adapter 桥接策略

公共层是 push（subscribe + listener）。三后端各自把非标准事件流桥接成统一 push：

**pi-adapter：零桥接**
pi 原生就是 push（`subscribe` 回调）。adapter 只做事件拍平：pi 两层 `AgentEvent { AssistantMessageEvent }` → 单层 `RuntimeEvent`。pi 路径行为不变。

**rig-adapter：pull→push 桥接**
rig 是 pull（`StreamingPromptRequest::into_future()` → `Stream<MultiTurnStreamItem>`）。adapter 在 `process` 内 spawn 驱动 task，poll rig 流，把每个 `MultiTurnStreamItem` 翻译成 `RuntimeEvent` 推 listener：

- `StreamAssistantItem(Text)` → `TextDelta`
- `StreamAssistantItem(ToolCall)` → `ToolCallEnd`（rig 无 Start/Delta 细分，只发 End）
- `CompletionCall` → 累积 usage
- 流末尾 → `RunEnd { messages, aborted: false }`

历史由 `RigSession` 自持（`Vec<rig::Message>` + `with_history` 回喂），封在 adapter 内。

**adk-adapter：粗粒度 Event → 规范增量 的语义映射**
adk 是 pull（`Runner::run() -> EventStream`），且 `Event` 是一整轮快照而非增量。adapter poll adk 流，把每个 `Event.llm_response.content.parts` 逐 Part 映射：

- `Part::Text(t)` → `TextDelta`（adk 一轮的完整文本作为一个块推出）
- `Part::ToolCall(tc)` → `ToolCallEnd`
- 流末尾 → `RunEnd`

**adk 的语义代价（如实标注）：**

- **无真·增量**：`Event` 是一轮完整 LlmResponse，adapter 把文本作为整块 `TextDelta` 推出——consumer 看到的是"一整段一次性到达"而非逐字流式。web/tui 的打字机效果在 adk 后端退化成"整段出现"。这是 adk 事件模型固有的，adapter 无法凭空造增量。
- **无 ToolCallStart/Delta**：adk tool 调用成块，adapter 只发 `ToolCallEnd`。
- `state_delta` / `transfer_to_agent` / `tool_confirmation` 等 adk 独有语义不进 `RuntimeEvent`，归 adk 专属借用。

三者对照：

```
pi  : push 原生  →  拍平两层 Event         →  RuntimeEvent（细粒度增量，行为不变）
rig : pull 流    →  poll→emit + RunEnd     →  RuntimeEvent（混合增量）
adk : pull 流    →  poll→emit + 粗粒度拆解  →  RuntimeEvent（退化成块状，无真增量）
```

### 3.5 依赖、feature 与配置

**`crates/agent/Cargo.toml`**：

```toml
[features]
default = ["runtime-pi"]
runtime-pi  = ["dep:pi-agent-core"]
runtime-rig = ["dep:rig-core"]
runtime-adk = ["dep:adk-rust"]

[dependencies]
pi-agent-core = { git = "https://github.com/oouxx/pi-rs", tag = "v1.78.7", optional = true }
rig-core      = { version = "0.39", optional = true }
adk-rust      = { version = "1", optional = true }   # zavora-ai/adk-rust
```

模块 cfg：`runtime/mod.rs` 无条件导出公共类型；各 adapter 模块用 `#[cfg(feature = "runtime-xx")]`。枚举变体同理。

**运行时分发**（`Runtime::build`，非 recompile）：读 `cfg.runtime`，match 字符串构造对应变体。`runtime = "rig"` 但编译时未开 `runtime-rig` feature → 返回 `RuntimeError::BackendNotCompiled("rig")`（明确报错，不静默）。

**`config.toml`** —— `LlmTomlConfig` 加一字段：

```toml
[llm]
runtime = "pi"   # "pi" | "rig" | "adk"，默认 pi
provider = "anthropic"
model = "claude-sonnet-4-6"
```

**`AppContext`**：`pub llm: LlmClient` → `pub runtime: Runtime`。`AppContext::build` 签名从 `(provider, model)` 改为收 `&RuntimeConfig`，内部 `Runtime::build(cfg)`。所有下游 `ctx.llm` → `ctx.runtime`。

**`provider` crate**：`llm.rs` 的简单路径（`chat` / `chat_stream_with_messages` / `chat_simple`）下沉为 `agent` crate 的自由函数 `chat_one` / `stream_one`，`LlmClient` 退役。`rig.rs` 现有半成品删除，rig 逻辑整体进 `agent/src/runtime/rig/`。

---

## 4. 改动范围

| 层 | crate | 改动 |
|---|---|---|
| 规范类型 + Runtime/Session 枚举 + 公共 trait | `agent`（新增 `src/runtime/`） | 新增类型层、枚举、三 adapter 骨架 |
| pi 适配 | `agent/src/runtime/pi/` | 从 `runner.rs` 搬迁 + 事件拍平 |
| rig 适配 | `agent/src/runtime/rig/` | 新写（补三缺口 + pull→push 桥接） |
| adk 适配 | `agent/src/runtime/adk/` | 新写（pull→push + 粗粒度拆解） |
| 工具桥接 | `agent/src/runtime/tool_bridge.rs` | canonical `Tool` ↔ pi `DynTool` / rig `ToolDyn` / adk `#[tool]` |
| 切断 pi_ai_types 依赖 | `core`、`web`、`tui`、`cli` | 9 处 import 改为规范类型；match 表达式重写 |
| 配置 | `cli/src/cli/config.rs` | `LlmTomlConfig` 加 `runtime` 字段 |
| AppContext | `core/src/context.rs` | `llm` → `runtime` |
| 退役 | `provider/src/llm.rs`、`provider/src/rig.rs` | 简单路径下沉到 `agent`，半成品删除 |

**upper layer 实际改动点（已实测）**：

- `web/src/chat.rs`、`web/src/handlers.rs`：`AgentEvent` 嵌套 match → 单层 `RuntimeEvent`；`ai_replay_analyze` 的 `chat_stream_with_messages` → `stream_one`
- `tui/src/chat.rs`：同上；手写的 subscribe+mpsc 桥接删除（由 adapter 承担）
- `core/src/llm/mod.rs`、`context.rs`、`session.rs`、`mcp_server.rs`、`mcp_tools.rs`：`pi_ai_types` import + `LlmClient` → 规范类型 + `Runtime`
- `cli/src/cli/doctor/check_llm.rs`：`LlmClient::from_config` + `chat()` → `Runtime::build` + `chat_one`
- `crates/core/tests/*_integration_test.rs`：`LlmClient::with_config` → `Runtime::build`

---

## 5. 成本收益分析

### 收益

1. **运行时可切后端**：同一二进制通过 `config.toml` 的 `runtime` 字段在 pi / rig / adk 间切换，无需重新编译。为对比、回退、provider 受限场景提供出路。
2. **解耦 pi-ai 类型**：upper layer 不再 import `pi_ai_types`，pi-rs 升级或后端替换不再波及 web/tui/cli。
3. **消除现存半成品**：`rig.rs` 三缺口（非流式 / 丢历史 / 无 tool）修复，rig 成为真实可用后端。
4. **简化 upper layer**：事件流拍平 + 桥接集中到 adapter，web/tui 各自手写的 subscribe+mpsc 桥接删除（每处约 10 行）。
5. **诚实的能力边界**：pi 独有能力不逼 rig/adk 伪造，后端不支持的特性如实降级而非哑实现。

### 成本

1. **改动面广**：跨 6 个 crate，9 处 import 重写，多处 match 表达式重写。属中等规模重构。
2. **新增规范类型层**：约 10–12 个 enum/struct，增加一层间接。类型转换（pi-ai ↔ canonical、rig ↔ canonical、adk ↔ canonical）在 adapter 内承担。
3. **rig/adk 适配工作量**：rig 需补三缺口 + 写桥接；adk 需处理粗粒度→规范的语义映射 + 工具桥接（adk 用 `#[tool]` 宏，与 canonical `Tool` trait 的桥接需设计）。
4. **adk 功能退化**：adk 后端下 web/tui 打字机效果退化为整段出现，无真增量——这是后端固有限制，需在文档/UI 层如实标注。
5. **pi-agent-core 仍为默认主力**：抽象主要收益是"可选其他后端"，而非 pi 路径本身的改进。若短期内无真实使用 rig/adk 的需求，抽象的 ROI 取决于未来计划。
6. **测试成本**：三后端的 tool 桥接、事件映射、历史管理都需要各自的集成测试。

---

## 6. 待决点

以下两点已在讨论中提出，尚未拍板，影响 tool 桥接与迁移计划：

1. **adk 无真增量的退化是否接受**：adk 后端下 web/tui 打字机效果变成"整段出现"。建议如实接受并在 spec/UI 标注，而非在 adapter 伪造假增量。
2. **adk 的 `state_delta` / `transfer_to_agent` / `tool_confirmation` 是否 v1 进规范层**：建议不进（归 adk 专属借用），等出现真实 consumer 再加。

---

## 7. 后续展开（待"决定推进"后再细化）

若决定推进，仍需展开两部分：

- **canonical `Tool` 与三后端 tool 桥接**：canonical `DynTool` ↔ pi `DynTool`（直接结构对应）/ rig `ToolDyn`（实现 rig trait）/ adk `#[tool]` 宏（宏与动态 trait 的桥接最需设计）的形状。
- **迁移顺序 + 测试矩阵**：分阶段迁移（pi 路径先迁并保持行为不变 → rig → adk），每阶段的功能测试与回归点。
