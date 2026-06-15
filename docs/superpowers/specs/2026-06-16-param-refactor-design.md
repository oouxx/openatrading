# 函数参数重构设计

## 目标

将 6 个参数超过 5 的 Rust 函数重构为领域聚合的 struct 入参，满足 CLAUDE.md 中"单个函数参数不超过 3 个"的约束。

## 原则

- 按业务语义聚合参数，不搞大一统 struct
- 新 struct 放在使用它的模块内，跨模块需求时再提取
- 不改变函数行为，仅改签名和调用方

## 重构明细

### 1. CronContext — `run_decision` / `run_decision_for_role`

**新增 struct：**

```rust
// crates/core/src/cron/mod.rs

pub struct CronContext {
    pub memory: Arc<dyn MemoryStore>,
    pub trade: Arc<dyn TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    pub llm: LlmClient,
    pub api: ApiClient,
}
```

**设计决策：**
- `social` 用 `Arc<dyn>` 而非 `Box<dyn>`：cron 调度器多个闭包需要共享同一个 sender，`Arc` 支持跨闭包 clone。代码库现有模式也是 `Arc<dyn NotificationSender>`。
- 不包含 `tools`：tools 是业务层概念，不属于"基础设施"。`run_collection` 等其他 cron 函数不需要 tools，放进去会导致构造时填空值。`run_decision` 的 tools 作为独立参数传入。

**重构后签名：**

```rust
pub async fn run_decision(ctx: &CronContext, roles: &[RoleEntry], tools: &[Arc<DynTool>]) -> Result<()>

async fn run_decision_for_role(
    ctx: &CronContext,
    role: &RoleEntry,
    market_context: &str,
    tools: &[Arc<DynTool>],
) -> Result<()>
```

`ctx` 是上下文类参数不计入，`run_decision` 业务参数 2 个，`run_decision_for_role` 业务参数 3 个。

**调用方：**
- `cron/mod.rs:146` — 定时任务回调，直接构造 `CronContext { memory, trade, social, llm, api }`
- `social/telegram_bot.rs:455` — Telegram 命令触发
- `cli/mod.rs:279` — CLI 命令触发

### 2. PositionSnapshot — `validate_buy`

**新增 struct：**

```rust
// crates/core/src/trade/paper.rs

pub struct PositionSnapshot {
    pub same_symbol_value: f64,
    pub total_position_value: f64,
}
```

**重构后签名：**

```rust
pub fn validate_buy(
    &self,
    shares: i64,
    exec_price: f64,
    positions: &PositionSnapshot,
) -> Result<(), String>
```

**设计决策：** 字段保持裸 `f64`，与 `PaperBroker` 模块整体一致。newtype 改造（如 `Money(f64)`）是独立任务，不混入本次重构。

**调用方：**
- `trade/broker.rs:42` — 包成 `PositionSnapshot { same_symbol_value, total_position_value }`
- `trade/paper.rs` 内部测试

### 3. McpToolSpec — `create_tool`

**新增 struct：**

```rust
// crates/core/src/llm/mcp_tools.rs

pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub method: String,
}
```

**重构后签名：**

```rust
pub fn create_tool(&self, spec: McpToolSpec) -> Arc<DynTool>
```

**设计决策：** 字段用 `String`（owning）而非 `&str`。`McpToolSpec` 是一次性构造的消费型 struct，内部 `create_tool` 本来就在做 `.to_string()` 转入闭包，用 `String` 直接移动反而少一次分配。不带生命周期使用更灵活。

**调用方：**
- `mcp_tools.rs:265-273` — 唯一调用方

### 4. ChatArgs 复用 — `run_llm`

不复用新建 `LlmSession`，直接复用已有的 `ChatArgs`：

```rust
// crates/tui/src/chat.rs (已存在)

pub struct ChatArgs {
    pub llm: LlmClient,
    pub tools: Vec<Arc<DynTool>>,
    pub system_prompt: String,
}
```

**重构后签名：**

```rust
async fn run_llm(
    msg_tx: tokio::sync::mpsc::Sender<Msg>,
    args: ChatArgs,
    messages: Vec<serde_json::Value>,
)
```

**设计决策：** `ChatArgs` 与提议的 `LlmSession` 字段完全相同，是同一份数据在调用链上的两次表达，新建 struct 是纯粹重复。调用方 `chat.rs:139` 已有这三个变量的 clone，直接包成 `ChatArgs { llm, tools, system_prompt }` 即可。

### 5. CommunityQuery — `fetch_community`

**新增 struct：**

```rust
// crates/api/src/jiuyangongshe.rs

pub struct CommunityQuery {
    pub category_id: String,
    pub article_type: i32,
    /// 1-based page number
    pub page: i32,
    pub page_size: i32,
}
```

**重构后签名：**

```rust
pub async fn fetch_community(&self, query: CommunityQuery) -> Result<CommunityData>
```

**设计决策：** `page` 保持 1-based，与内部 `CommunityReq.start = (page - 1) * limit` 对齐，加注释明确。改为 0-based 会连带改调用方逻辑，属于行为变更。

**调用方：**
- `agents/sentiment.rs:37`

## 不在本次范围

- `PaperBroker` 模块的裸 `f64` newtype 改造
- 其他 cron 函数（`run_collection` 等）的 `CronContext` 适配——可后续渐进迁移
