use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::Future;
use tokio::sync::{mpsc, watch, Mutex};

use openatrading_agent::agent::UnsubscribeHandle;
use openatrading_agent::llm::LlmClient;
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentEvent, AgentMessage, DynTool};
use pi_coding_agent::core::agent_session::AgentSession as CodingAgentSession;
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_coding_agent::core::session_manager::SessionManager;
use pi_coding_agent::core::system_prompt::SkillInfo;

use super::extension::TradingToolExtension;
use super::StreamEvent;

// ── AgentResult ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}

// ── AgentSession (internal) — wraps pi-coding-agent's AgentSession ────────

/// Wraps pi-coding-agent's AgentSession with trading-specific additions.
/// Uses `sdk::create_agent_session()` for construction.
pub(crate) struct AgentSession {
    inner: Mutex<CodingAgentSession>,
    message_count: AtomicUsize,
}

impl AgentSession {
    async fn new(
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        _skills: Vec<SkillInfo>,
        _thinking_level: &str,
        _session_manager: &SessionManager,
        _model_registry: &ModelRegistry,
    ) -> Self {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Create extension registry with trading tool definitions
        let ext_registry = TradingToolExtension::create_registry();

        // Use pi-coding-agent's sdk to create the session
        let (session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
            cwd: cwd.clone(),
            agent_dir: None,
            model: None,
            thinking_level: None,
            scoped_models: None,
            no_tools: Some(NoToolsMode::All),
            tools: Some(trading_tools.iter().map(|t| t.name.clone()).collect()),
            exclude_tools: None,
            custom_prompt: Some(system_prompt.to_string()),
            append_system_prompt: None,
            session_name: None,
            stream_fn: Some(llm.to_stream_fn()),
            convert_to_llm: None,
            extension_paths: Vec::new(),
            enable_extensions: false,
            extension_registry: Some(ext_registry),
            cli_provider: None,
            cli_model: None,
            persist_session: false,
            session_file: None,
            fork_from: None,
            session_dir: None,
        })
        .await
        .map_err(|e| tracing::error!("create_agent_session failed: {e}"))
        .expect("create_agent_session failed");

        // Inject trading tool implementations into the agent's state.
        // The extension's register_tools() provides metadata (name, description, schema);
        // the actual DynTool instances are added here for the agent to execute.
        let agent = session.get_agent();
        let state = agent.state().await;
        let mut all_tools = state.tools;
        all_tools.extend(trading_tools);

        Self {
            inner: Mutex::new(session),
            message_count: AtomicUsize::new(0),
        }
    }

    /// pi-coding-agent style: add a user message and trigger the agent loop.
    async fn add_user_text(&self, text: &str) {
        self.inner.lock().await.add_user_text(text).await;
    }

    /// pi-coding-agent style: get all messages from the session.
    async fn get_messages(&self) -> Vec<AgentMessage> {
        self.inner.lock().await.get_messages().await
    }

    /// pi-coding-agent style: subscribe to agent events.
    /// Returns the UnsubscribeHandle — caller must keep it alive.
    async fn subscribe(
        &self,
        listener: Arc<
            dyn Fn(
                    AgentEvent,
                    Option<watch::Receiver<bool>>,
                ) -> Pin<Box<dyn Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    ) -> UnsubscribeHandle {
        // Subscribe via pi-coding-agent's session (for internal event bus)
        self.inner.lock().await.subscribe(listener.clone()).await;
        // Also subscribe via the underlying agent (returns a handle we can keep alive)
        let guard = self.inner.lock().await;
        let agent = guard.get_agent();
        let handle = agent.subscribe(listener).await;
        handle
    }

    fn message_count(&self) -> usize {
        self.message_count.load(Ordering::Relaxed)
    }

    fn reset_message_count(&self) {
        self.message_count.store(0, Ordering::Relaxed);
    }
}

// ── AgentRuntime ─────────────────────────────────────────────────────────

/// Manages the lifecycle of agent sessions.
///
/// Uses pi-coding-agent's `sdk::create_agent_session()` internally.
/// Provides two execution modes:
/// - `stream()` — real-time event streaming for frontend rendering
/// - `run()` — blocking wait for final result (backend/decision use)
pub struct AgentRuntime {
    session_manager: SessionManager,
    model_registry: ModelRegistry,
    sessions: Mutex<HashMap<String, Arc<AgentSession>>>,
}

impl AgentRuntime {
    pub fn new(session_manager: SessionManager, model_registry: ModelRegistry) -> Self {
        Self {
            session_manager,
            model_registry,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    // ── Lifecycle ────────────────────────────────────────────────────

    /// Create a new agent session.
    pub async fn create(
        &self,
        id: &str,
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
        thinking_level: &str,
    ) -> String {
        let session = Arc::new(
            AgentSession::new(
                llm,
                system_prompt,
                trading_tools,
                skills,
                thinking_level,
                &self.session_manager,
                &self.model_registry,
            )
            .await,
        );
        self.sessions.lock().await.insert(id.to_string(), session);
        id.to_string()
    }

    /// Destroy a session by ID.
    pub async fn destroy(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
    }

    /// Get the number of messages in a session.
    pub async fn message_count(&self, session_id: &str) -> Option<usize> {
        let sessions = self.sessions.lock().await;
        sessions.get(session_id).map(|s| s.message_count())
    }

    /// Compact a session: rebuild with fresh agent.
    pub async fn compact(
        &self,
        session_id: &str,
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
        thinking_level: &str,
    ) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(session_id) {
            session.reset_message_count();
        }
        let new_session = Arc::new(
            AgentSession::new(
                llm,
                system_prompt,
                trading_tools,
                skills,
                thinking_level,
                &self.session_manager,
                &self.model_registry,
            )
            .await,
        );
        sessions.insert(session_id.to_string(), new_session);
    }

    // ── Unified execution core ──────────────────────────────────────

    /// Execute the agent loop using pi-coding-agent's AgentSession.
    ///
    /// Flow: subscribe → add_user_text → wait_for_idle → get_messages
    async fn execute(
        session: &AgentSession,
        user_message: &str,
        listener: Option<Arc<dyn Fn(AgentEvent) + Send + Sync>>,
    ) -> Result<AgentResult> {
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);

        // 1. Subscribe to events via the underlying agent
        let event_tx = tx.clone();
        let subscribe_listener = Arc::new(
            move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
                let tx = event_tx.clone();
                Box::pin(async move {
                    let _ = tx.send(event).await;
                }) as Pin<Box<dyn Future<Output = ()> + Send>>
            },
        );
        // Subscribe via the underlying agent (returns a handle we keep alive)
        let _handle = {
            let guard = session.inner.lock().await;
            let agent = guard.get_agent();
            agent.subscribe(subscribe_listener).await
            // guard dropped here, but handle is independent
        };

        // 2. Add user message (pi-coding-agent style) — triggers agent loop
        session.add_user_text(user_message).await;

        // 4. Collect events
        let mut final_text = String::new();
        let mut tool_outputs: Vec<serde_json::Value> = Vec::new();

        while let Some(event) = rx.recv().await {
            if let Some(ref cb) = listener {
                cb(event.clone());
            }

            match &event {
                AgentEvent::MessageUpdate {
                    assistant_message_event:
                        openatrading_agent::pi_ai_types::AssistantMessageEvent::TextDelta {
                            delta, ..
                        },
                    ..
                } => {
                    final_text.push_str(delta);
                },
                AgentEvent::MessageUpdate {
                    assistant_message_event:
                        openatrading_agent::pi_ai_types::AssistantMessageEvent::Done { message, .. },
                    ..
                } => {
                    for block in &message.content {
                        if let ContentBlock::ToolCall { arguments, .. } = block {
                            tool_outputs.push(arguments.clone());
                        }
                    }
                    break;
                },
                AgentEvent::MessageUpdate {
                    assistant_message_event:
                        openatrading_agent::pi_ai_types::AssistantMessageEvent::Error { .. },
                    ..
                } => break,
                _ => {},
            }
        }

        Ok(AgentResult {
            final_text,
            tool_outputs,
        })
    }

    // ── Mode 1: Real-time event streaming (frontend) ────────────────

    /// Stream agent events in real-time to the frontend.
    pub async fn stream(
        &self,
        session_id: &str,
        user_message: &str,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<AgentResult> {
        let session = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(session_id)
                .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?
                .clone()
        };

        let listener = Arc::new(move |event: AgentEvent| {
            let tx = tx.clone();
            if let AgentEvent::MessageUpdate {
                assistant_message_event,
                ..
            } = event
            {
                let stream_events =
                    super::assistant_event_to_stream_events(&assistant_message_event);
                for se in stream_events {
                    let _ = tx.try_send(se);
                }
            }
        });

        let result = Self::execute(&session, user_message, Some(listener)).await?;
        session.message_count.fetch_add(1, Ordering::Relaxed);
        Ok(result)
    }

    // ── Mode 2: Blocking wait for result (backend) ──────────────────

    /// Run the agent loop and return the final result.
    pub async fn run(&self, session_id: &str, user_message: &str) -> Result<AgentResult> {
        let session = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(session_id)
                .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?
                .clone()
        };

        let result = Self::execute(&session, user_message, None).await?;
        session.message_count.fetch_add(1, Ordering::Relaxed);
        Ok(result)
    }
}
