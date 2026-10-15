//! 交易数据的 SQLite 持久化。
//!
//! 只负责行级读写与**事务边界**；金额与盈亏的算式放在 [`super::order`] 的纯函数里，
//! 便于单测。
//!
//! 现金不落库，由成交流水推导（见模块文档），因此不存在现金列与流水不一致的可能。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use super::order::{round_money, ClosedTrade, Position, Side, Trade};
use super::{OrderError, Result};
use crate::types::StockCode;

/// 浮点比较容差：买卖金额都是 `price × shares` 的累加，判「现金是否够」时
/// 允许 1 分钱的误差，避免 `100.0` 与 `99.99999999999999` 被判为不足。
const CASH_EPSILON: f64 = 0.01;

/// 当前 schema 版本。`user_version` 小于此值时重建交易表。
const SCHEMA_VERSION: i64 = 2;

/// 写入一笔成交所需的全部信息。
///
/// 字段多于 3 个，按项目规范合并为 struct。
pub struct PlaceOrderCtx<'a> {
    pub role_name: &'a str,
    pub symbol: StockCode,
    pub stock_name: &'a str,
    pub side: Side,
    pub shares: i64,
    pub price: f64,
    pub reason: &'a str,
    pub initial_capital: f64,
    pub traded_at: &'a str,
}

/// 一笔成交的结果。
pub struct OrderOutcome {
    pub trade: Trade,
    /// 卖光时持仓被删除，此时为 `None`。
    pub position: Option<Position>,
}

/// 交易数据存储。
pub struct TradeStore {
    conn: Mutex<Connection>,
}

impl TradeStore {
    /// 打开数据库并确保 schema 就绪。
    ///
    /// 旧版本的 `positions` / `trade_logs` 与当前 schema 不兼容，版本低于
    /// [`SCHEMA_VERSION`] 时直接重建这两张表（其余表不动）。
    ///
    /// # Errors
    /// 数据库无法打开或建表失败时返回错误。
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // busy_timeout:memory / trade / automation 三处各持一个连接写同一个库，
        // 没有它会直接拿到 SQLITE_BUSY。
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version < SCHEMA_VERSION {
            // 顺序要紧：旧库的 `trade_logs` 有指向 `positions(id)` 的外键，
            // 先删父表会直接撞 FOREIGN KEY constraint failed。子表先删。
            conn.execute_batch(
                "DROP TABLE IF EXISTS trade_logs;
                 DROP TABLE IF EXISTS positions;",
            )?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS positions (
                role_name  TEXT    NOT NULL,
                symbol     TEXT    NOT NULL,
                stock_name TEXT    NOT NULL,
                shares     INTEGER NOT NULL,
                cost_price REAL    NOT NULL,
                opened_at  TEXT    NOT NULL,
                PRIMARY KEY (role_name, symbol)
            );
            CREATE TABLE IF NOT EXISTS trades (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                role_name    TEXT    NOT NULL,
                symbol       TEXT    NOT NULL,
                stock_name   TEXT    NOT NULL,
                side         TEXT    NOT NULL CHECK (side IN ('buy','sell')),
                price        REAL    NOT NULL,
                shares       INTEGER NOT NULL,
                cost_price   REAL    NOT NULL,
                realized_pnl REAL,
                reason       TEXT    NOT NULL DEFAULT '',
                traded_at    TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trades_role ON trades(role_name, traded_at DESC);",
        )?;
        if version < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| OrderError::Store(format!("数据库锁已中毒: {e}")))
    }

    /// 读取某角色的持仓（按代码排序）。
    ///
    /// # Errors
    /// 查询失败时返回错误。
    pub fn list_positions(&self, role_name: &str) -> Result<Vec<Position>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT role_name, symbol, stock_name, shares, cost_price, opened_at
             FROM positions WHERE role_name = ?1 ORDER BY symbol",
        )?;
        let rows = stmt.query_map([role_name], Self::row_to_position)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 读取某角色的成交流水（倒序）。
    ///
    /// # Errors
    /// 查询失败时返回错误。
    pub fn list_trades(&self, role_name: &str, limit: u32) -> Result<Vec<Trade>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, role_name, symbol, stock_name, side, price, shares,
                    cost_price, realized_pnl, reason, traded_at
             FROM trades WHERE role_name = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![role_name, limit], Self::row_to_trade)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 读取某角色按时间正序的全部流水（盈亏曲线需要按发生顺序回放）。
    ///
    /// # Errors
    /// 查询失败时返回错误。
    pub fn all_trades(&self, role_name: &str) -> Result<Vec<Trade>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, role_name, symbol, stock_name, side, price, shares,
                    cost_price, realized_pnl, reason, traded_at
             FROM trades WHERE role_name = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([role_name], Self::row_to_trade)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 读取某角色的已平仓记录（只有卖出）。
    ///
    /// # Errors
    /// 查询失败时返回错误。
    pub fn list_closed_trades(&self, role_name: &str, limit: u32) -> Result<Vec<ClosedTrade>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, role_name, symbol, stock_name, shares, cost_price, price,
                    COALESCE(realized_pnl, 0.0), reason, traded_at
             FROM trades WHERE role_name = ?1 AND side = 'sell'
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![role_name, limit], |row| {
            let cost_price: f64 = row.get(5)?;
            let exit_price: f64 = row.get(6)?;
            let realized_pnl: f64 = row.get(7)?;
            let cost_total = cost_price * row.get::<_, i64>(4)? as f64;
            let pct = if cost_total > 0.0 {
                realized_pnl / cost_total * 100.0
            } else {
                0.0
            };
            Ok(ClosedTrade {
                id: row.get(0)?,
                role_name: row.get(1)?,
                symbol: StockCode::new(row.get::<_, String>(2)?),
                stock_name: row.get(3)?,
                shares: row.get(4)?,
                cost_price,
                exit_price,
                realized_pnl: round_money(realized_pnl),
                realized_pct: (pct * 100.0).round() / 100.0,
                closed_at: row.get(9)?,
                reason: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// 由流水推导当前现金与累计已实现盈亏。
    ///
    /// # Errors
    /// 查询失败时返回错误。
    pub fn cash_and_realized(&self, role_name: &str, initial_capital: f64) -> Result<(f64, f64)> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT side, price, shares, COALESCE(realized_pnl, 0.0)
             FROM trades WHERE role_name = ?1",
        )?;
        let rows = stmt.query_map([role_name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;

        let mut cash = initial_capital;
        let mut realized = 0.0;
        for row in rows {
            let (side, price, shares, pnl) = row?;
            let amount = price * shares as f64;
            match Side::parse(&side)? {
                Side::Buy => cash -= amount,
                Side::Sell => {
                    cash += amount;
                    realized += pnl;
                }
            }
        }
        Ok((round_money(cash), round_money(realized)))
    }

    /// 在一个事务内校验并写入一笔成交。
    ///
    /// 校验（现金是否够、持仓是否够卖、买入是否整手）与写入处于同一事务，
    /// 失败时**不留下任何半截状态**。
    ///
    /// # Errors
    /// 校验失败返回对应的 [`OrderError`] 变体；存储失败返回 [`OrderError::Store`]。
    pub fn place_order(&self, ctx: &PlaceOrderCtx<'_>) -> Result<OrderOutcome> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;

        let amount = round_money(ctx.price * ctx.shares as f64);
        let existing = Self::load_position(&tx, ctx.role_name, ctx.symbol.as_str())?;

        // 名称一律以**非空**的入参为准（空值不覆盖已有名称）。前端是在搜索框里选中
        // 股票的，`stock_name` 来自后端搜索接口，本身就是权威名称；程序化调用不带
        // 名称时由服务层用行情补全。因此这里不需要区分名称的来源。
        let stock_name: String = if ctx.stock_name.is_empty() {
            existing
                .as_ref()
                .map_or_else(String::new, |p| p.stock_name.clone())
        } else {
            ctx.stock_name.to_string()
        };

        let (cost_price, realized_pnl, remaining) = match ctx.side {
            Side::Buy => {
                let (cash, _) =
                    Self::cash_and_realized_tx(&tx, ctx.role_name, ctx.initial_capital)?;
                if amount > cash + CASH_EPSILON {
                    return Err(OrderError::InsufficientCash {
                        available: cash,
                        required: amount,
                    });
                }
                // 加权平均成本：加仓后成本被摊薄/抬高，而不是新开一行。
                let prev_shares = existing.as_ref().map_or(0, |p| p.shares);
                let prev_cost = existing.as_ref().map_or(0.0, |p| p.cost_price);
                let total_shares = prev_shares + ctx.shares;
                let blended = (prev_cost * prev_shares as f64 + amount) / total_shares as f64;
                (blended, None, total_shares)
            }
            Side::Sell => {
                let held = existing.as_ref().map_or(0, |p| p.shares);
                if ctx.shares > held {
                    return Err(OrderError::InsufficientShares {
                        held,
                        requested: ctx.shares,
                    });
                }
                let basis = existing.as_ref().map_or(0.0, |p| p.cost_price);
                let pnl = round_money((ctx.price - basis) * ctx.shares as f64);
                (basis, Some(pnl), held - ctx.shares)
            }
        };

        if remaining == 0 {
            tx.execute(
                "DELETE FROM positions WHERE role_name = ?1 AND symbol = ?2",
                rusqlite::params![ctx.role_name, ctx.symbol.as_str()],
            )?;
        } else {
            // opened_at 只在建仓时写入，加仓不覆盖。
            let opened_at = existing
                .as_ref()
                .map_or_else(|| ctx.traded_at.to_string(), |p| p.opened_at.clone());
            tx.execute(
                "INSERT INTO positions (role_name, symbol, stock_name, shares, cost_price, opened_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(role_name, symbol) DO UPDATE SET
                     shares = excluded.shares,
                     cost_price = excluded.cost_price,
                     stock_name = excluded.stock_name",
                rusqlite::params![
                    ctx.role_name,
                    ctx.symbol.as_str(),
                    stock_name,
                    remaining,
                    cost_price,
                    opened_at
                ],
            )?;
        }

        tx.execute(
            "INSERT INTO trades (role_name, symbol, stock_name, side, price, shares,
                                 cost_price, realized_pnl, reason, traded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                ctx.role_name,
                ctx.symbol.as_str(),
                stock_name,
                ctx.side.as_str(),
                ctx.price,
                ctx.shares,
                round_money(cost_price),
                realized_pnl,
                ctx.reason,
                ctx.traded_at
            ],
        )?;
        let trade_id = tx.last_insert_rowid();
        tx.commit()?;

        let trade = Trade {
            id: trade_id,
            role_name: ctx.role_name.to_string(),
            symbol: ctx.symbol.clone(),
            stock_name: stock_name.clone(),
            side: ctx.side,
            price: ctx.price,
            shares: ctx.shares,
            cost_price: round_money(cost_price),
            realized_pnl,
            reason: ctx.reason.to_string(),
            traded_at: ctx.traded_at.to_string(),
        };
        let position = if remaining == 0 {
            None
        } else {
            Some(Position {
                role_name: ctx.role_name.to_string(),
                symbol: ctx.symbol.clone(),
                stock_name: stock_name.clone(),
                shares: remaining,
                cost_price: round_money(cost_price),
                opened_at: ctx.traded_at.to_string(),
            })
        };
        Ok(OrderOutcome { trade, position })
    }

    /// 清空某角色的持仓与流水（保留角色本身）。
    ///
    /// # Errors
    /// 写入失败时返回错误。
    pub fn reset_role(&self, role_name: &str) -> Result<()> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM positions WHERE role_name = ?1", [role_name])?;
        tx.execute("DELETE FROM trades WHERE role_name = ?1", [role_name])?;
        tx.commit()?;
        Ok(())
    }

    fn load_position(
        tx: &rusqlite::Transaction<'_>,
        role_name: &str,
        symbol: &str,
    ) -> Result<Option<Position>> {
        let mut stmt = tx.prepare(
            "SELECT role_name, symbol, stock_name, shares, cost_price, opened_at
             FROM positions WHERE role_name = ?1 AND symbol = ?2",
        )?;
        let mut rows =
            stmt.query_map(rusqlite::params![role_name, symbol], Self::row_to_position)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 事务内的现金/已实现盈亏推导。语义与 [`Self::cash_and_realized`] 一致，
    /// 区别是走事务句柄，避免在持有写事务时再去抢连接锁（会死锁）。
    fn cash_and_realized_tx(
        tx: &rusqlite::Transaction<'_>,
        role_name: &str,
        initial_capital: f64,
    ) -> Result<(f64, f64)> {
        let mut stmt = tx.prepare(
            "SELECT side, price, shares, COALESCE(realized_pnl, 0.0)
             FROM trades WHERE role_name = ?1",
        )?;
        let rows = stmt.query_map([role_name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;
        let mut cash = initial_capital;
        let mut realized = 0.0;
        for row in rows {
            let (side, price, shares, pnl) = row?;
            let amount = price * shares as f64;
            match Side::parse(&side)? {
                Side::Buy => cash -= amount,
                Side::Sell => {
                    cash += amount;
                    realized += pnl;
                }
            }
        }
        Ok((round_money(cash), round_money(realized)))
    }

    fn row_to_position(row: &rusqlite::Row<'_>) -> rusqlite::Result<Position> {
        Ok(Position {
            role_name: row.get(0)?,
            symbol: StockCode::new(row.get::<_, String>(1)?),
            stock_name: row.get(2)?,
            shares: row.get(3)?,
            cost_price: row.get(4)?,
            opened_at: row.get(5)?,
        })
    }

    fn row_to_trade(row: &rusqlite::Row<'_>) -> rusqlite::Result<Trade> {
        let side: String = row.get(4)?;
        Ok(Trade {
            id: row.get(0)?,
            role_name: row.get(1)?,
            symbol: StockCode::new(row.get::<_, String>(2)?),
            stock_name: row.get(3)?,
            // 库里的取值受 CHECK 约束，只可能是 buy / sell。
            side: match side.as_str() {
                "buy" => Side::Buy,
                _ => Side::Sell,
            },
            price: row.get(5)?,
            shares: row.get(6)?,
            cost_price: row.get(7)?,
            realized_pnl: row.get(8)?,
            reason: row.get(9)?,
            traded_at: row.get(10)?,
        })
    }
}
