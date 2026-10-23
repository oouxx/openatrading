use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::replay::ReplayStore;
use super::MarketSnapshot;

pub struct MemoryStore {
    conn: Arc<Mutex<Connection>>,
    replay: ReplayStore,
}

impl MemoryStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Arc::new(Mutex::new(Connection::open(db_path)?));
        {
            let guard = conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
            guard.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
            guard.execute_batch(
                "CREATE TABLE IF NOT EXISTS market_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    captured_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    data_source TEXT NOT NULL,
                    raw_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_snapshots_date ON market_snapshots(captured_at);
                CREATE INDEX IF NOT EXISTS idx_snapshots_source ON market_snapshots(data_source);",
            )?;
        }
        let replay = ReplayStore::new(Arc::clone(&conn))?;
        Ok(Self { conn, replay })
    }

    /// 行情回放事件的持久化入口。
    pub fn replay(&self) -> &ReplayStore {
        &self.replay
    }

    pub fn save_snapshot(&self, source: &str, json: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
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
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "DELETE FROM market_snapshots WHERE captured_at < datetime('now', 'localtime', '-10 days')",
            [],
        )?;
        Ok(())
    }
}
