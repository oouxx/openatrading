use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use super::MarketSnapshot;

pub struct MemoryStore {
    conn: Mutex<Connection>,
}

impl MemoryStore {
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
        Ok(MemoryStore {
            conn: Mutex::new(conn),
        })
    }

    pub fn save_snapshot(&self, source: &str, json: &str) -> Result<i64> {
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

    pub fn query_recent(&self, days: u32) -> Result<Vec<MarketSnapshot>> {
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn query_by_source(&self, source: &str, days: u32) -> Result<Vec<MarketSnapshot>> {
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn cleanup_old(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM market_snapshots WHERE captured_at < datetime('now', 'localtime', '-10 days')",
            [],
        )?;
        Ok(())
    }

    pub fn get_historical_context(&self, days: u32) -> Result<String> {
        let snapshots = self.query_recent(days)?;
        if snapshots.is_empty() {
            return Ok("暂无历史数据".to_string());
        }
        let mut ctx = format!("## 近{days}日市场数据快照 ({}条记录)\n\n", snapshots.len());
        for s in &snapshots {
            ctx.push_str(&format!(
                "- {} | {} | {}\n",
                s.captured_at, s.data_source, &s.raw_json
            ));
        }
        Ok(ctx)
    }
}
