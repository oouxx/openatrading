use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// A chat thread (session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatThread {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A single message within a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub thread_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// Request body for creating a new thread.
#[derive(Debug, Deserialize)]
pub struct CreateThreadRequest {
    pub title: Option<String>,
}

/// Request body for renaming a thread.
#[derive(Debug, Deserialize)]
pub struct RenameThreadRequest {
    pub title: String,
}

/// Request body for saving a message.
#[derive(Debug, Deserialize)]
pub struct SaveMessageRequest {
    pub role: String,
    pub content: String,
}

pub struct ChatThreadStore {
    conn: Mutex<Connection>,
}

impl ChatThreadStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("无法打开 SQLite: {}", db_path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chat_threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT '新对话',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS chat_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                thread_id INTEGER NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE INDEX IF NOT EXISTS idx_chat_messages_thread
                ON chat_messages(thread_id, created_at);",
        )?;

        Ok(ChatThreadStore {
            conn: Mutex::new(conn),
        })
    }

    /// List all threads, most recent first.
    pub fn list_threads(&self) -> Result<Vec<ChatThread>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at FROM chat_threads ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatThread {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Create a new thread, returns the thread id.
    pub fn create_thread(&self, title: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        conn.execute(
            "INSERT INTO chat_threads (title) VALUES (?1)",
            rusqlite::params![title],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Rename a thread.
    pub fn rename_thread(&self, id: i64, title: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        conn.execute(
            "UPDATE chat_threads SET title = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
            rusqlite::params![title, id],
        )?;
        Ok(())
    }

    /// Delete a thread and all its messages.
    pub fn delete_thread(&self, id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        conn.execute(
            "DELETE FROM chat_messages WHERE thread_id = ?1",
            rusqlite::params![id],
        )?;
        conn.execute(
            "DELETE FROM chat_threads WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    /// Get messages for a thread, oldest first.
    pub fn get_messages(&self, thread_id: i64) -> Result<Vec<ChatMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, role, content, created_at FROM chat_messages \
             WHERE thread_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![thread_id], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Save a message to a thread.
    pub fn save_message(&self, thread_id: i64, role: &str, content: &str) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))?;
        conn.execute(
            "INSERT INTO chat_messages (thread_id, role, content) VALUES (?1, ?2, ?3)",
            rusqlite::params![thread_id, role, content],
        )?;
        let msg_id = conn.last_insert_rowid();
        // Touch updated_at on the thread
        conn.execute(
            "UPDATE chat_threads SET updated_at = datetime('now','localtime') WHERE id = ?1",
            rusqlite::params![thread_id],
        )?;
        Ok(msg_id)
    }
}
