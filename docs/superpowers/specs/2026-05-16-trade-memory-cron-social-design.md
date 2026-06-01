# Trade + Memory + Cron + Social + Gateway Design

## Overview

将 openatrading 升级为持续运行的交易 agent。新增 trade（持仓）、memory（SQLite 历史）、cron（定时调度）、social（Telegram 交互）、gateway（session 管理）五大模块。

## 1. Crate 架构

```
crates/
├── core/                        # 共享库（所有业务逻辑）
│   ├── src/
│   │   ├── trade/
│   │   │   ├── mod.rs           # Position, TradeLog, TradeManager trait
│   │   │   └── sqlite.rs        # SQLite 实现
│   │   ├── memory/
│   │   │   ├── mod.rs           # MarketSnapshot, MemoryStore trait
│   │   │   └── sqlite.rs        # SQL 建表、10日清理
│   │   ├── cron/
│   │   │   └── mod.rs           # CollectionTask, DecisionTask, scheduler
│   │   ├── social/
│   │   │   ├── mod.rs           # NotificationSender trait
│   │   │   └── telegram.rs      # teloxide bot 实现
│   │   ├── gateway/
│   │   │   ├── mod.rs           # Gateway — 统一管理 session 生命周期
│   │   │   ├── cron_session.rs  # cron scheduler session
│   │   │   └── social_session.rs# telegram bot session
│   │   ├── roles/               # 已有
│   │   ├── llm/                 # 已有 + function calling
│   │   └── ...
│   └── Cargo.toml               # teloxide + tokio-cron-scheduler + rusqlite
├── cli/                         # CLI 二进制
│   └── src/cli/
│       ├── serve.rs             # serve start/stop/status 命令
│       └── trade_cmd.rs         # 持仓 CRUD 命令
└── web/                         # Web 二进制（已有，独立部署）
```

**进程模型：** Gateway（CLI serve）单进程运行 cron + telegram sessions，Web 独立进程部署。Gateway 内部通过 Arc 共享 memory/trade 实例，无需进程间通信。

## 2. config.toml 扩展

```toml
# ── Trending Agent 配置 ──────────────────────────────────
# 优先级：项目根 config.toml > 项目根/.atrading/config.toml > ~/.atrading/config.toml
# 同名角色：高优先级完整覆盖低优先级

default_role = "zarili"

# ── 后台服务 ─────────────────────────────────────────────
[service]
db_path = "data.db"                    # 相对 .atrading/，SQLite 数据库

# ── Cron 调度 ────────────────────────────────────────────
[cron]
enabled = true
collection_schedule = "*/30 9-15 * * 1-5"   # 采集：周一至五 9:00-15:00 每半小时
decision_schedule = "0 0 * * 1-5"           # 决策：周一至五 晚上 00:00

# ── 社交平台 ─────────────────────────────────────────────
[social.telegram]
enabled = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id = "${TELEGRAM_CHAT_ID}"
notify_on_collect = true
notify_on_decision = true
notify_on_trade = true

# ── 角色定义（已有）───────────────────────────────────────
[[roles]]
name = "zarili"
display_name = "zarili量价派"
dir = "strategies/zarili"
merge_mode = "supplement"
```

## 3. Trade 模块

### `crates/core/src/trade/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub role_name: String,          // "zarili"
    pub symbol: String,             // "000001"
    pub stock_name: String,         // "平安银行"
    pub entry_date: NaiveDate,
    pub cost_price: f64,
    pub shares: i64,               // 持仓股数
    pub current_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub target_price: Option<f64>,
    pub entry_reason: String,
    pub sector: String,
    pub position_pct: f64,          // 仓位占比 0.0-1.0
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLog {
    pub id: i64,
    pub position_id: i64,
    pub action: TradeAction,        // Open | Add | Reduce | Close
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: NaiveDateTime,
}

pub trait TradeManager: Send + Sync {
    fn list_positions(&self, role_name: &str) -> Result<Vec<Position>>;
    fn add_position(&self, pos: Position) -> Result<i64>;
    fn update_position(&self, pos: Position) -> Result<()>;
    fn close_position(&self, id: i64, price: f64, reason: &str) -> Result<()>;
    fn get_trade_logs(&self, position_id: i64) -> Result<Vec<TradeLog>>;
    fn format_positions(&self, role_name: &str) -> Result<String>; // 人类可读
}
```

**SQLite 表：** positions + trade_logs

**CLI 命令：**
```bash
openatrading trade add --role zarili --symbol 000001 --price 10.5 --shares 1000
openatrading trade list [--role zarili]
openatrading trade close <id> --price 11.2
```

## 4. Memory 模块

### `crates/core/src/memory/mod.rs`

```rust
pub struct MarketSnapshot {
    pub id: i64,
    pub captured_at: NaiveDateTime,
    pub data_source: String,        // "xuangubao" / "jiuyan" / "cls" / "sentiment"
    pub raw_json: String,           // 完整 API 响应 JSON
}

pub trait MemoryStore: Send + Sync {
    fn save_snapshot(&self, source: &str, json: &str) -> Result<i64>;
    fn query_recent(&self, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn query_by_source(&self, source: &str, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn cleanup_old(&self) -> Result<()>;          // 保留最近10天
    fn get_historical_context(&self, days: u32) -> Result<String>; // LLM 可读摘要
}
```

**SQLite 表：** market_snapshots (id, captured_at, data_source, raw_json)

**数据保留策略：** 自动清理超过 10 天的记录，每次写入后触发 cleanup。

## 5. Cron 模块

### `crates/core/src/cron/mod.rs`

使用 `tokio-cron-scheduler` 库，按 cron 表达式触发。

```rust
pub struct CronConfig {
    pub enabled: bool,
    pub collection_schedule: String,  // "*/30 9-15 * * 1-5"
    pub decision_schedule: String,    // "0 0 * * 1-5"
}

pub struct CollectSummary {
    pub timestamp: NaiveDateTime,
    pub sources: Vec<String>,
    pub snapshot_count: u32,
}

pub struct Decision {
    pub role_name: String,
    pub summary: String,
    pub trade_suggestions: Vec<TradeSuggestion>,
}

pub async fn start_scheduler(
    config: CronConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    llm: Arc<LlmClient>,
    social: Arc<dyn NotificationSender>,
) -> Result<JobScheduler>;
```

**采集 Job：** 调所有数据源 API → save_snapshot → notify_collect  
**决策 Job：** memory.get_historical + trade.list_positions → build_prompt → LLM(function calling) → apply_suggestions → notify_decision

## 6. Social 模块

### `crates/core/src/social/mod.rs`

```rust
pub trait NotificationSender: Send + Sync {
    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()>;
    async fn notify_decision(&self, decision: &Decision) -> Result<()>;
    async fn notify_trade(&self, log: &TradeLog) -> Result<()>;
}
```

### `crates/core/src/social/telegram.rs`

集成 `teloxide` 库（optional feature），实现 Telegram bot。

**能力：**
1. **通知推送** — 采集摘要、决策结果、持仓变动
2. **对话交互** — 用户消息 → 构建 prompt（策略+持仓+今日数据）→ LLM 流式回复
3. **指令支持** — `/status` 持仓概览、`/positions` 详细持仓、`/trigger` 手动触发决策

**消息处理流程：**
```
用户消息 → teloxide Dispatcher → MessageHandler
  → 构建完整 prompt（策略 + 持仓 + 市场数据）
  → LLM 流式回复
  → 回送 Telegram
```

### `crates/core/Cargo.toml`

```toml
[features]
telegram = ["dep:teloxide"]

[dependencies]
teloxide = { version = "0.15", optional = true }
```

核心社交逻辑和其他模块不依赖 `teloxide`，只有 feature 启用时才加载。

## 7. Gateway 模块

### `crates/core/src/gateway/mod.rs`

统一管理所有 session 生命周期。

```rust
pub struct Gateway {
    config: AppConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    llm: Arc<LlmClient>,
}

impl Gateway {
    pub async fn start(config: AppConfig) -> Result<()> {
        // 初始化共享资源
        // 启动 cron session
        // 启动 telegram session
        // 阻塞等待
    }

    pub async fn stop(&self) -> Result<()>;
    pub fn status(&self) -> GatewayStatus;
}
```

**CLI 命令：**
```bash
openatrading serve start     # 启动 Gateway（cron + telegram）
openatrading serve stop      # 停止
openatrading serve status    # 查看状态
```

Gateway 只管理 cron + telegram sessions。Web server 独立部署。

## 8. build_prompt 变更

### `crates/core/src/roles/prompt.rs`

```rust
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // OpenAI function calling schema
}

pub fn build_prompt(
    strategy_text: &str,
    market_data_json: &str,
    position_context: &str,        // 新：持仓信息文本
    available_tools: &[ToolDef],   // 新：function calling 工具
    display_name: &str,
) -> Prompt;
```

**LLM function calling 工具：**
```json
[{
    "name": "query_market_memory",
    "description": "查询近N日市场历史数据",
    "parameters": {
        "type": "object",
        "properties": {
            "days": { "type": "integer", "default": 5 },
            "source": { "type": "string", "enum": ["xuangubao", "jiuyan", "cls", "all"] }
        }
    }
}]
```

**system prompt 模板：**
```
[strategy_text]

## 当前持仓
[position_context]

## 可用工具
[available_tools]

## 核心原则
- 红涨绿跌（A股规则）
- 严格遵守上述策略体系进行交易决策
- 结合持仓情况，给出明确的加仓、减仓、持有、清仓建议及理由
- 用数据和逻辑说话，不预测具体涨跌点位
```

**user message 模板：**
```
## 今日市场数据
```json
[market_data_json]
```

请以【{name}】身份，根据策略体系分析当前市场状态和持仓情况，给出明确的持仓调整建议。
需要历史数据对比时可调用 query_market_memory。
```

**position_context 示例：**
```
- #1 000001 平安银行 | 成本10.5 | 现价11.2 | +6.7% | 1000股 | 银行 | 01-15买入
- #2 600519 贵州茅台 | 成本1680 | 现价1650 | -1.8% | 100股 | 白酒 | 01-10买入
```

## 9. 完整数据流

```
┌─ Cron Session ─────────────────────────────────────┐
│  每30分钟(交易时间):                                 │
│    各API采集 → MarketSnapshot → memory.save()      │
│    → telegram.notify_collect()                     │
│                                                     │
│  每天 00:00(周一至五):                                │
│    memory.get_historical() + trade.list_positions() │
│    → build_prompt(strategy, data, positions, tools) │
│    → LLM function calling (可查历史)                 │
│    → Decision(持仓建议)                               │
│    → trade.apply_suggestion()                      │
│    → telegram.notify_decision()                    │
│    → telegram.notify_trade()                       │
└────────────────────────────────────────────────────┘

┌─ Telegram Session ────────────────────────────────┐
│  用户消息 → 构建 prompt(策略+持仓+今日数据)           │
│  → LLM 流式回复（可 function call 查历史）            │
│  → 支持 /status /positions /trigger 指令            │
└────────────────────────────────────────────────────┘

┌─ CLI ─────────────────────────────────────────────┐
│  openatrading serve start/stop/status            │
│  openatrading trade add/list/close               │
│  openatrading --role --ai (单次分析)               │
│  openatrading init                               │
└────────────────────────────────────────────────────┘

┌─ Web (独立部署) ───────────────────────────────────┐
│  /api/ai/chat (已有逻辑)                            │
│  前端 Vercel 静态部署                               │
└────────────────────────────────────────────────────┘
```

**共享资源：** Gateway 内 memory、trade、llm 均为 Arc 共享，cron 和 social session 通过 Arc 访问同一 SQLite 实例。

## 10. Implementation Order

1. **trade 模块** — Position/TradeLog 模型 + SQLite 建表 + TradeManager trait + CLI 命令
2. **memory 模块** — MarketSnapshot + SQLite + MemoryStore trait + 10日清理
3. **build_prompt 扩展** — 新增 position_context + ToolDef + function calling 支持
4. **cron 模块** — CronConfig + CollectionTask + DecisionTask + tokio-cron-scheduler
5. **social 模块** — NotificationSender trait + telegram.rs (teloxide)
6. **gateway 模块** — Gateway + cron_session + social_session
7. **CLI serve** — serve start/stop/status 命令
8. **config.toml** — service + cron + social section
