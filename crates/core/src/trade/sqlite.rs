#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::missing_errors_doc)]
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use super::{ClosedPosition, Position, TradeAction, TradeLog, TradeLogEntry};
use crate::types::StockCode;

pub struct TradeManager {
    conn: Mutex<Connection>,
}

impl TradeManager {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("无法打开 SQLite: {}", db_path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=OFF;")?;

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

        // Migration: add role_name to trade_logs for closed-position traceability
        conn.execute(
            "ALTER TABLE trade_logs ADD COLUMN role_name TEXT NOT NULL DEFAULT ''",
            [],
        )
        .ok();

        // Migration: add symbol to trade_logs for closed-position traceability
        conn.execute(
            "ALTER TABLE trade_logs ADD COLUMN symbol TEXT NOT NULL DEFAULT ''",
            [],
        )
        .ok();

        // Migration: add stock_name to trade_logs for historical position display
        conn.execute(
            "ALTER TABLE trade_logs ADD COLUMN stock_name TEXT NOT NULL DEFAULT ''",
            [],
        )
        .ok();

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list_positions(&self, role_name: &str) -> Result<Vec<Position>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id,role_name,symbol,stock_name,entry_date,cost_price,shares,\
             current_price,stop_loss,target_price,entry_reason,sector,position_pct,\
             created_at,updated_at FROM positions WHERE role_name=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([role_name], |row| {
            Ok(Position {
                id: row.get(0)?,
                role_name: row.get(1)?,
                symbol: StockCode::new(row.get::<_, String>(2)?),
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn add_position(&self, pos: Position) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "INSERT INTO positions (role_name,symbol,stock_name,entry_date,cost_price,shares,\
             current_price,stop_loss,target_price,entry_reason,sector,position_pct)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                pos.role_name,
                pos.symbol.to_string(),
                pos.stock_name,
                pos.entry_date,
                pos.cost_price,
                pos.shares,
                pos.current_price,
                pos.stop_loss,
                pos.target_price,
                pos.entry_reason,
                pos.sector,
                pos.position_pct
            ],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO trade_logs (position_id,action,price,shares,reason,role_name,symbol,stock_name) VALUES (?1,'open',?2,?3,?4,?5,?6,?7) /* action=open via as_str */",
            rusqlite::params![id, pos.cost_price, pos.shares, pos.entry_reason, pos.role_name, pos.symbol.to_string(), pos.stock_name],
        )?;
        Ok(id)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn update_position(&self, pos: Position) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "UPDATE positions SET current_price=?1,stop_loss=?2,target_price=?3,\
             updated_at=datetime('now','localtime') WHERE id=?4",
            rusqlite::params![pos.current_price, pos.stop_loss, pos.target_price, pos.id],
        )?;
        Ok(())
    }

    pub fn close_position(&self, id: i64, price: f64, reason: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let (role_name, symbol, shares, stock_name): (String, String, i64, String) = conn
            .query_row(
                "SELECT role_name, symbol, shares, stock_name FROM positions WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        conn.execute(
            "INSERT INTO trade_logs (position_id,action,price,shares,reason,role_name,symbol,stock_name) VALUES (?1,'close',?2,?3,?4,?5,?6,?7)",
            rusqlite::params![id, price, shares, reason, role_name, symbol, stock_name],
        )?;
        conn.execute("DELETE FROM positions WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn get_trade_logs(&self, position_id: i64) -> Result<Vec<TradeLog>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id,position_id,action,price,shares,reason,created_at \
             FROM trade_logs WHERE position_id=?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([position_id], |row| {
            Ok(TradeLog {
                id: row.get(0)?,
                position_id: row.get(1)?,
                action: {
                    let a: String = row.get(2)?;
                    match a.as_str() {
                        "open" => TradeAction::Open,
                        "close" => TradeAction::Close,
                        _ => TradeAction::Buy,
                    }
                },
                price: row.get(3)?,
                shares: row.get(4)?,
                reason: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_trade_logs_by_role(
        &self,
        role_name: &str,
        limit: usize,
    ) -> Result<Vec<TradeLogEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT tl.id, tl.position_id, tl.action, tl.price, tl.shares, \
             tl.reason, tl.created_at, \
             CASE WHEN tl.symbol != '' THEN tl.symbol ELSE COALESCE(p.symbol, '') END as symbol \
             FROM trade_logs tl \
             LEFT JOIN positions p ON tl.position_id = p.id \
             WHERE tl.role_name = ?1 \
             ORDER BY tl.created_at DESC \
             LIMIT ?2",
        )?;
        let limit_i64: i64 = limit.try_into().unwrap_or(i64::MAX);
        let rows = stmt.query_map(rusqlite::params![role_name, limit_i64], |row| {
            Ok(TradeLogEntry {
                id: row.get(0)?,
                position_id: row.get(1)?,
                action: {
                    let a: String = row.get(2)?;
                    match a.as_str() {
                        "open" => TradeAction::Open,
                        "close" => TradeAction::Close,
                        _ => TradeAction::Buy,
                    }
                },
                price: row.get(3)?,
                shares: row.get(4)?,
                reason: row.get(5)?,
                created_at: row.get(6)?,
                symbol: StockCode::new(row.get::<_, String>(7)?),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn format_positions(&self, role_name: &str) -> Result<String> {
        let positions = self.list_positions(role_name)?;
        if positions.is_empty() {
            return Ok("暂无持仓".to_string());
        }
        let lines: Vec<String> = positions
            .iter()
            .map(|p| {
                let pnl = match p.current_price {
                    Some(cp) if cp > 0.0 => {
                        format!("{:+.1}%", (cp - p.cost_price) / p.cost_price * 100.0)
                    },
                    _ => "N/A".to_string(),
                };
                format!(
                    "- #{} {} {} | 成本{} | 现价{} | {} | {}股 | {} | {}买入",
                    p.id,
                    p.symbol,
                    p.stock_name,
                    p.cost_price,
                    p.current_price
                        .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
                    pnl,
                    p.shares,
                    p.sector,
                    p.entry_date
                )
            })
            .collect();
        Ok(lines.join("\n"))
    }

    pub fn list_all_positions(&self) -> Result<Vec<Position>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id,role_name,symbol,stock_name,entry_date,cost_price,shares,\
             current_price,stop_loss,target_price,entry_reason,sector,position_pct,\
             created_at,updated_at FROM positions ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Position {
                id: row.get(0)?,
                role_name: row.get(1)?,
                symbol: StockCode::new(row.get::<_, String>(2)?),
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_position_price(&self, id: i64, current_price: f64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "UPDATE positions SET current_price=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            rusqlite::params![current_price, id],
        )?;
        Ok(())
    }

    pub fn delete_positions_by_role(&self, role_name: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let n = conn.execute("DELETE FROM positions WHERE role_name=?1", [role_name])?;
        Ok(n)
    }

    pub fn delete_trade_logs_by_role(&self, role_name: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let n = conn.execute("DELETE FROM trade_logs WHERE role_name=?1", [role_name])?;
        Ok(n)
    }

    pub fn get_all_trade_logs_by_role(&self, role_name: &str) -> Result<Vec<TradeLogEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT tl.id, tl.position_id, tl.action, tl.price, tl.shares, \
             tl.reason, tl.created_at, \
             CASE WHEN tl.symbol != '' THEN tl.symbol ELSE COALESCE(p.symbol, '') END as symbol \
             FROM trade_logs tl \
             LEFT JOIN positions p ON tl.position_id = p.id \
             WHERE tl.role_name = ?1 \
             ORDER BY tl.created_at ASC",
        )?;
        let rows = stmt.query_map([role_name], |row| {
            Ok(TradeLogEntry {
                id: row.get(0)?,
                position_id: row.get(1)?,
                action: {
                    let a: String = row.get(2)?;
                    match a.as_str() {
                        "open" => TradeAction::Open,
                        "close" => TradeAction::Close,
                        _ => TradeAction::Buy,
                    }
                },
                price: row.get(3)?,
                shares: row.get(4)?,
                reason: row.get(5)?,
                created_at: row.get(6)?,
                symbol: StockCode::new(row.get::<_, String>(7)?),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_closed_positions(
        &self,
        role_name: &str,
        limit: usize,
    ) -> Result<Vec<ClosedPosition>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        // Pair Close logs with their corresponding Open logs by position_id
        let mut stmt = conn.prepare(
            "SELECT c.position_id, c.symbol, COALESCE(o.stock_name, c.stock_name, '') as stock_name,
             o.created_at as entry_date, c.created_at as exit_date,
             o.price as entry_price, c.price as exit_price,
             c.shares, o.reason as entry_reason, c.reason as exit_reason
             FROM trade_logs c
             JOIN trade_logs o ON o.position_id = c.position_id AND o.action = 'open'
             WHERE c.action = 'close' AND c.role_name = ?1
             ORDER BY c.created_at DESC
             LIMIT ?2",
        )?;
        let limit_i64: i64 = limit.try_into().unwrap_or(i64::MAX);
        let rows = stmt.query_map(rusqlite::params![role_name, limit_i64], |row| {
            let entry_price: f64 = row.get(5)?;
            let exit_price: f64 = row.get(6)?;
            let pnl_pct = if entry_price > 0.0 {
                (exit_price - entry_price) / entry_price * 100.0
            } else {
                0.0
            };
            Ok(ClosedPosition {
                position_id: row.get(0)?,
                symbol: StockCode::new(row.get::<_, String>(1)?),
                stock_name: row.get(2)?,
                entry_date: row.get::<_, String>(3)?.chars().take(10).collect(),
                exit_date: row.get::<_, String>(4)?.chars().take(10).collect(),
                entry_price,
                exit_price,
                shares: row.get(7)?,
                pnl_pct: (pnl_pct * 100.0).round() / 100.0,
                entry_reason: row.get(8)?,
                exit_reason: row.get(9)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::Position;

    fn test_pos(role: &str, symbol: &str, name: &str) -> Position {
        Position {
            id: 0,
            role_name: role.into(),
            symbol: StockCode::new(symbol),
            stock_name: name.into(),
            entry_date: "2026-01-15".into(),
            cost_price: 10.5,
            shares: 1000,
            current_price: Some(11.2),
            stop_loss: Some(9.8),
            target_price: Some(12.0),
            entry_reason: "测试".into(),
            sector: "银行".into(),
            position_pct: 0.3,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_add_and_list_positions() {
        let tm = TradeManager::open(Path::new(":memory:")).unwrap();
        tm.add_position(test_pos("zarili", "000001", "平安银行"))
            .unwrap();
        let positions = tm.list_positions("zarili").unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, StockCode::new("000001"));
    }

    #[test]
    fn test_close_position() {
        let tm = TradeManager::open(Path::new(":memory:")).unwrap();
        let id = tm
            .add_position(test_pos("trader", "600519", "茅台"))
            .unwrap();
        tm.close_position(id, 1700.0, "止盈").unwrap();
        assert!(tm.list_positions("trader").unwrap().is_empty());
        assert_eq!(tm.get_trade_logs(id).unwrap().len(), 2);
    }

    #[test]
    fn test_format_positions() {
        let tm = TradeManager::open(Path::new(":memory:")).unwrap();
        tm.add_position(test_pos("zarili", "000001", "平安银行"))
            .unwrap();
        let text = tm.format_positions("zarili").unwrap();
        assert!(text.contains("000001"));
        assert!(text.contains("平安银行"));
    }
}
