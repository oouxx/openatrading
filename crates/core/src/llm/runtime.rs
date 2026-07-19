use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::Future;
use tokio::sync::{mpsc, watch};

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentEvent, AgentMessage};
use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_extension_api::{ExtensionRegistry, ToolDefinition};

// ── AgentRuntime ─────────────────────────────────────────────────────────

/// Unified runtime for all LLM operations.
///
/// Every LLM call goes through `create_session()` → `subscribe()` → `send()`.
/// No one-shot shortcuts — all invocations are streaming.
pub struct AgentRuntime {
    _private: (),
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Create a new agent session (the only way to invoke LLM).
    ///
    /// - `system_prompt`: system prompt for the session
    /// - `custom_tools`: additional tools (positions, historical_context, etc.)
    /// - `persist`: whether to persist the session to JSONL
    /// - `session_id`: pass existing session ID to resume; None creates a new session
    pub async fn create_session(
        &self,
        system_prompt: &str,
        custom_tools: Vec<ToolDefinition>,
        persist: bool,
        session_id: Option<&str>,
    ) -> Result<SessionHandle> {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let session_dir = format!("{cwd}/.pi-rs/agent/sessions");

        // Derive session file path if resuming
        let (sid, session_file) = match session_id {
            Some(id) if !id.is_empty() => {
                let file = format!("{session_dir}/{id}.jsonl");
                (id.to_string(), Some(file))
            },
            _ => {
                let id = pi_coding_agent::core::session_manager::derive_short_session_id();
                (id, None)
            },
        };

        let ext_registry = openatrading_extension::create_registry();

        let custom_tools_opt = if custom_tools.is_empty() {
            None
        } else {
            Some(custom_tools)
        };

        let (session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
            cwd,
            agent_dir: None,
            model: None,
            thinking_level: None,
            scoped_models: None,
            no_tools: Some(NoToolsMode::All),
            tools: None,
            exclude_tools: None,
            custom_prompt: Some(system_prompt.to_string()),
            append_system_prompt: None,
            session_name: Some(sid.clone()),
            stream_fn: None,
            convert_to_llm: None,
            custom_tools: custom_tools_opt,
            extension_paths: Vec::new(),
            enable_extensions: true,
            extension_registry: Some(ext_registry),
            cli_provider: None,
            cli_model: None,
            persist_session: persist,
            session_file,
            fork_from: None,
            session_dir: Some(session_dir),
        })
        .await
        .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;

        // Get the actual session ID (pi-coding-agent may have renamed the file)
        let actual_id = session
            .get_session_file()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or(sid);

        Ok(SessionHandle {
            session,
            session_id: actual_id,
        })
    }
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ── SessionHandle ────────────────────────────────────────────────────────

/// A handle to an active pi-coding-agent session.
///
/// Usage pattern:
/// ```rust,ignore
/// let mut handle = runtime.create_session(&prompt, tools, true, None).await?;
/// handle.subscribe(tx).await;
/// handle.send(&user_msg).await;
/// // events flow through tx
/// ```
pub struct SessionHandle {
    session: AgentSession,
    session_id: String,
}

impl SessionHandle {
    /// Subscribe to agent events. Must be called before `send()`.
    pub async fn subscribe(&mut self, tx: mpsc::Sender<AgentEvent>) {
        let event_tx = tx.clone();
        let listener = Arc::new(
            move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
                let tx = event_tx.clone();
                Box::pin(async move {
                    let _ = tx.send(event).await;
                }) as Pin<Box<dyn Future<Output = ()> + Send>>
            },
        );
        self.session.subscribe(listener).await;
    }

    /// Send a user message and run the agent loop.
    /// Events are delivered through the channel passed to `subscribe()`.
    pub async fn send(&mut self, message: &str) {
        self.session.add_user_text(message).await;
    }

    /// Get all messages in the current session.
    pub async fn get_messages(&self) -> Vec<AgentMessage> {
        self.session.get_messages().await
    }

    /// Abort the current agent loop.
    pub async fn abort(&self) {
        self.session.abort().await;
    }

    /// Get the session ID (JSONL file stem).
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Run the agent loop with a timeout. Returns the final text and tool outputs.
    ///
    /// Convenience method that creates a channel, subscribes, sends, and collects
    /// results with a timeout. Useful for callers that don't need streaming
    /// (cron, telegram, etc.) but still go through the streaming path internally.
    pub async fn run_with_timeout(
        &mut self,
        message: &str,
        timeout_secs: u64,
    ) -> Result<SessionResult> {
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);
        self.subscribe(tx).await;
        self.send(message).await;

        // Wait for completion or timeout
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
}

// ── SessionResult ────────────────────────────────────────────────────────

/// Result extracted from a completed session.
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}

/// Extract final text and tool outputs from agent messages.
fn extract_session_result(messages: &[AgentMessage]) -> SessionResult {
    let mut final_text = String::new();
    let mut tool_outputs = Vec::new();

    for msg in messages {
        if let AgentMessage::Assistant { content, .. } = msg {
            for block in content {
                match block {
                    ContentBlock::Text { text, .. } => final_text.push_str(text),
                    ContentBlock::ToolCall { arguments, .. } => {
                        tool_outputs.push(arguments.clone());
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
