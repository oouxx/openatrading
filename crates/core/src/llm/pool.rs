use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions};
use pi_coding_agent::pi_ai_types::Model;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages multiple concurrent LLM agent sessions.
///
/// Each session is backed by a pi-coding-agent [`AgentSession`] created
/// via `sdk::create_agent_session`. Sessions are identified by UUID and
/// stored in a HashMap for concurrent access.
///
/// The extension registry is created fresh for each session via
/// `openatrading_extension::create_registry()` since `ExtensionRegistry`
/// does not implement `Clone` and cannot be shared across sessions.
pub struct SessionPool {
    /// All active sessions, keyed by session ID.
    sessions: Mutex<HashMap<String, Arc<Mutex<AgentSession>>>>,
    /// Base directory for agent data.
    agent_dir: String,
    /// Default model override.
    model: Option<Model>,
    /// Default tool names to activate.
    tools: Option<Vec<String>>,
}

impl SessionPool {
    pub fn new(agent_dir: &str, model: Option<Model>, tools: Option<Vec<String>>) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            agent_dir: agent_dir.to_string(),
            model,
            tools,
        }
    }

    /// Create a new session and return its ID.
    ///
    /// `system_prompt` — if provided, overrides the default system prompt
    ///   on the session's agent after creation.
    /// `custom_tools` — additional tool definitions to register.
    pub async fn create_session(&self, cwd: &str) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();

        // Create a fresh extension registry for each session.
        // ExtensionRegistry is !Clone, so it cannot be stored and shared;
        // each session gets its own independent copy.
        let ext_registry = openatrading_extension::create_registry();

        let (session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
            cwd: cwd.to_string(),
            agent_dir: Some(self.agent_dir.clone()),
            model: self.model.clone(),
            tools: self.tools.clone(),
            extension_registry: Some(ext_registry),
            session_dir: Some(session_dir),
            persist_session: true,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Failed to create session: {e}"))?;

        let session = Arc::new(Mutex::new(session));
        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Get a reference to a session by ID.
    pub async fn get_session(&self, id: &str) -> Option<Arc<Mutex<AgentSession>>> {
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
        let s = session.lock().await;
        s.prompt(text, None).await;
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
                let s = session.lock().await;
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
}
