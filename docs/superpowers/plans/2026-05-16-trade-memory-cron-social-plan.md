# Trade + Memory + Cron + Social + Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 trending-agent 升级为持续运行的交易 agent，新增持仓管理、SQLite 历史存储、定时调度、Telegram 交互、Gateway 生命周期管理。

**Architecture:** Core crate 新增 trade/memory/cron/social/gateway 五个模块，CLI 新增 serve/trade 命令。Gateway 单进程运行 cron + telegram sessions，Web 独立部署。

**Tech Stack:** Rust, rusqlite (bundled), tokio-cron-scheduler, teloxide (optional feature), serde, clap

---

### Task 1: 添加核心依赖 & 更新 lib.rs

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: 添加 rusqlite + tokio-cron-scheduler + teloxide**

Read `crates/core/Cargo.toml`, add to `[dependencies]`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
tokio-cron-scheduler = "0.11"
teloxide = { version = "0.15", optional = true }
```

Add `[features]` section:

```toml
[features]
telegram = ["dep:teloxide"]
```

- [ ] **Step 2: 更新 lib.rs 声明新模块**

Read `crates/core/src/lib.rs`, replace `pub mod` declarations:

```rust
pub mod agents;
pub mod api;
pub mod llm;
pub mod report;
pub mod roles;
pub mod templates;
pub mod utils;
pub mod trade;
pub mod memory;
pub mod cron;
pub mod social;
pub mod gateway;
```

- [ ] **Step 3: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/lib.rs
git commit -m "chore(core): add rusqlite, cron-scheduler, teloxide deps + new module declarations"
```

---

### Task 2: Trade 模块 — 数据模型 + TradeManager trait

**Files:**
- Create: `crates/core/src/trade/mod.rs`

- [ ] **Step 1: 创建 trade/mod.rs**

```rust
use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub role_name: String,
    pub symbol: String,
    pub stock_name: String,
    pub entry_date: String,
    pub cost_price: f64,
    pub shares: i64,
    pub current_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub target_price: Option<f64>,
    pub entry_reason: String,
    pub sector: String,
    pub position_pct: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLog {
    pub id: i64,
    pub position_id: i64,
    pub action: String, // "Open" | "Add" | "Reduce" | "Close"
    pub price: f64,
    pub shares: i64,
    pub reason: String,
    pub created_at: String,
}

pub trait TradeManager: Send + Sync {
    fn list_positions(&self, role_name: &str) -> Result<Vec<Position>>;
    fn add_position(&self, pos: Position) -> Result<i64>;
    fn update_position(&self, pos: Position) -> Result<()>;
    fn close_position(&self, id: i64, price: f64, reason: &str) -> Result<()>;
    fn get_trade_logs(&self, position_id: i64) -> Result<Vec<TradeLog>>;
    fn format_positions(&self, role_name: &str) -> Result<String>;
}
```

- [ ] **Step 2: 编译**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/trade/
git commit -m "feat(trade): add Position, TradeLog models and TradeManager trait"
```

---

### Task 3: Trade 模块 — SQLite 实现

**Files:**
- Create: `crates/core/src/trade/sqlite.rs`

- [ ] **Step 1: 创建 sqlite.rs**

```rust
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use super::{Position, TradeLog, TradeManager};

pub struct SqliteTradeManager {
    conn: Mutex<Connection>,
}

impl SqliteTradeManager {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("无法打开 SQLite: {}", db_path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                role_name TEXT NOT NULL,
                symbol TEXT NOT NULL,
                stock_name TEXT NOT NULL DEFAULT '',
                entry_date TEXT NOT NULL DEFAULT '',
                cost_price REAL NOT NULL DEFAULT 0.0,
                shares INTEGER NOT NULL DEFAULT 0,
                current_price REAL,
                stop_loss REAL,
                target_price REAL,
                entry_reason TEXT NOT NULL DEFAULT '',
                sector TEXT NOT NULL DEFAULT '',
                position_pct REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS trade_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                position_id INTEGER NOT NULL REFERENCES positions(id),
                action TEXT NOT NULL,
                price REAL NOT NULL DEFAULT 0.0,
                shares INTEGER NOT NULL DEFAULT 0,
                reason TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );",
        )?;

        Ok(SqliteTradeManager {
            conn: Mutex::new(conn),
        })
    }
}

impl TradeManager for SqliteTradeManager {
    fn list_positions(&self, role_name: &str) -> Result<Vec<Position>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,role_name,symbol,stock_name,entry_date,cost_price,shares,\
             current_price,stop_loss,target_price,entry_reason,sector,position_pct,\
             created_at,updated_at FROM positions WHERE role_name=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([role_name], |row| {
            Ok(Position {
                id: row.get(0)?,
                role_name: row.get(1)?,
                symbol: row.get(2)?,
                stock_name: row.get(3)?,
                entry_date: row.get(4)?,
                cost_price: row.get(5)?,
                shares: row.get(6)?,
                current_price: row.get(7)?,
                stop_loss: row.get(8)?,
                target_price: row.get(9)?,
                entry_reason: row.get(10)?,
                sector: row.get(11)?,
                position_pct: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn add_position(&self, pos: Position) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO positions (role_name,symbol,stock_name,entry_date,cost_price,shares,\
             current_price,stop_loss,target_price,entry_reason,sector,position_pct)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                pos.role_name, pos.symbol, pos.stock_name, pos.entry_date,
                pos.cost_price, pos.shares, pos.current_price, pos.stop_loss,
                pos.target_price, pos.entry_reason, pos.sector, pos.position_pct
            ],
        )?;
        let id = conn.last_insert_rowid();
        // Write trade log for "Open" action
        conn.execute(
            "INSERT INTO trade_logs (position_id,action,price,shares,reason) VALUES (?1,'Open',?2,?3,?4)",
            rusqlite::params![id, pos.cost_price, pos.shares, pos.entry_reason],
        )?;
        Ok(id)
    }

    fn update_position(&self, pos: Position) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE positions SET current_price=?1,stop_loss=?2,target_price=?3,\
             updated_at=datetime('now','localtime') WHERE id=?4",
            rusqlite::params![pos.current_price, pos.stop_loss, pos.target_price, pos.id],
        )?;
        Ok(())
    }

    fn close_position(&self, id: i64, price: f64, reason: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO trade_logs (position_id,action,price,shares,reason) VALUES (?1,'Close',?2,0,?3)",
            rusqlite::params![id, price, reason],
        )?;
        conn.execute("DELETE FROM positions WHERE id=?1", [id])?;
        Ok(())
    }

    fn get_trade_logs(&self, position_id: i64) -> Result<Vec<TradeLog>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,position_id,action,price,shares,reason,created_at \
             FROM trade_logs WHERE position_id=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([position_id], |row| {
            Ok(TradeLog {
                id: row.get(0)?,
                position_id: row.get(1)?,
                action: row.get(2)?,
                price: row.get(3)?,
                shares: row.get(4)?,
                reason: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn format_positions(&self, role_name: &str) -> Result<String> {
        let positions = self.list_positions(role_name)?;
        if positions.is_empty() {
            return Ok("暂无持仓".to_string());
        }
        let lines: Vec<String> = positions.iter().map(|p| {
            let pnl = match p.current_price {
                Some(cp) if cp > 0.0 => format!("{:+.1}%", (cp - p.cost_price) / p.cost_price * 100.0),
                _ => "N/A".to_string(),
            };
            format!(
                "- #{} {} {} | 成本{} | 现价{} | {} | {}股 | {} | {}买入",
                p.id, p.symbol, p.stock_name, p.cost_price,
                p.current_price.map_or("N/A".to_string(), |v| v.to_string()),
                pnl, p.shares, p.sector, p.entry_date
            )
        }).collect();
        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::Position;

    fn test_db() -> SqliteTradeManager {
        SqliteTradeManager::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_add_and_list_positions() {
        let tm = test_db();
        tm.add_position(Position {
            id: 0, role_name: "zarili".into(), symbol: "000001".into(),
            stock_name: "平安银行".into(), entry_date: "2026-01-15".into(),
            cost_price: 10.5, shares: 1000, current_price: Some(11.2),
            stop_loss: Some(9.8), target_price: Some(12.0),
            entry_reason: "均线突破".into(), sector: "银行".into(),
            position_pct: 0.3, created_at: String::new(), updated_at: String::new(),
        }).unwrap();

        let positions = tm.list_positions("zarili").unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "000001");
    }

    #[test]
    fn test_close_position() {
        let tm = test_db();
        let id = tm.add_position(Position {
            id: 0, role_name: "trader".into(), symbol: "600519".into(),
            stock_name: "茅台".into(), entry_date: "2026-01-10".into(),
            cost_price: 1680.0, shares: 100, current_price: None,
            stop_loss: None, target_price: None,
            entry_reason: "".into(), sector: "白酒".into(),
            position_pct: 0.5, created_at: String::new(), updated_at: String::new(),
        }).unwrap();

        tm.close_position(id, 1700.0, "止盈").unwrap();
        assert!(tm.list_positions("trader").unwrap().is_empty());
        assert_eq!(tm.get_trade_logs(id).unwrap().len(), 2); // Open + Close
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test -p trending-agent-core trade::
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/trade/sqlite.rs
git commit -m "feat(trade): add SqliteTradeManager with WAL mode, CRUD, tests"
```

---

### Task 4: Memory 模块 — 数据模型 + MemoryStore trait

**Files:**
- Create: `crates/core/src/memory/mod.rs`

- [ ] **Step 1: 创建 memory/mod.rs**

```rust
use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub id: i64,
    pub captured_at: String,
    pub data_source: String,
    pub raw_json: String,
}

pub trait MemoryStore: Send + Sync {
    fn save_snapshot(&self, source: &str, json: &str) -> Result<i64>;
    fn query_recent(&self, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn query_by_source(&self, source: &str, days: u32) -> Result<Vec<MarketSnapshot>>;
    fn cleanup_old(&self) -> Result<()>;
    fn get_historical_context(&self, days: u32) -> Result<String>;
}
```

- [ ] **Step 2: 编译**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/memory/
git commit -m "feat(memory): add MarketSnapshot model and MemoryStore trait"
```

---

### Task 5: Memory 模块 — SQLite 实现

**Files:**
- Create: `crates/core/src/memory/sqlite.rs`

- [ ] **Step 1: 创建 sqlite.rs**

```rust
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use super::{MarketSnapshot, MemoryStore};

pub struct SqliteMemoryStore {
    conn: Mutex<Connection>,
}

impl SqliteMemoryStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS market_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                captured_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                data_source TEXT NOT NULL,
                raw_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_date ON market_snapshots(captured_at);
            CREATE INDEX IF NOT EXISTS idx_snapshots_source ON market_snapshots(data_source);",
        )?;
        Ok(SqliteMemoryStore {
            conn: Mutex::new(conn),
        })
    }
}

impl MemoryStore for SqliteMemoryStore {
    fn save_snapshot(&self, source: &str, json: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO market_snapshots (data_source, raw_json) VALUES (?1, ?2)",
            rusqlite::params![source, json],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.cleanup_old()?;
        Ok(id)
    }

    fn query_recent(&self, days: u32) -> Result<Vec<MarketSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, data_source, raw_json FROM market_snapshots \
             WHERE captured_at >= datetime('now', 'localtime', ?1) ORDER BY captured_at DESC",
        )?;
        let rows = stmt.query_map([format!("-{days} days")], |row| {
            Ok(MarketSnapshot {
                id: row.get(0)?,
                captured_at: row.get(1)?,
                data_source: row.get(2)?,
                raw_json: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn query_by_source(&self, source: &str, days: u32) -> Result<Vec<MarketSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, data_source, raw_json FROM market_snapshots \
             WHERE data_source = ?1 AND captured_at >= datetime('now', 'localtime', ?2) \
             ORDER BY captured_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![source, format!("-{days} days")], |row| {
            Ok(MarketSnapshot {
                id: row.get(0)?,
                captured_at: row.get(1)?,
                data_source: row.get(2)?,
                raw_json: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn cleanup_old(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM market_snapshots WHERE captured_at < datetime('now', 'localtime', '-10 days')",
            [],
        )?;
        Ok(())
    }

    fn get_historical_context(&self, days: u32) -> Result<String> {
        let snapshots = self.query_recent(days)?;
        if snapshots.is_empty() {
            return Ok("暂无历史数据".to_string());
        }
        let mut ctx = format!("## 近{days}日市场数据快照 ({})条记录\n\n", snapshots.len());
        for s in &snapshots {
            // Try to extract key fields from JSON for a readable summary
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s.raw_json) {
                ctx.push_str(&format!(
                    "- {} | {} | 摘要: {}\n",
                    s.captured_at,
                    s.data_source,
                    &serde_json::to_string(&v).unwrap_or_default()
                        .chars().take(200).collect::<String>()
                ));
            }
        }
        Ok(ctx)
    }
}

// Note: tests use :memory: DB which can't test datetimes easily
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/memory/sqlite.rs
git commit -m "feat(memory): add SqliteMemoryStore with 10-day cleanup"
```

---

### Task 6: build_prompt 扩展 — 持仓 + ToolDef

**Files:**
- Modify: `crates/core/src/roles/prompt.rs`

- [ ] **Step 1: 更新 prompt.rs**

Read current prompt.rs, replace with:

```rust
use serde::Serialize;

pub struct Prompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub fn default_tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: "query_market_memory".into(),
        description: "查询近N日市场历史数据，用于对比分析".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "days": {
                    "type": "integer",
                    "description": "查询天数，默认5天",
                    "default": 5
                },
                "source": {
                    "type": "string",
                    "enum": ["xuangubao", "jiuyan", "cls", "sentiment", "all"],
                    "description": "数据源，all表示所有",
                    "default": "all"
                }
            }
        }),
    }]
}

/// system = 策略 + 持仓 + 工具 + 核心原则
/// user = 市场数据 + 分析指令
pub fn build_prompt(
    strategy_text: &str,
    market_data_json: &str,
    position_context: &str,
    available_tools: &[ToolDef],
    display_name: &str,
) -> Prompt {
    let tools_json = serde_json::to_string_pretty(available_tools).unwrap_or_default();

    Prompt {
        system: format!(
            r#"{strategy}

## 当前持仓
{positions}

## 可用工具
```json
{tools}
```

## 核心原则
- 红涨绿跌（A股规则）
- 严格遵守上述策略体系进行交易决策
- 结合持仓情况，给出明确的加仓、减仓、持有、清仓建议及理由
- 用数据和逻辑说话，不预测具体涨跌点位"#,
            strategy = strategy_text,
            positions = position_context,
            tools = tools_json,
        ),
        user: format!(
            r#"## 今日市场数据

```json
{market_data}
```

请以【{name}】身份，根据策略体系分析当前市场状态和持仓情况，给出明确的持仓调整建议。
需要历史数据对比时可调用 query_market_memory。"#,
            market_data = market_data_json,
            name = display_name,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_with_positions() {
        let tools = default_tools();
        let prompt = build_prompt(
            "策略正文",
            r#"{"rise": 3000}"#,
            "- #1 000001 平安银行 | 成本10.5",
            &tools,
            "测试角色",
        );
        assert!(prompt.system.contains("策略正文"));
        assert!(prompt.system.contains("当前持仓"));
        assert!(prompt.system.contains("000001"));
        assert!(prompt.system.contains("query_market_memory"));
        assert!(prompt.system.contains("红涨绿跌"));
        assert!(prompt.user.contains("测试角色"));
        assert!(prompt.user.contains("query_market_memory"));
    }

    #[test]
    fn test_build_prompt_empty_positions() {
        let tools = default_tools();
        let prompt = build_prompt("策略", "{}", "暂无持仓", &tools, "测试");
        assert!(prompt.system.contains("暂无持仓"));
    }
}
```

- [ ] **Step 2: 运行测试**

```bash
cargo test -p trending-agent-core roles::prompt::
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/roles/prompt.rs
git commit -m "feat(prompt): add position_context + ToolDef + function calling to build_prompt"
```

---

### Task 7: Cron 模块

**Files:**
- Create: `crates/core/src/cron/mod.rs`

- [ ] **Step 1: 创建 cron/mod.rs**

```rust
use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::memory::MemoryStore;
use crate::trade::TradeManager;
use crate::social::NotificationSender;

#[derive(Debug, Clone, Deserialize)]
pub struct CronConfig {
    pub enabled: bool,
    pub collection_schedule: String,
    pub decision_schedule: String,
}

#[derive(Debug, Clone)]
pub struct CollectSummary {
    pub timestamp: NaiveDateTime,
    pub sources: Vec<String>,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeSuggestion {
    pub symbol: String,
    pub action: String, // "add" | "reduce" | "hold" | "close"
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub role_name: String,
    pub summary: String,
    pub trade_suggestions: Vec<TradeSuggestion>,
}

/// 启动 cron scheduler，注册采集和决策两个 Job
pub async fn start_scheduler(
    config: &CronConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: Arc<dyn NotificationSender>,
    config_path: String,
) -> Result<JobScheduler> {
    let scheduler = JobScheduler::new().await?;

    if !config.enabled {
        return Ok(scheduler);
    }

    // ── 采集 Job ──────────────────────────────────────
    let mem_c = memory.clone();
    let soc_c = social.clone();
    let schedule = config.collection_schedule.clone();
    scheduler.add(
        Job::new_async(schedule.as_str(), move |_uuid, _lock| {
            let mem = mem_c.clone();
            let soc = soc_c.clone();
            Box::pin(async move {
                // Stub: collection logic will call actual API clients
                // For now, record that a collection tick fired
                if let Err(e) = mem.save_snapshot("cron_tick", "{}") {
                    eprintln!("[cron] save_snapshot error: {e}");
                }
                // TODO: wire real API clients for collection
                let summary = CollectSummary {
                    timestamp: chrono::Local::now().naive_local(),
                    sources: vec!["cron_tick".into()],
                    snapshot_count: 1,
                };
                let _ = soc.notify_collect(&summary).await;
                Ok(())
            })
        })?
    ).await?;

    // ── 决策 Job ───────────────────────────────────────
    let mem_d = memory.clone();
    let trd = trade.clone();
    let soc_d = social.clone();
    let schedule_d = config.decision_schedule.clone();
    let cfg_path = config_path.clone();
    scheduler.add(
        Job::new_async(schedule_d.as_str(), move |_uuid, _lock| {
            let mem = mem_d.clone();
            let trd = trd.clone();
            let soc = soc_d.clone();
            let cp = cfg_path.clone();
            Box::pin(async move {
                if let Err(e) = run_decision(&mem, &trd, &soc, &cp).await {
                    eprintln!("[cron] decision error: {e}");
                }
                Ok(())
            })
        })?
    ).await?;

    scheduler.start().await?;
    Ok(scheduler)
}

async fn run_decision(
    memory: &dyn MemoryStore,
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
    config_path: &str,
) -> Result<()> {
    // Load config, get default role
    let app_config = crate::gateway::load_app_config_from_path(config_path)?;
    let role_name = app_config.default_role.as_deref().unwrap_or("zarili");

    // Get positions
    let position_context = trade.format_positions(role_name)?;

    // Get recent market data context
    let historical = memory.get_historical_context(5)?;

    // Build a decision summary
    let summary = format!(
        "角色【{}】决策检查\n持仓:\n{}\n近5日历史:\n{}",
        role_name, position_context, historical
    );

    let decision = Decision {
        role_name: role_name.to_string(),
        summary,
        trade_suggestions: vec![],
    };

    social.notify_decision(&decision).await?;
    Ok(())
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/cron/
git commit -m "feat(cron): add CronConfig, scheduler with collection + decision jobs"
```

---

### Task 8: Social 模块 — trait + telegram

**Files:**
- Create: `crates/core/src/social/mod.rs`
- Create: `crates/core/src/social/telegram.rs`

- [ ] **Step 1: 创建 social/mod.rs**

```rust
use anyhow::Result;
use async_trait::async_trait;

use crate::cron::{CollectSummary, Decision};
use crate::trade::TradeLog;

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()>;
    async fn notify_decision(&self, decision: &Decision) -> Result<()>;
    async fn notify_trade(&self, log: &TradeLog) -> Result<()>;
}

/// 空实现 — 当 telegram 未启用时使用
pub struct NoopSender;

#[async_trait]
impl NotificationSender for NoopSender {
    async fn notify_collect(&self, _: &CollectSummary) -> Result<()> { Ok(()) }
    async fn notify_decision(&self, _: &Decision) -> Result<()> { Ok(()) }
    async fn notify_trade(&self, _: &TradeLog) -> Result<()> { Ok(()) }
}
```

- [ ] **Step 2: 创建 social/telegram.rs (stub)**

```rust
use anyhow::Result;
use async_trait::async_trait;

use super::NotificationSender;
use crate::cron::{CollectSummary, Decision};
use crate::trade::TradeLog;

/// Telegram sender — stub implementation
/// Full teloxide integration requires `telegram` feature flag
pub struct TelegramSender {
    pub bot_token: String,
    pub chat_id: String,
    pub notify_on_collect: bool,
    pub notify_on_decision: bool,
    pub notify_on_trade: bool,
}

impl TelegramSender {
    async fn send_message(&self, text: &str) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        let client = reqwest::Client::new();
        client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl NotificationSender for TelegramSender {
    async fn notify_collect(&self, summary: &CollectSummary) -> Result<()> {
        if !self.notify_on_collect { return Ok(()); }
        let text = format!(
            "📊 *采集完成*\n时间: {}\n数据源: {}\n快照: {} 条",
            summary.timestamp.format("%H:%M:%S"),
            summary.sources.join(", "),
            summary.snapshot_count,
        );
        self.send_message(&text).await?;
        Ok(())
    }

    async fn notify_decision(&self, decision: &Decision) -> Result<()> {
        if !self.notify_on_decision { return Ok(()); }
        self.send_message(&decision.summary).await?;
        Ok(())
    }

    async fn notify_trade(&self, log: &TradeLog) -> Result<()> {
        if !self.notify_on_trade { return Ok(()); }
        let text = format!(
            "💹 *持仓变动*\n操作: {}\n价格: {}\n股数: {}\n理由: {}",
            log.action, log.price, log.shares, log.reason,
        );
        self.send_message(&text).await?;
        Ok(())
    }
}
```

- [ ] **Step 3: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/social/
git commit -m "feat(social): add NotificationSender trait + NoopSender + TelegramSender stub"
```

---

### Task 9: Gateway 模块

**Files:**
- Create: `crates/core/src/gateway/mod.rs`

- [ ] **Step 1: 创建 gateway/mod.rs**

```rust
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::cron;
use crate::memory::SqliteMemoryStore;
use crate::social::{NoopSender, NotificationSender, TelegramSender};
use crate::trade::SqliteTradeManager;

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

impl Gateway {
    /// 初始化 Gateway，打开 SQLite 数据库
    pub fn init(db_path: &Path) -> Result<Self> {
        let memory = Arc::new(SqliteMemoryStore::open(db_path)?);
        let trade = Arc::new(SqliteTradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    /// 启动 cron + social sessions
    pub async fn start(
        &self,
        cron_config: &cron::CronConfig,
        telegram_config: Option<&crate::social::telegram::TelegramSender>,
        config_path: &str,
    ) -> Result<()> {
        let social: Arc<dyn NotificationSender> = if let Some(tg) = telegram_config {
            Arc::new(TelegramSender {
                bot_token: tg.bot_token.clone(),
                chat_id: tg.chat_id.clone(),
                notify_on_collect: tg.notify_on_collect,
                notify_on_decision: tg.notify_on_decision,
                notify_on_trade: tg.notify_on_trade,
            })
        } else {
            Arc::new(NoopSender)
        };

        let scheduler = cron::start_scheduler(
            cron_config,
            self.memory.clone(),
            self.trade.clone(),
            social,
            config_path.to_string(),
        )
        .await?;

        // Block forever
        tokio::signal::ctrl_c().await?;
        drop(scheduler);
        Ok(())
    }
}
```

- [ ] **Step 2: 创建 load_app_config_from_path 辅助函数**

Add to `gateway/mod.rs`:

```rust
use crate::cli::config::AppConfig;

pub fn load_app_config_from_path(config_path: &str) -> Result<AppConfig> {
    // Try to load from the given path or fall back to current dir
    let path = Path::new(config_path);
    let project_root = if path.is_file() {
        path.parent().unwrap_or(Path::new("."))
    } else {
        Path::new(".")
    };
    let app_config = crate::cli::config::AppConfig::load(project_root)?;
    Ok(app_config)
}
```

Wait — this creates a circular dependency (core → cli). Let me fix this by moving AppConfig out, or by having gateway accept a config struct directly.

Actually, the clean approach: gateway doesn't depend on CLI config. The CLI reads config and passes the relevant parts to Gateway. Let me redesign:

```rust
impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self> { ... }

    pub async fn start(
        &self,
        cron_config: &cron::CronConfig,
        social: Arc<dyn NotificationSender>,
    ) -> Result<()> { ... }
}
```

The CLI does: read config → init Gateway → start Gateway with config sections.

- [ ] **Step 3: Fix gateway/mod.rs — no AppConfig dependency**

```rust
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::cron::{self, CronConfig};
use crate::memory::SqliteMemoryStore;
use crate::social::NotificationSender;
use crate::trade::SqliteTradeManager;

pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(db_path.parent().unwrap_or(Path::new(".")))?;
        let memory = Arc::new(SqliteMemoryStore::open(db_path)?);
        let trade = Arc::new(SqliteTradeManager::open(db_path)?);
        Ok(Gateway { memory, trade })
    }

    pub async fn start(
        &self,
        cron_config: &CronConfig,
        social: Arc<dyn NotificationSender>,
    ) -> Result<()> {
        let scheduler = cron::start_scheduler(
            cron_config,
            self.memory.clone(),
            self.trade.clone(),
            social,
        )
        .await?;

        eprintln!("[gateway] 已启动 (Ctrl+C 停止)");
        tokio::signal::ctrl_c().await?;
        eprintln!("\n[gateway] 正在停止...");
        drop(scheduler);
        Ok(())
    }
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/gateway/
git commit -m "feat(gateway): add Gateway with init + start lifecycle"
```

---

### Task 10: Cron 模块 — 修复 start_scheduler 签名

**Files:**
- Modify: `crates/core/src/cron/mod.rs`

- [ ] **Step 1: 移除 config_path 参数**

`start_scheduler` 不再需要 config_path，decision job 改为调用 gateway 传入的参数：

```rust
pub async fn start_scheduler(
    config: &CronConfig,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: Arc<dyn NotificationSender>,
) -> Result<JobScheduler> {
```

And in run_decision, remove config_path and role config loading since the LLM call will be done via the decision job that has access to trade + memory:

```rust
async fn run_decision(
    memory: &dyn MemoryStore,
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
) -> Result<()> {
    // For now: collect positions from all roles and summarize
    let roles = ["zarili", "trader", "quant", "risk_officer"];
    for role_name in &roles {
        if let Ok(context) = trade.format_positions(role_name) {
            if context != "暂无持仓" {
                let historical = memory.get_historical_context(5).unwrap_or_default();
                let decision = Decision {
                    role_name: role_name.to_string(),
                    summary: format!(
                        "角色【{}】\n持仓:\n{}\n近5日数据: {}条",
                        role_name, context,
                        historical.lines().count()
                    ),
                    trade_suggestions: vec![],
                };
                let _ = social.notify_decision(&decision).await;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p trending-agent-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/cron/mod.rs
git commit -m "fix(cron): remove config_path dependency, iterate all roles in decision"
```

---

### Task 11: Config 模块扩展 — service + cron + social sections

**Files:**
- Modify: `crates/cli/src/cli/config.rs`

- [ ] **Step 1: 添加配置结构体**

Add to `crates/cli/src/cli/config.rs`:

```rust
use serde::Deserialize;

// ... existing RawConfig ...

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

fn default_db_path() -> String {
    "data.db".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct CronTomlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_schedule")]
    pub collection_schedule: String,
    #[serde(default = "default_decision_schedule")]
    pub decision_schedule: String,
}

fn default_true() -> bool { true }
fn default_collection_schedule() -> String { "*/30 9-15 * * 1-5".to_string() }
fn default_decision_schedule() -> String { "0 0 * * 1-5".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct SocialConfig {
    pub telegram: Option<TelegramConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    #[serde(default = "default_true")]
    pub notify_on_collect: bool,
    #[serde(default = "default_true")]
    pub notify_on_decision: bool,
    #[serde(default = "default_true")]
    pub notify_on_trade: bool,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    default_role: Option<String>,
    #[serde(default)]
    roles: Vec<RoleConfig>,
    #[serde(default)]
    service: Option<ServiceConfig>,
    #[serde(default)]
    cron: Option<CronTomlConfig>,
    #[serde(default)]
    social: Option<SocialConfig>,
}

// Extend AppConfig
impl AppConfig {
    pub fn service(&self) -> ServiceConfig { ... }
    pub fn cron(&self) -> Option<CronTomlConfig> { ... }
    pub fn social(&self) -> Option<SocialConfig> { ... }
}
```

- [ ] **Step 2: 存储 service/cron/social 到 AppConfig**

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<RoleEntry>,
    pub service: ServiceConfig,
    pub cron: Option<CronTomlConfig>,
    pub social: Option<SocialConfig>,
}
```

Update `load_from`, `merge`, and `DEFAULT_CONFIG_TOML` to include new sections.

- [ ] **Step 3: 编译验证**

```bash
cargo check -p trending-agent-cli
```

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/cli/config.rs
git commit -m "feat(config): add service + cron + social sections"
```

---

### Task 12: CLI serve 命令

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Create: `crates/cli/src/cli/serve.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 添加 serve 子命令到 app.rs**

Add to `Commands` enum:

```rust
#[derive(Subcommand)]
pub enum Commands {
    Init,
    /// 启动后台服务 (cron + telegram)
    Serve,
}
```

- [ ] **Step 2: 创建 serve.rs**

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::config::AppConfig;
use trending_agent_core::gateway::Gateway;
use trending_agent_core::social::telegram::TelegramSender;
use trending_agent_core::social::{NoopSender, NotificationSender};

pub async fn cmd_serve(project_root: &PathBuf) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;

    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let social: Arc<dyn NotificationSender> = match &app_config.social {
        Some(sc) if sc.telegram.as_ref().map(|t| t.enabled).unwrap_or(false) => {
            let tg = sc.telegram.as_ref().unwrap();
            Arc::new(TelegramSender {
                bot_token: tg.bot_token.clone().unwrap_or_default(),
                chat_id: tg.chat_id.clone().unwrap_or_default(),
                notify_on_collect: tg.notify_on_collect,
                notify_on_decision: tg.notify_on_decision,
                notify_on_trade: tg.notify_on_trade,
            })
        }
        _ => Arc::new(NoopSender),
    };

    let cron_config = app_config.cron.clone().unwrap_or_default();

    gateway.start(&cron_config.into(), social).await
}
```

- [ ] **Step 3: 更新 mod.rs 调用 serve**

In `crates/cli/src/cli/mod.rs`, add handler for the Serve command:

```rust
use crate::cli::serve::cmd_serve;

// In run():
if let Some(Commands::Serve) = cli.command {
    return cmd_serve(&project_root).await;
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo check -p trending-agent-cli
```

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/cli/app.rs crates/cli/src/cli/serve.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): add serve command to start gateway"
```

---

### Task 13: CLI trade 命令

**Files:**
- Modify: `crates/cli/src/cli/app.rs`
- Create: `crates/cli/src/cli/trade_cmd.rs`
- Modify: `crates/cli/src/cli/mod.rs`

- [ ] **Step 1: 添加 Trade 子命令到 app.rs**

```rust
#[derive(Subcommand)]
pub enum Commands {
    Init,
    Serve,
    /// 持仓管理
    Trade {
        #[command(subcommand)]
        action: TradeAction,
    },
}

#[derive(Subcommand)]
pub enum TradeAction {
    /// 添加持仓
    Add {
        #[arg(long)]
        role: String,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        price: f64,
        #[arg(long)]
        shares: i64,
    },
    /// 列出持仓
    List {
        #[arg(long)]
        role: Option<String>,
    },
    /// 平仓
    Close {
        id: i64,
        #[arg(long)]
        price: f64,
    },
}
```

- [ ] **Step 2: 创建 trade_cmd.rs**

```rust
use anyhow::Result;
use std::path::PathBuf;
use chrono::Local;

use crate::cli::config::AppConfig;
use trending_agent_core::trade::{Position, SqliteTradeManager, TradeManager};

pub fn cmd_trade_add(
    project_root: &PathBuf,
    role: &str, symbol: &str, price: f64, shares: i64,
) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root.join(".atrading").join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    let pos = Position {
        id: 0,
        role_name: role.to_string(),
        symbol: symbol.to_string(),
        stock_name: String::new(),
        entry_date: Local::now().format("%Y-%m-%d").to_string(),
        cost_price: price,
        shares,
        current_price: None,
        stop_loss: None,
        target_price: None,
        entry_reason: String::new(),
        sector: String::new(),
        position_pct: 0.0,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let id = tm.add_position(pos)?;
    println!("已添加持仓 #{id}: {symbol} @ {price} x {shares}股");
    Ok(())
}

pub fn cmd_trade_list(project_root: &PathBuf, role: Option<&str>) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root.join(".atrading").join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    let role_name = role.unwrap_or("zarili");
    let text = tm.format_positions(role_name)?;
    println!("{text}");
    Ok(())
}

pub fn cmd_trade_close(project_root: &PathBuf, id: i64, price: f64) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;
    let db_path = project_root.join(".atrading").join(&app_config.service.db_path);
    let tm = SqliteTradeManager::open(&db_path)?;
    tm.close_position(id, price, "手动平仓")?;
    println!("已平仓 #{id} @ {price}");
    Ok(())
}
```

- [ ] **Step 3: 更新 mod.rs**

```rust
use crate::cli::trade_cmd::{cmd_trade_add, cmd_trade_list, cmd_trade_close};

// In run():
if let Some(Commands::Trade { action }) = cli.command {
    match action {
        TradeAction::Add { role, symbol, price, shares } => {
            return cmd_trade_add(&project_root, &role, &symbol, price, shares);
        }
        TradeAction::List { role } => {
            return cmd_trade_list(&project_root, role.as_deref());
        }
        TradeAction::Close { id, price } => {
            return cmd_trade_close(&project_root, id, price);
        }
    }
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo check -p trending-agent-cli
```

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/cli/app.rs crates/cli/src/cli/trade_cmd.rs crates/cli/src/cli/mod.rs
git commit -m "feat(cli): add trade add/list/close commands"
```

---

### Task 14: 全量编译 & 集成测试

**Files:** (none, verification only)

- [ ] **Step 1: 全量编译**

```bash
cargo check --workspace
```

Expected: all crates compile without errors.

- [ ] **Step 2: 运行全部测试**

```bash
cargo test --workspace
```

Expected: all existing + new tests pass.

- [ ] **Step 3: Commit any fixes if needed**
