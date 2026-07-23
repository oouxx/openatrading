use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::Future;

use pi_coding_agent::core::session_manager::NewSessionOptions;
use tokio::sync::mpsc;

use openatrading_agent::pi_ai_types::ContentBlock;
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

        let session_dir = format!("{cwd}/.pi-rs/agent/sessions");

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
