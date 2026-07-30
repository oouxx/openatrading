use lru::LruCache;
use pi_coding_agent::core::agent_session_runtime::{
    create_agent_session_runtime, AgentSessionRuntime, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeParams,
};
use pi_coding_agent::core::session_manager::{NewSessionOptions, SessionManager};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::runtime::find_session_file_by_id;

/// Default maximum number of sessions in the pool.
///
/// When the pool is full, the least-recently-used session is evicted.
/// Sessions are persisted to disk, so eviction only removes the in-memory
/// runtime — the session data remains on disk and can be resumed later.
const DEFAULT_POOL_CAPACITY: usize = 128;

/// Manages multiple concurrent LLM agent sessions with LRU eviction.
///
/// Each session is backed by a pi-coding-agent [`AgentSessionRuntime`] created
/// via `create_agent_session_runtime` + a factory. Sessions are identified by
/// UUID and stored in an LRU cache. When the cache is full, the least-recently-
/// accessed session is evicted (its in-memory runtime is dropped; session data
/// on disk is preserved and can be resumed later).
pub struct SessionPool {
    /// Active sessions with LRU eviction, keyed by session ID.
    sessions: Mutex<LruCache<String, Arc<Mutex<AgentSessionRuntime>>>>,
    /// Factory for creating new `AgentSessionRuntime` instances.
    factory: CreateAgentSessionRuntimeFactory,
}

impl SessionPool {
    pub fn new(factory: CreateAgentSessionRuntimeFactory) -> Self {
        Self {
            sessions: Mutex::new(LruCache::new(
                // SAFETY: DEFAULT_POOL_CAPACITY is a positive constant
                #[allow(clippy::unwrap_used)]
                NonZeroUsize::new(DEFAULT_POOL_CAPACITY).unwrap(),
            )),
            factory,
        }
    }

    /// Create a new session and return its ID.
    ///
    /// If `session_id` is `Some`, use that value instead of generating a new UUID.
    /// The caller is responsible for ensuring the ID is unique.
    pub async fn create_session(&self, cwd: &str) -> Result<String, String> {
        self.create_session_with_id(cwd, None).await
    }

    /// Create a new session with an optional custom ID.
    pub async fn create_session_with_id(
        &self,
        cwd: &str,
        session_id: Option<&str>,
    ) -> Result<String, String> {
        let session_id = session_id
            .unwrap_or(&uuid::Uuid::new_v4().to_string())
            .to_string();
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        // Pass the custom session ID via NewSessionOptions so pi-coding-agent
        // uses the same ID internally (session file naming, header, etc.).
        let new_session_opts = NewSessionOptions {
            id: Some(session_id.clone()),
            parent_session: None,
        };
        let session_manager =
            SessionManager::new(cwd, &session_dir, None, true, Some(new_session_opts));

        let runtime = create_agent_session_runtime(
            self.factory.clone(),
            CreateAgentSessionRuntimeParams {
                cwd: cwd.to_string(),
                agent_dir,
                session_manager,
                session_start_event: None,
            },
        )
        .await;

        let runtime = Arc::new(Mutex::new(runtime));
        self.sessions.lock().await.put(session_id.clone(), runtime);
        Ok(session_id)
    }

    /// Resume a session from disk by ID (for reconnection after pool miss).
    ///
    /// Finds the session's JSONL file on disk via `find_session_file_by_id`,
    /// creates a fresh `AgentSessionRuntime`, and imports the session state
    /// from the file. Returns an error if the session already exists in the
    /// pool or if no session file is found on disk.
    pub async fn resume_session(&self, session_id: &str) -> Result<(), String> {
        // Check for duplicate — drop lock before the !Send import_from_jsonl.
        {
            let sessions = self.sessions.lock().await;
            if sessions.contains(session_id) {
                return Err(format!("Session {session_id} already exists in pool"));
            }
        }

        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        let session_file = find_session_file_by_id(&session_dir, session_id)
            .ok_or_else(|| format!("No session file found for {session_id}"))?;

        // import_from_jsonl is !Send (holds std::sync::MutexGuard across .await),
        // so isolate the entire create+import via spawn_blocking.
        let factory = self.factory.clone();
        let session_dir2 = session_dir.clone();
        let agent_dir2 = agent_dir.clone();
        let session_file2 = session_file.clone();
        let sid = session_id.to_string();

        let runtime = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let session_manager = SessionManager::new("/tmp", &session_dir2, None, true, None);

                let mut runtime = create_agent_session_runtime(
                    factory,
                    CreateAgentSessionRuntimeParams {
                        cwd: "/tmp".to_string(),
                        agent_dir: agent_dir2,
                        session_manager,
                        session_start_event: None,
                    },
                )
                .await;

                runtime.import_from_jsonl(&session_file2, None).await?;

                Ok::<_, String>(runtime)
            })
        })
        .await
        .map_err(|e| format!("Blocking task failed: {e}"))?
        .map_err(|e| format!("Failed to import session: {e}"))?;

        let runtime = Arc::new(Mutex::new(runtime));
        self.sessions.lock().await.put(sid, runtime);
        Ok(())
    }

    /// Get a reference to a session runtime by ID.
    ///
    /// Accessing a session promotes it to the front of the LRU cache.
    pub async fn get_session(&self, id: &str) -> Option<Arc<Mutex<AgentSessionRuntime>>> {
        self.sessions.lock().await.get(id).cloned()
    }

    /// Get a session from the pool, or resume it from disk if not found.
    ///
    /// This combines `get_session()` + `resume_session()` into a single call,
    /// returning an error if the session doesn't exist on disk either.
    pub async fn get_or_resume(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<AgentSessionRuntime>>, String> {
        if let Some(rt) = self.get_session(session_id).await {
            return Ok(rt);
        }
        self.resume_session(session_id).await?;
        self.get_session(session_id)
            .await
            .ok_or_else(|| "Session not found after resume".to_string())
    }

    /// Send a prompt to a session (streaming output).
    pub async fn prompt(&self, session_id: &str, text: &str) -> Result<(), String> {
        let session = self
            .sessions
            .lock()
            .await
            .get(session_id)
            .ok_or_else(|| "Session not found".to_string())?
            .clone();
        let rt = session.lock().await;
        rt.session().prompt(text, None).await;
        Ok(())
    }

    /// Send a prompt with full options, handling `!Send` isolation internally.
    ///
    /// `prompt()` on `AgentSession` is `!Send` (holds `std::sync::MutexGuard`
    /// across `.await`), so we isolate it via `spawn_blocking`.
    pub async fn prompt_with_opts(
        &self,
        session_id: &str,
        text: &str,
        opts: pi_coding_agent::core::agent_session::PromptOptions,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .lock()
            .await
            .get(session_id)
            .ok_or_else(|| "Session not found".to_string())?
            .clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async {
                let rt = session.lock().await;
                rt.session().prompt(&text, Some(opts)).await;
            });
        })
        .await
        .map_err(|e| format!("Blocking task failed: {e}"))?;
        Ok(())
    }

    /// Run a prompt and return the last assistant text.
    ///
    /// This is a convenience for one-shot sessions (e.g. cron jobs, social bots)
    /// that create a session, run one prompt, get the result, and discard.
    ///
    /// `prompt()` on `AgentSession` is `!Send` (holds `std::sync::MutexGuard`
    /// across `.await`), so we isolate it via `spawn_blocking`.
    pub async fn prompt_once(&self, cwd: &str, text: &str) -> Result<String, String> {
        let session_id = self.create_session(cwd).await?;
        let session = self
            .get_session(&session_id)
            .await
            .ok_or_else(|| "Session not found after creation".to_string())?;

        let msg = text.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let rt = session.lock().await;
                let s = rt.session();
                s.prompt(&msg, None).await;
                s.get_last_assistant_text()
                    .await
                    .ok_or_else(|| "No assistant response".to_string())
            })
        })
        .await
        .map_err(|e| format!("Blocking task failed: {e}"))?;

        self.close_session(&session_id).await;
        result
    }

    /// Close and remove a session.
    pub async fn close_session(&self, session_id: &str) {
        self.sessions.lock().await.pop(session_id);
    }

    /// List all active session IDs.
    pub async fn list_session_ids(&self) -> Vec<String> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Check if a session exists in the pool.
    pub async fn has_session(&self, session_id: &str) -> bool {
        self.sessions.lock().await.contains(session_id)
    }

    /// Check whether a session is currently streaming a response.
    ///
    /// Returns `false` if the session does not exist in the pool.
    pub async fn is_stream(&self, session_id: &str) -> bool {
        let Some(session) = self.sessions.lock().await.get(session_id).cloned() else {
            return false;
        };
        let rt = session.lock().await;
        rt.session().is_streaming().await
    }
}
