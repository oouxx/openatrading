use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::Future;

use tokio::sync::mpsc;

use openatrading_agent::types::{AgentEvent, AgentMessage};
use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, prelude::SettingsManager, CreateAgentSessionOptions};
use pi_extension_api::ToolDefinition;
use tokio::sync::Mutex;

// ── AgentRuntime ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentRuntime {
    sessions: Arc<Mutex<HashMap<String, Arc<AgentFacade>>>>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new agent session (the only way to invoke LLM).
    pub async fn create_session(
        &self,
        system_prompt: Option<&str>,
        custom_tools: Option<Vec<ToolDefinition>>,
        persist: Option<bool>,
        _: Option<&str>,
        session_name: Option<&str>,
    ) -> Result<Arc<AgentFacade>> {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();

        let session_mgr = pi_coding_agent::core::session_manager::SessionManager::new(
            &cwd,
            &session_dir,
            None,
            persist.unwrap_or(true),
            None,
        );
        let ext_registry = openatrading_extension::create_registry();

        let settings_mgr = SettingsManager::create(&cwd, None);

        let (mut agent_session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
            cwd,
            custom_prompt: system_prompt.map(str::to_owned),
            custom_tools,
            extension_paths: Vec::new(),
            enable_extensions: true,
            extension_registry: Some(ext_registry),
            persist_session: persist.unwrap_or(true),
            session_manager: Some(session_mgr),
            settings_manager: Some(settings_mgr),
            session_dir: Some(session_dir.to_string()),
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;

        // Set session name if provided (append to JSONL via AgentSession)
        if let Some(name) = session_name {
            if !name.is_empty() {
                agent_session.set_session_name(name);
            }
        }
        let facade = Arc::new(AgentFacade {
            session: agent_session,
        });

        self.sessions
            .lock()
            .await
            .insert(facade.get_session_id().to_string(), facade.clone());

        Ok(facade)
    }
}
pub struct AgentFacade {
    pub session: AgentSession,
}

impl AgentFacade {
    pub async fn subscribe(&self, tx: mpsc::Sender<AgentEvent>) {
        let event_tx = tx.clone();
        let listener = Arc::new(
            move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
                let tx = event_tx.clone();
                Box::pin(async move {
                    let _ = tx.send(event).await;
                }) as Pin<Box<dyn Future<Output = ()> + Send>>
            },
        );
        self.session.subscribe(listener).await;
    }

    pub async fn send(&self, message: &str) {
        self.session.add_user_text(message).await;
    }

    pub async fn get_messages(&self) -> Vec<AgentMessage> {
        self.session.get_messages().await
    }

    pub async fn abort(&self) {
        self.session.abort().await;
    }

    pub fn get_session_id(&self) -> String {
        self.session.get_session_id()
    }
    pub fn get_session_name(&self) -> String {
        self.session
            .get_session_name()
            .unwrap_or("default".to_string())
    }

    /// Run the agent and return the final text reply.
    ///
    /// `prompt()` on `AgentSession` holds `std::sync::MutexGuard` across `.await`
    /// points, making the future `!Send`. We isolate it via `spawn_blocking` so
    /// callers can use this in `tokio::spawn` contexts.
    pub async fn process_chat(self: Arc<Self>, user_message: &str) -> Result<String> {
        let this = self.clone();
        let msg = user_message.to_string();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                this.session.prompt(&msg, None).await;
                this.session
                    .get_last_assistant_text()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("No assistant response"))
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("Blocking task failed: {e}"))?
    }

    /// Run a decision cycle.
    ///
    /// The caller is responsible for injecting the `make_decision` tool (or
    /// other trading tools) into the session at creation time via
    /// `create_session(…, custom_tools, …)`. This method simply runs the
    /// prompt and returns the last assistant text.
    ///
    /// `prompt()` on `AgentSession` is `!Send` (holds `std::sync::MutexGuard`
    /// across `.await`), so we isolate it via `spawn_blocking`.
    pub async fn process_decision(self: Arc<Self>, user_message: &str) -> Result<String> {
        let this = self.clone();
        let msg = user_message.to_string();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                this.session.prompt(&msg, None).await;
                this.session
                    .get_last_assistant_text()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("No assistant response"))
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("Decision task failed: {e}"))?
    }
}

#[derive(Debug, Clone)]
pub struct SessionResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}
