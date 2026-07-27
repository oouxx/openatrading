use pi_coding_agent::core::agent_session_runtime::{
    create_agent_session_runtime, AgentSessionRuntime, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeParams,
};
use pi_coding_agent::core::session_manager::SessionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages multiple concurrent LLM agent sessions.
///
/// Each session is backed by a pi-coding-agent [`AgentSessionRuntime`] created
/// via `create_agent_session_runtime` + a factory. Sessions are identified by
/// UUID and stored in a HashMap for concurrent access.
pub struct SessionPool {
    /// All active sessions, keyed by session ID.
    sessions: Mutex<HashMap<String, Arc<Mutex<AgentSessionRuntime>>>>,
    /// Factory for creating new `AgentSessionRuntime` instances.
    factory: CreateAgentSessionRuntimeFactory,
}

impl SessionPool {
    pub fn new(factory: CreateAgentSessionRuntimeFactory) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            factory,
        }
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self, cwd: &str) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        let session_manager = SessionManager::new(cwd, &session_dir, None, true, None);

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
        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), runtime);
        Ok(session_id)
    }

    /// Create a new session with a specific ID (for reconnection after pool miss).
    /// Returns an error if a session with this ID already exists.
    pub async fn create_session_with_id(&self, session_id: &str, cwd: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(session_id) {
            return Err(format!("Session {session_id} already exists"));
        }
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();

        let session_manager = SessionManager::new(cwd, &session_dir, None, true, None);

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
        sessions.insert(session_id.to_string(), runtime);
        Ok(())
    }

    /// Get a reference to a session runtime by ID.
    pub async fn get_session(&self, id: &str) -> Option<Arc<Mutex<AgentSessionRuntime>>> {
        self.sessions.lock().await.get(id).cloned()
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
        self.sessions.lock().await.remove(session_id);
    }

    /// List all active session IDs.
    pub async fn list_session_ids(&self) -> Vec<String> {
        self.sessions.lock().await.keys().cloned().collect()
    }

    /// Check if a session exists in the pool.
    pub async fn has_session(&self, session_id: &str) -> bool {
        self.sessions.lock().await.contains_key(session_id)
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
