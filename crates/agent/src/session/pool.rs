//! Session pool: supervisor for per-session actors.
//!
//! Each session is backed by a [`SessionHandle`] (mailbox to a dedicated
//! actor thread — see [`crate::session::actor`]). The pool owns the handle
//! map with LRU eviction: when the pool is full, the least-recently-used
//! actor is sent `Shutdown` and its thread exits. Sessions are persisted to
//! disk, so eviction only removes the in-memory actor — the session data
//! remains on disk and can be resumed later.
//!
//! Per-session [`SessionEventState`] (monotonic `seq` + ring buffer) is kept
//! *outside* the actor so numbering survives actor eviction: a later resume
//! reuses the same state and reconnect-replay keeps working.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use pi_coding_agent::core::agent_session::PromptOptions;
use pi_coding_agent::core::sdk::{SessionStartEvent, SessionStartReason};
use pi_coding_agent::core::session_manager::{NewSessionOptions, SessionManager};
use tokio::sync::Mutex;

use super::actor::{spawn_session_actor, SessionHandle, SessionStatus};
use super::factory::{CreateSessionFactory, CreateSessionParams};
use super::runtime::find_session_file_by_id;
use crate::events::SessionEventState;

/// Default maximum number of sessions in the pool.
///
/// When the pool is full, the least-recently-used session's actor is stopped.
const DEFAULT_POOL_CAPACITY: usize = 128;

/// Manages multiple concurrent LLM agent sessions with LRU eviction.
///
/// The public API intentionally stays close to the previous
/// mutex-guarded-runtime pool so the web / Tauri / social layers only need
/// mechanical updates (the returned handles replace raw runtime access).
pub struct SessionPool {
    /// Active session actors with LRU eviction, keyed by session ID.
    sessions: Mutex<LruCache<String, SessionHandle>>,
    /// Per-session event state (seq + ring), kept across actor eviction so a
    /// reconnect resumes the same numbering (a resumed actor reuses it).
    event_states: Mutex<HashMap<String, Arc<SessionEventState>>>,
    /// Factory for creating new sessions (owned by the spawned actor).
    factory: CreateSessionFactory,
}

impl SessionPool {
    pub fn new(factory: CreateSessionFactory) -> Self {
        Self {
            sessions: Mutex::new(LruCache::new(
                // SAFETY: DEFAULT_POOL_CAPACITY is a positive constant.
                #[allow(clippy::unwrap_used)]
                NonZeroUsize::new(DEFAULT_POOL_CAPACITY).unwrap(),
            )),
            event_states: Mutex::new(HashMap::new()),
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
        self.spawn_session(cwd, Some(session_id), None).await
    }

    /// Create a session excluding specific tools (e.g. read-only automations
    /// excluding bash/edit/write). Returns the new session ID.
    ///
    /// # Errors
    /// Session creation failed.
    pub async fn create_session_with_tools(
        &self,
        cwd: &str,
        exclude_tools: &[&str],
    ) -> Result<String, String> {
        let excluded: Vec<String> = exclude_tools.iter().map(|s| (*s).to_string()).collect();
        self.spawn_session(cwd, None, Some(excluded)).await
    }

    /// Unified session creation: optional custom ID + optional tool exclusion.
    async fn spawn_session(
        &self,
        cwd: &str,
        session_id: Option<String>,
        exclude_tools: Option<Vec<String>>,
    ) -> Result<String, String> {
        let session_id = session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        // Pass the custom session ID via NewSessionOptions so pi-rs uses the
        // same ID internally (session file naming, header, etc.).
        let new_session_opts = NewSessionOptions {
            id: Some(session_id.clone()),
            parent_session: None,
        };
        let session_manager =
            SessionManager::new(cwd, &session_dir, None, true, Some(new_session_opts));

        let created = (self.factory.clone())(CreateSessionParams {
            cwd: cwd.to_string(),
            agent_dir,
            exclude_tools,
            session_manager,
            session_start_event: None,
        })
        .await?;

        let event_state = self.event_state_for(&session_id).await;
        let handle = spawn_session_actor(session_id.clone(), created, event_state);
        self.evict_on_put(session_id.clone(), handle).await;
        Ok(session_id)
    }

    /// Resume a session from disk by ID (for reconnection after pool miss).
    ///
    /// Finds the session's JSONL file on disk via `find_session_file_by_id`
    /// and opens it directly (pi-rs `SessionManager::open`, which extracts
    /// the cwd from the session header — the ACP `session/load` semantics).
    /// Returns an error if the session already exists in the pool or if no
    /// session file is found on disk.
    pub async fn resume_session(&self, session_id: &str) -> Result<(), String> {
        // Check for duplicate.
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

        // Open the existing session file (cwd extracted from the header).
        let session_manager = SessionManager::open(&session_file, Some(&session_dir), None);

        let created = (self.factory.clone())(CreateSessionParams {
            cwd: session_manager.get_cwd().to_string(),
            agent_dir,
            exclude_tools: None,
            session_manager,
            session_start_event: Some(SessionStartEvent {
                reason: SessionStartReason::Resume,
                previous_session_file: None,
            }),
        })
        .await?;

        let event_state = self.event_state_for(session_id).await;
        let handle = spawn_session_actor(session_id.to_string(), created, event_state);
        self.evict_on_put(session_id.to_string(), handle).await;
        Ok(())
    }

    /// Get a reference to a session actor by ID.
    ///
    /// Accessing a session promotes it to the front of the LRU cache.
    pub async fn get_session(&self, id: &str) -> Option<SessionHandle> {
        self.sessions.lock().await.get(id).cloned()
    }

    /// Get a session from the pool, or resume it from disk if not found.
    ///
    /// This combines `get_session()` + `resume_session()` into a single call,
    /// returning an error if the session doesn't exist on disk either.
    pub async fn get_or_resume(&self, session_id: &str) -> Result<SessionHandle, String> {
        if let Some(handle) = self.get_session(session_id).await {
            return Ok(handle);
        }
        self.resume_session(session_id).await?;
        self.get_session(session_id)
            .await
            .ok_or_else(|| "Session not found after resume".to_string())
    }

    /// Get the per-session [`SessionEventState`] (seq + ring) for reconnect
    /// replay. Created lazily; survives actor eviction.
    pub async fn event_state(&self, session_id: &str) -> Arc<SessionEventState> {
        self.event_state_for(session_id).await
    }

    /// Send a prompt to a session (streaming output via event subscribers).
    pub async fn prompt(&self, session_id: &str, text: &str) -> Result<(), String> {
        let handle = self
            .get_session(session_id)
            .await
            .ok_or_else(|| "Session not found".to_string())?;
        handle.prompt(text, PromptOptions::default()).await
    }

    /// Send a prompt with full options.
    ///
    /// The actor queues the prompt if another turn is in flight (pi-acp turn
    /// queue) and resolves once the turn has fully settled — the terminal
    /// `AgentEnd` event is emitted before this returns.
    pub async fn prompt_with_opts(
        &self,
        session_id: &str,
        text: &str,
        opts: PromptOptions,
    ) -> Result<(), String> {
        let handle = self
            .get_session(session_id)
            .await
            .ok_or_else(|| "Session not found".to_string())?;
        handle.prompt(text, opts).await
    }

    /// Run a prompt and return the last assistant text.
    ///
    /// This is a convenience for one-shot sessions (e.g. cron jobs, social bots)
    /// that create a session, run one prompt, get the result, and discard.
    pub async fn prompt_once(&self, cwd: &str, text: &str) -> Result<String, String> {
        let session_id = self.create_session(cwd).await?;
        let handle = self
            .get_session(&session_id)
            .await
            .ok_or_else(|| "Session not found after creation".to_string())?;

        handle.prompt(text, PromptOptions::default()).await?;
        let result = handle
            .last_assistant_text()
            .await
            .ok_or_else(|| "No assistant response".to_string())?;

        self.close_session(&session_id).await;
        Ok(result)
    }

    /// Close and remove a session (stops its actor thread).
    pub async fn close_session(&self, session_id: &str) {
        if let Some(handle) = self.sessions.lock().await.pop(session_id) {
            handle.shutdown().await;
        }
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
        let Some(handle) = self.get_session(session_id).await else {
            return false;
        };
        handle.is_streaming()
    }

    /// Get a richer status snapshot for a session (streaming + queue depth).
    pub async fn status(&self, session_id: &str) -> Option<SessionStatus> {
        let handle = self.get_session(session_id).await?;
        handle.status().await
    }

    /// Signal an in-flight prompt for `session_id` to abort.
    ///
    /// Returns `true` if the cancel message was delivered to a live actor,
    /// `false` if the session has no actor (not in the pool). Never blocks:
    /// the actor handles the cancel itself from its own thread.
    pub async fn signal_abort(&self, session_id: &str) -> bool {
        let Some(handle) = self.get_session(session_id).await else {
            return false;
        };
        handle.cancel().await
    }

    /// Get (or lazily create) the per-session event state. Not removed on
    /// actor eviction so `seq` numbering and the ring survive resume.
    async fn event_state_for(&self, session_id: &str) -> Arc<SessionEventState> {
        let mut states = self.event_states.lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(SessionEventState::new()))
            .clone()
    }

    /// Insert a session into the LRU cache, stopping the actor of any session
    /// replaced or evicted (same-key replacement or LRU eviction of a
    /// *different* key).
    async fn evict_on_put(&self, id: String, handle: SessionHandle) {
        let evicted = self.sessions.lock().await.push(id, handle);
        if let Some((evicted_id, evicted_handle)) = evicted {
            eprintln!("[pool] stopping replaced/evicted session {evicted_id}");
            evicted_handle.shutdown().await;
        }
    }
}
