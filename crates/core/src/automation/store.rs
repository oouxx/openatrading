//! 自动化任务的 SQLite 持久化:任务定义 + 运行记录(收件箱)。
//!
//! 与 [`crate::memory::sqlite::MemoryStore`] 同一模式:共享 db 文件,
//! WAL 模式,`CREATE TABLE IF NOT EXISTS` 幂等建表。

use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use rusqlite::Connection;

use super::model::{
    validate, Automation, AutomationId, AutomationPatch, NewAutomation, NewRun, RunStatus,
    Schedule, Scope,
};

/// 运行记录保留天数。
const RUN_RETENTION_DAYS: u32 = 30;

/// 一条运行记录(收件箱条目)。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AutomationRun {
    pub id: i64,
    pub automation_id: AutomationId,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: RunStatus,
    /// 最终助手回复;成功但为空 = 无可报告内容(自动归档)。
    pub output: Option<String>,
    pub error: Option<String>,
    pub unread: bool,
    pub archived: bool,
}

/// 运行记录查询条件。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct RunListQuery {
    pub automation_id: Option<AutomationId>,
    /// 仅未读。
    pub unread_only: bool,
    /// 仅未归档(Codex 收件箱视图)。
    pub active_only: bool,
    pub limit: u32,
}

/// 自动化任务存储。
pub struct AutomationStore {
    conn: Mutex<Connection>,
}

impl AutomationStore {
    /// 打开(或创建)自动化表;`db_path` 与 MemoryStore/TradeManager 同一数据库文件。
    ///
    /// # Errors
    /// 数据库无法打开或建表失败时返回错误。
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS automations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                prompt TEXT NOT NULL,
                schedule_json TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                cwd TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );
            CREATE TABLE IF NOT EXISTS automation_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                automation_id TEXT NOT NULL,
                started_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                finished_at TEXT,
                status TEXT NOT NULL,
                output TEXT,
                error TEXT,
                unread INTEGER NOT NULL DEFAULT 1,
                archived INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_runs_auto
                ON automation_runs(automation_id, started_at DESC);",
        )?;
        // 增量迁移:老库补列(列已存在时 ALTER 报错,忽略即可)。
        for column in [
            "read_only INTEGER NOT NULL DEFAULT 0",
            "timeout_secs INTEGER",
        ] {
            let sql = format!("ALTER TABLE automations ADD COLUMN {column}");
            if let Err(e) = conn.execute_batch(&sql) {
                crate::log::debug!("[automation] schema migrate skipped: {e}");
            }
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 新建任务,返回完整定义(id/时间戳在此生成)。
    ///
    /// # Errors
    /// 业务校验失败或数据库写入失败。
    pub fn create(&self, draft: &NewAutomation) -> Result<Automation> {
        let schedule_json = serde_json::to_string(&draft.schedule)
            .map_err(|e| anyhow::anyhow!("serialize schedule: {e}"))?;
        let scope_json = serde_json::to_string(&draft.scope)
            .map_err(|e| anyhow::anyhow!("serialize scope: {e}"))?;
        let id = AutomationId::generate();

        validate(
            &draft.name,
            &draft.prompt,
            &draft.schedule,
            &draft.scope,
            draft.timeout_secs,
        )
        .map_err(anyhow::Error::msg)?;

        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO automations
                (id, name, prompt, schedule_json, scope_json, cwd, enabled, read_only, timeout_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
            rusqlite::params![
                id.0,
                draft.name.trim(),
                draft.prompt,
                schedule_json,
                scope_json,
                draft.cwd,
                i64::from(draft.read_only),
                draft.timeout_secs.map(|t| t as i64),
            ],
        )?;
        drop(conn);
        self.get(&id)?
            .ok_or_else(|| anyhow::anyhow!("automation {id} missing right after insert"))
    }

    /// 按补丁更新任务,返回更新后的定义。
    ///
    /// # Errors
    /// 任务不存在或数据库写入失败。
    pub fn update(&self, id: &AutomationId, patch: &AutomationPatch) -> Result<Automation> {
        let schedule_json = patch
            .schedule
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| anyhow::anyhow!("serialize schedule: {e}"))?;
        let scope_json = patch
            .scope
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| anyhow::anyhow!("serialize scope: {e}"))?;

        // 未提到的字段用现有值占位;业务校验用"更新后"的完整定义做。
        let current = self
            .get(id)?
            .ok_or_else(|| anyhow::anyhow!("automation {id} not found"))?;
        let merged = Automation {
            read_only: patch.read_only.unwrap_or(current.read_only),
            timeout_secs: patch.timeout_secs.or(current.timeout_secs),
            name: patch.name.clone().unwrap_or_else(|| current.name.clone()),
            prompt: patch
                .prompt
                .clone()
                .unwrap_or_else(|| current.prompt.clone()),
            schedule: patch
                .schedule
                .clone()
                .unwrap_or_else(|| current.schedule.clone()),
            scope: patch.scope.clone().unwrap_or_else(|| current.scope.clone()),
            cwd: patch.cwd.clone().unwrap_or_else(|| current.cwd.clone()),
            enabled: patch.enabled.unwrap_or(current.enabled),
            ..current
        };
        validate(
            &merged.name,
            &merged.prompt,
            &merged.schedule,
            &merged.scope,
            merged.timeout_secs,
        )
        .map_err(anyhow::Error::msg)?;

        let conn = self.lock_conn()?;
        let changed = conn.execute(
            "UPDATE automations SET
                name = COALESCE(?2, name),
                prompt = COALESCE(?3, prompt),
                schedule_json = COALESCE(?4, schedule_json),
                scope_json = COALESCE(?5, scope_json),
                cwd = COALESCE(?6, cwd),
                enabled = COALESCE(?7, enabled),
                read_only = COALESCE(?8, read_only),
                timeout_secs = COALESCE(?9, timeout_secs),
                updated_at = datetime('now','localtime')
             WHERE id = ?1",
            rusqlite::params![
                id.0,
                patch.name.as_deref().map(str::trim),
                patch.prompt.as_deref(),
                schedule_json,
                scope_json,
                patch.cwd.as_deref(),
                patch.enabled,
                patch.read_only,
                patch.timeout_secs.map(|t| t as i64),
            ],
        )?;
        drop(conn);
        if changed == 0 {
            anyhow::bail!("automation {id} not found");
        }
        self.get(id)?
            .ok_or_else(|| anyhow::anyhow!("automation {id} missing after update"))
    }

    /// 删除任务(运行记录保留,作为历史)。
    ///
    /// # Errors
    /// 任务不存在或数据库写入失败。
    pub fn delete(&self, id: &AutomationId) -> Result<()> {
        let conn = self.lock_conn()?;
        let changed = conn.execute("DELETE FROM automations WHERE id = ?1", [id.0.as_str()])?;
        drop(conn);
        if changed == 0 {
            anyhow::bail!("automation {id} not found");
        }
        Ok(())
    }

    /// 读取单个任务。
    ///
    /// # Errors
    /// 数据库读取失败或序列化字段损坏。
    pub fn get(&self, id: &AutomationId) -> Result<Option<Automation>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, prompt, schedule_json, scope_json, cwd, enabled, read_only,
                    timeout_secs, created_at, updated_at
             FROM automations WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id.0.as_str()], row_to_automation)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 列出全部任务(按创建时间倒序)。
    ///
    /// # Errors
    /// 数据库读取失败或序列化字段损坏。
    pub fn list(&self) -> Result<Vec<Automation>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, prompt, schedule_json, scope_json, cwd, enabled, read_only,
                    timeout_secs, created_at, updated_at
             FROM automations ORDER BY created_at DESC, id",
        )?;
        let rows = stmt.query_map([], row_to_automation)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 记录一次运行开始(status = running,视为未读)。
    ///
    /// # Errors
    /// 数据库写入失败。
    pub fn start_run(&self, automation_id: &AutomationId) -> Result<i64> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO automation_runs (automation_id, status, unread) VALUES (?1, 'running', 1)",
            rusqlite::params![automation_id.0],
        )?;
        let run_id = conn.last_insert_rowid();
        drop(conn);
        self.cleanup_old_runs()?;
        Ok(run_id)
    }

    /// 完成一次运行,并应用收件箱语义:
    /// 成功且有产出 → 未读;成功但无产出 → 直接归档不打扰;失败 → 未读。
    ///
    /// # Errors
    /// 运行记录不存在或数据库写入失败。
    pub fn finish_run(&self, run_id: i64, run: &NewRun) -> Result<()> {
        let (unread, archived) = inbox_flags(run);
        let conn = self.lock_conn()?;
        let changed = conn.execute(
            "UPDATE automation_runs SET
                finished_at = datetime('now','localtime'),
                status = ?2, output = ?3, error = ?4, unread = ?5, archived = ?6
             WHERE id = ?1",
            rusqlite::params![
                run_id,
                run.status.as_str(),
                run.output.as_deref(),
                run.error.as_deref(),
                unread,
                archived
            ],
        )?;
        drop(conn);
        if changed == 0 {
            anyhow::bail!("automation run {run_id} not found");
        }
        Ok(())
    }

    /// 回收滞留的 running 记录:超过阈值仍无 finished_at 的视为进程中断,置为失败。
    ///
    /// # Errors
    /// 数据库写入失败。
    pub fn reap_stale_runs(&self, older_than_secs: i64) -> Result<usize> {
        let conn = self.lock_conn()?;
        let changed = conn.execute(
            "UPDATE automation_runs SET
                finished_at = datetime('now','localtime'),
                status = 'failed',
                error = 'run interrupted (stale running row)'
             WHERE status = 'running'
               AND started_at < datetime('now','localtime', ?1)",
            rusqlite::params![format!("-{older_than_secs} seconds")],
        )?;
        Ok(changed)
    }

    /// 按条件列出运行记录(新→旧)。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn list_runs(&self, query: &RunListQuery) -> Result<Vec<AutomationRun>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, automation_id, started_at, finished_at, status, output, error, unread, archived
             FROM automation_runs
             WHERE (?1 IS NULL OR automation_id = ?1)
               AND (?2 = 0 OR unread = 1)
               AND (?3 = 0 OR archived = 0)
             ORDER BY started_at DESC, id DESC
             LIMIT ?4",
        )?;
        let limit = if query.limit == 0 { 50 } else { query.limit };
        let rows = stmt.query_map(
            rusqlite::params![
                query.automation_id.as_ref().map(|id| id.0.as_str()),
                i64::from(query.unread_only),
                i64::from(query.active_only),
                limit,
            ],
            |row| {
                let status_str: String = row.get(4)?;
                Ok(AutomationRun {
                    id: row.get(0)?,
                    automation_id: AutomationId(row.get(1)?),
                    started_at: row.get(2)?,
                    finished_at: row.get(3)?,
                    status: RunStatus::from_str(&status_str).ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            format!("unknown run status '{status_str}'").into(),
                        )
                    })?,
                    output: row.get(5)?,
                    error: row.get(6)?,
                    unread: row.get::<_, i64>(7)? != 0,
                    archived: row.get::<_, i64>(8)? != 0,
                })
            },
        )?;
        let out = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into);
        drop(stmt);
        drop(conn);
        out
    }

    /// 标记运行记录已读。
    ///
    /// # Errors
    /// 记录不存在或数据库写入失败。
    pub fn mark_read(&self, run_id: i64) -> Result<()> {
        self.update_run_flag(run_id, "unread", 0)
    }

    /// 设置运行记录归档状态。
    ///
    /// # Errors
    /// 记录不存在或数据库写入失败。
    pub fn set_archived(&self, run_id: i64, archived: bool) -> Result<()> {
        self.update_run_flag(run_id, "archived", i64::from(archived))
    }

    fn update_run_flag(&self, run_id: i64, column: &str, value: i64) -> Result<()> {
        // column 来自内部调用点(白名单),非用户输入。
        let sql = format!("UPDATE automation_runs SET {column} = ?1 WHERE id = ?2");
        let conn = self.lock_conn()?;
        let changed = conn.execute(&sql, rusqlite::params![value, run_id])?;
        drop(conn);
        if changed == 0 {
            anyhow::bail!("automation run {run_id} not found");
        }
        Ok(())
    }

    /// 任务表变更指纹(行数 + 最大 updated_at)。
    ///
    /// 用于调度器周期性 reconcile:跨进程(tauri 桌面端)的 CRUD 无法
    /// 直接触发本进程 reload,靠指纹比对感知变更。
    ///
    /// # Errors
    /// 数据库读取失败。
    pub fn fingerprint(&self) -> Result<String> {
        let conn = self.lock_conn()?;
        let mut stmt =
            conn.prepare("SELECT COUNT(*), COALESCE(MAX(updated_at), '') FROM automations")?;
        let (count, max_updated): (i64, String) =
            stmt.query_row([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(format!("{count}:{max_updated}"))
    }

    /// 清理超期运行记录。
    ///
    /// # Errors
    /// 数据库写入失败。
    pub fn cleanup_old_runs(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM automation_runs WHERE started_at < datetime('now','localtime', ?1)",
            [format!("-{RUN_RETENTION_DAYS} days")],
        )?;
        Ok(())
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("automation store mutex poisoned: {e}"))
    }
}

/// 收件箱语义:成功且有产出 → 未读;成功但无产出 → 自动归档;失败 → 未读。
fn inbox_flags(run: &NewRun) -> (i64, i64) {
    match run.status {
        RunStatus::Success => {
            let has_findings = run.output.as_deref().is_some_and(|o| !o.trim().is_empty());
            if has_findings {
                (1, 0)
            } else {
                (0, 1)
            }
        }
        RunStatus::Running | RunStatus::Failed => (1, 0),
    }
}

fn row_to_automation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Automation> {
    let schedule_json: String = row.get(3)?;
    let scope_json: String = row.get(4)?;
    let schedule: Schedule = serde_json::from_str(&schedule_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            format!("invalid schedule_json: {e}").into(),
        )
    })?;
    let scope: Scope = serde_json::from_str(&scope_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            format!("invalid scope_json: {e}").into(),
        )
    })?;
    let timeout_secs: Option<i64> = row.get(8)?;
    Ok(Automation {
        id: AutomationId(row.get(0)?),
        name: row.get(1)?,
        prompt: row.get(2)?,
        schedule,
        scope,
        cwd: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        read_only: row.get::<_, i64>(7)? != 0,
        timeout_secs: timeout_secs.map(|t| t as u64),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}
