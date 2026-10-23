//! 行情回放事件持久化。
//!
//! 事件是追加型数据：盘中可以增量写入，收盘后再做一次全量对账并标记
//! [`ReplayDayStatus::Complete`]。这样既不会因为收盘任务失败丢掉一整天，
//! 也能保证跨日分析只使用完整交易日。

use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::api::xuangubao::ReplayEvent;

/// 回放日的采集状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayDayStatus {
    /// 盘中增量数据，或者收盘后尚未完成全量对账。
    Partial,
    /// 收盘后已校验覆盖 09:30–15:00 的完整数据。
    Complete,
}

impl ReplayDayStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Partial => "partial",
            Self::Complete => "complete",
        }
    }

    fn from_db(value: &str) -> Self {
        if value == Self::Complete.as_str() {
            Self::Complete
        } else {
            Self::Partial
        }
    }
}

/// 某个交易日的回放元信息。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayDay {
    pub date: String,
    pub status: ReplayDayStatus,
    pub event_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_event_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_timestamp: Option<i64>,
    pub captured_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// 回放事件存储。与 [`crate::memory::sqlite::MemoryStore`] 共用同一个 SQLite 连接。
pub struct ReplayStore {
    conn: Arc<Mutex<Connection>>,
}

impl ReplayStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        {
            let guard = conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
            guard.execute_batch(
                "CREATE TABLE IF NOT EXISTS replay_events (
                    trade_date TEXT NOT NULL,
                    event_id INTEGER NOT NULL,
                    event_type INTEGER NOT NULL,
                    event_timestamp INTEGER NOT NULL,
                    good_or_bad INTEGER NOT NULL,
                    target TEXT NOT NULL DEFAULT '',
                    plate_name TEXT,
                    stock_name TEXT,
                    symbol TEXT,
                    pcp REAL,
                    related_stocks_json TEXT NOT NULL DEFAULT '[]',
                    first_seen_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    last_seen_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    PRIMARY KEY (trade_date, event_id)
                );
                CREATE INDEX IF NOT EXISTS idx_replay_events_date_ts
                    ON replay_events(trade_date, event_timestamp);
                CREATE TABLE IF NOT EXISTS replay_days (
                    trade_date TEXT PRIMARY KEY,
                    status TEXT NOT NULL,
                    event_count INTEGER NOT NULL DEFAULT 0,
                    first_event_ts INTEGER,
                    last_event_ts INTEGER,
                    captured_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                    completed_at TEXT,
                    index_json TEXT
                );",
            )?;
        }
        Ok(Self { conn })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))
    }

    /// 增量写入事件（按 `(trade_date, event_id)` 幂等）。
    ///
    /// 盘中采集走这里；事件不会因为收盘而失效，所以可以随时写。
    ///
    /// # Errors
    /// 数据库写入或序列化关联股票失败。
    pub fn upsert_events(&self, date: &str, events: &[ReplayEvent]) -> Result<()> {
        let mut guard = self.lock()?;
        let tx = guard.transaction()?;
        insert_events(&tx, date, events)?;
        tx.commit()?;
        Ok(())
    }

    /// 用一份完整结果替换某天的事件（收盘对账）。
    ///
    /// 只有确认分页已覆盖开盘时间后才应调用，避免用残缺结果覆盖盘中数据。
    ///
    /// # Errors
    /// 数据库写入或序列化关联股票失败。
    pub fn replace_events(&self, date: &str, events: &[ReplayEvent]) -> Result<()> {
        let mut guard = self.lock()?;
        let tx = guard.transaction()?;
        tx.execute(
            "DELETE FROM replay_events WHERE trade_date = ?1",
            params![date],
        )?;
        insert_events(&tx, date, events)?;
        tx.commit()?;
        Ok(())
    }

    /// 读取某天的全部事件，按时间升序。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn events(&self, date: &str) -> Result<Vec<ReplayEvent>> {
        let guard = self.lock()?;
        let mut stmt = guard.prepare(
            "SELECT event_id, event_type, event_timestamp, good_or_bad, target,
                    plate_name, stock_name, symbol, pcp, related_stocks_json
             FROM replay_events
             WHERE trade_date = ?1
             ORDER BY event_timestamp ASC, event_id ASC",
        )?;
        let rows = stmt.query_map(params![date], |row| {
            let related_json: String = row.get(9)?;
            let related_stocks = serde_json::from_str(&related_json).unwrap_or_else(|e| {
                crate::log::warn!("[replay] related_stocks JSON 解析失败: {e}");
                Vec::new()
            });
            Ok(ReplayEvent {
                id: row.get(0)?,
                event_type: row.get(1)?,
                event_timestamp: row.get(2)?,
                good_or_bad: row.get(3)?,
                target: row.get(4)?,
                plate_name: row.get(5)?,
                stock_name: row.get(6)?,
                symbol: row.get(7)?,
                pcp: row.get(8)?,
                related_stocks,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 读取某天的元信息。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn day(&self, date: &str) -> Result<Option<ReplayDay>> {
        let guard = self.lock()?;
        let mut stmt = guard.prepare(
            "SELECT trade_date, status, event_count, first_event_ts, last_event_ts,
                    captured_at, completed_at
             FROM replay_days WHERE trade_date = ?1",
        )?;
        stmt.query_row(params![date], map_day)
            .optional()
            .map_err(Into::into)
    }

    /// 最近 `limit` 个已采集的交易日，最新在前。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn days(&self, limit: u32) -> Result<Vec<ReplayDay>> {
        let guard = self.lock()?;
        let mut stmt = guard.prepare(
            "SELECT trade_date, status, event_count, first_event_ts, last_event_ts,
                    captured_at, completed_at
             FROM replay_days
             ORDER BY trade_date DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![i64::from(limit)], map_day)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 写入/更新某天的元信息。
    ///
    /// # Errors
    /// 数据库写入失败。
    pub fn save_day(&self, day: &ReplayDay) -> Result<()> {
        let guard = self.lock()?;
        guard.execute(
            "INSERT INTO replay_days
                (trade_date, status, event_count, first_event_ts, last_event_ts,
                 captured_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(trade_date) DO UPDATE SET
                status = excluded.status,
                event_count = excluded.event_count,
                first_event_ts = excluded.first_event_ts,
                last_event_ts = excluded.last_event_ts,
                captured_at = excluded.captured_at,
                completed_at = excluded.completed_at",
            params![
                day.date,
                day.status.as_str(),
                i64::from(day.event_count),
                day.first_event_timestamp,
                day.last_event_timestamp,
                day.captured_at,
                day.completed_at,
            ],
        )?;
        Ok(())
    }

    /// 保存指数分时 JSON（只在采集当天写入，历史日期通常为空）。
    ///
    /// # Errors
    /// 数据库写入失败。
    pub fn set_index_json(&self, date: &str, json: &str) -> Result<()> {
        let guard = self.lock()?;
        guard.execute(
            "UPDATE replay_days SET index_json = ?2 WHERE trade_date = ?1",
            params![date, json],
        )?;
        Ok(())
    }

    /// 读取指数分时 JSON。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn index_json(&self, date: &str) -> Result<Option<String>> {
        let guard = self.lock()?;
        let mut stmt = guard.prepare("SELECT index_json FROM replay_days WHERE trade_date = ?1")?;
        stmt.query_row(params![date], |row| row.get(0))
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }
}

fn insert_events(tx: &rusqlite::Transaction<'_>, date: &str, events: &[ReplayEvent]) -> Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO replay_events
            (trade_date, event_id, event_type, event_timestamp, good_or_bad, target,
             plate_name, stock_name, symbol, pcp, related_stocks_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(trade_date, event_id) DO UPDATE SET
            event_type = excluded.event_type,
            event_timestamp = excluded.event_timestamp,
            good_or_bad = excluded.good_or_bad,
            target = excluded.target,
            plate_name = excluded.plate_name,
            stock_name = excluded.stock_name,
            symbol = excluded.symbol,
            pcp = excluded.pcp,
            related_stocks_json = excluded.related_stocks_json,
            last_seen_at = datetime('now','localtime')",
    )?;
    for event in events {
        let related_json = serde_json::to_string(&event.related_stocks)?;
        stmt.execute(params![
            date,
            event.id,
            event.event_type,
            event.event_timestamp,
            event.good_or_bad,
            event.target,
            event.plate_name,
            event.stock_name,
            event.symbol,
            event.pcp,
            related_json,
        ])?;
    }
    Ok(())
}

fn map_day(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReplayDay> {
    let status: String = row.get(1)?;
    let event_count: i64 = row.get(2)?;
    Ok(ReplayDay {
        date: row.get(0)?,
        status: ReplayDayStatus::from_db(&status),
        event_count: u32::try_from(event_count).unwrap_or(u32::MAX),
        first_event_timestamp: row.get(3)?,
        last_event_timestamp: row.get(4)?,
        captured_at: row.get(5)?,
        completed_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::xuangubao::{RelatedStock, ReplayEvent};
    use crate::memory::MemoryStore;
    use tempfile::tempdir;

    fn plate_event(id: i64, timestamp: i64, name: &str, pcp: f64) -> ReplayEvent {
        ReplayEvent {
            id,
            event_type: 11_000,
            event_timestamp: timestamp,
            good_or_bad: 1,
            target: name.to_string(),
            plate_name: Some(name.to_string()),
            stock_name: None,
            symbol: None,
            pcp: Some(pcp),
            related_stocks: vec![RelatedStock {
                symbol: "600519.SS".to_string(),
                name: "贵州茅台".to_string(),
                mtm: 1.0,
                pcp,
            }],
        }
    }

    fn stock_event(id: i64, timestamp: i64) -> ReplayEvent {
        ReplayEvent {
            id,
            event_type: 11_001,
            event_timestamp: timestamp,
            good_or_bad: 2,
            target: "300750.SZ".to_string(),
            plate_name: None,
            stock_name: Some("宁德时代".to_string()),
            symbol: Some("300750.SZ".to_string()),
            pcp: Some(-0.018),
            related_stocks: Vec::new(),
        }
    }

    fn sample_day() -> ReplayDay {
        ReplayDay {
            date: "2026-10-22".to_string(),
            status: ReplayDayStatus::Partial,
            event_count: 2,
            first_event_timestamp: Some(1_000),
            last_event_timestamp: Some(2_000),
            captured_at: "2026-10-22 14:30:00".to_string(),
            completed_at: None,
        }
    }

    #[test]
    fn replay_store_round_trip_and_replace() -> Result<()> {
        let dir = tempdir()?;
        let memory = MemoryStore::open(&dir.path().join("data.db"))?;
        let store = memory.replay();

        store.upsert_events(
            "2026-10-22",
            &[plate_event(1, 1_000, "算力", 0.031), stock_event(2, 2_000)],
        )?;
        let events = store.events("2026-10-22")?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].plate_name.as_deref(), Some("算力"));
        assert_eq!(events[0].related_stocks[0].symbol, "600519.SS");
        assert_eq!(events[1].stock_name.as_deref(), Some("宁德时代"));
        assert_eq!(events[1].symbol.as_deref(), Some("300750.SZ"));

        store.save_day(&sample_day())?;
        let day = store
            .day("2026-10-22")?
            .ok_or_else(|| anyhow::anyhow!("day should exist"))?;
        assert_eq!(day.status, ReplayDayStatus::Partial);
        assert_eq!(day.event_count, 2);
        assert_eq!(day.first_event_timestamp, Some(1_000));
        assert!(day.completed_at.is_none());

        store.set_index_json("2026-10-22", "{\"sh\":[],\"sz\":[],\"cyb\":[]}")?;
        assert_eq!(
            store.index_json("2026-10-22")?.as_deref(),
            Some("{\"sh\":[],\"sz\":[],\"cyb\":[]}")
        );

        store.replace_events("2026-10-22", &[plate_event(1, 1_000, "算力", 0.031)])?;
        let replaced = store.events("2026-10-22")?;
        assert_eq!(replaced.len(), 1);
        assert_eq!(replaced[0].id, 1);
        Ok(())
    }

    #[test]
    fn days_are_sorted_desc() -> Result<()> {
        let dir = tempdir()?;
        let memory = MemoryStore::open(&dir.path().join("data.db"))?;
        let store = memory.replay();
        for date in ["2026-10-20", "2026-10-22", "2026-10-21"] {
            let mut day = sample_day();
            day.date = date.to_string();
            store.save_day(&day)?;
        }
        let days = store.days(2)?;
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "2026-10-22");
        assert_eq!(days[1].date, "2026-10-21");
        Ok(())
    }
}
