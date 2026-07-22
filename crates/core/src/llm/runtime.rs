use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::Future;

use tokio::sync::mpsc;

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentEvent, AgentMessage};
use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, prelude::SettingsManager, CreateAgentSessionOptions};
use pi_extension_api::ToolDefinition;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

// ── ActiveSession ────────────────────────────────────────────────────────
//
// One entry per live agent session. Replacing the old single
// `Arc<Mutex<Option<AgentSessionRuntime>>>` + generation counter with a map
// keyed by session_id means switching the "selected" session in the UI no
// longer needs to abort/discard/rebuild anything — the previous session's
// runtime just stays parked in the map and keeps streaming if it was.

struct ActiveSession {
    runtime: Option<pi_coding_agent::core::agent_session_runtime::AgentSessionRuntime>,
    is_streaming: Arc<AtomicBool>,
    abort_flag: Arc<AtomicBool>,
}

impl ActiveSession {
    fn new() -> Self {
        Self {
            runtime: None,
            is_streaming: Arc::new(AtomicBool::new(false)),
            abort_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ── AgentRuntime ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentRuntime {
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
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
        session_id: Option<&str>,
        session_name: Option<&str>,
    ) -> Result<AgentFacade> {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let session_dir = format!("{cwd}/.pi-rs/agent/sessions");

        // Use SessionManager directly to create session with our custom ID
        let sid = match session_id {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => pi_coding_agent::core::session_manager::derive_short_session_id(),
        };
        let session_mgr = pi_coding_agent::core::session_manager::SessionManager::new(
            &cwd,
            &session_dir,
            None,
            persist.unwrap_or(true),
            Some(pi_coding_agent::core::session_manager::NewSessionOptions {
                id: Some(sid.clone()),
                parent_session: None,
            }),
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
            session_dir: Some(session_dir),
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

        Ok(AgentFacade {
            session: agent_session,
        })
    }

    pub async fn abort(&self, session_id: &str) {
        let sessions = self.sessions.lock().await;
        if let Some(entry) = sessions.get(session_id) {
            entry.abort_flag.store(true, Ordering::SeqCst);
            if let Some(r) = entry.runtime.as_ref() {
                r.session().abort().await;
            }
            entry.is_streaming.store(false, Ordering::SeqCst);
        }
    }

    pub async fn is_streaming(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .await
            .get(session_id)
            .map(|e| e.is_streaming.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    pub async fn get_messages(&self, session_id: &str) -> Vec<AgentMessage> {
        let sessions = self.sessions.lock().await;
        match sessions.get(session_id).and_then(|e| e.runtime.as_ref()) {
            Some(r) => r.session().get_messages().await,
            None => vec![],
        }
    }

    /// Drop a session's runtime entirely (e.g. on delete/close). Desktop
    /// rarely needs this; web should call it on disconnect / session close
    /// to free memory instead of letting sessions accumulate forever.
    pub async fn evict(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
    }
}

pub struct AgentFacade {
    pub session: AgentSession,
}

impl AgentFacade {
    pub async fn subscribe(&mut self, tx: mpsc::Sender<AgentEvent>) {
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

    pub async fn send(&mut self, message: &str) {
        self.session.add_user_text(message).await;
    }

    pub async fn send_messages(&mut self, messages: Vec<AgentMessage>) {
        let agent = self.session.get_agent();
        let _ = agent.process(messages).await;
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

    pub async fn run_with_timeout(
        &mut self,
        message: &str,
        timeout_secs: u64,
    ) -> Result<SessionResult> {
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);
        self.subscribe(tx).await;
        self.send(message).await;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(AgentEvent::MessageEnd { .. })) => break,
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!("run_with_timeout timed out after {timeout_secs}s");
                    self.abort().await;
                    return Err(anyhow::anyhow!("agent timed out after {timeout_secs}s"));
                },
            }
        }

        let messages = self.get_messages().await;
        Ok(extract_session_result(&messages))
    }
    /// Run the agent and extract structured `DecisionOutput`.
    ///
    /// Prefers the decision-tool path (single LLM turn).  Falls back to
    /// `extract_decision_output` (one extra LLM call).  If no trades are
    /// produced, re-prompts the agent (max 3 attempts) to force a decision.
    pub async fn process_decision(
        &self,
        _user_message: &str,
    ) -> Result<crate::roles::prompt::DecisionOutput> {
        todo!("需要重新设计llm结构化输出提取以及重试机制");
    }

    /// Run the agent and return the final text reply.
    pub async fn process_chat(&self, _user_message: &str) -> Result<String> {
        todo!("需要重新设计llm结构化输出提取以及重试机制");
    }
}

#[derive(Debug, Clone)]
pub struct SessionResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}

fn extract_session_result(messages: &[AgentMessage]) -> SessionResult {
    let mut final_text = String::new();
    let mut tool_outputs = Vec::new();

    for msg in messages {
        if let AgentMessage::Assistant { content, .. } = msg {
            for block in content {
                match block {
                    ContentBlock::Text { text, .. } => final_text.push_str(text),
                    ContentBlock::ToolCall { arguments, .. } => {
                        tool_outputs.push(arguments.clone())
                    },
                    _ => {},
                }
            }
        }
    }

    SessionResult {
        final_text,
        tool_outputs,
    }
}
