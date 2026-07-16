use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::Future;
use tokio::sync::{mpsc, watch, Mutex};

use openatrading_agent::agent::{Agent, AgentOptions};
use openatrading_agent::llm::LlmClient;
use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentEvent, AgentMessage, AgentState, DynTool};
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::session_manager::SessionManager;
use pi_coding_agent::core::system_prompt::{self, BuildSystemPromptOptions, SkillInfo};
use pi_coding_agent::core::tools;

use super::StreamEvent;

// ── AgentResult ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}

// ── AgentSession (internal) — pi-coding-agent API surface ────────────────

/// Wraps pi-agent-core's Agent with pi-coding-agent-style API:
/// `add_user_text()`, `get_messages()`, `subscribe()`, `wait_for_idle()`.
pub(crate) struct AgentSession {
    agent: Arc<Agent>,
    message_count: AtomicUsize,
}

impl AgentSession {
    fn new(
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
        thinking_level: &str,
        session_manager: &SessionManager,
    ) -> Self {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // 1. Build system prompt via pi-coding-agent's builder
        let prompt = system_prompt::build_system_prompt(&BuildSystemPromptOptions {
            cwd: cwd.clone(),
            custom_prompt: Some(system_prompt.to_string()),
            skills: Some(skills),
            ..Default::default()
        });

        // 2. Read-only tools + trading tools (no bash/write/edit for security)
        let mut all_tools: Vec<Arc<DynTool>> = tools::create_read_only_tools(&cwd, None)
            .into_iter()
            .map(Arc::new)
            .collect();
        all_tools.extend(trading_tools);

        // 3. Construct Agent
        let agent = Agent::new(AgentOptions {
            initial_state: Some(AgentState {
                system_prompt: prompt,
                model: llm.model.clone(),
                thinking_level: thinking_level.to_string(),
                tools: all_tools,
                ..Default::default()
            }),
            stream_fn: Some(llm.to_stream_fn()),
            session_id: Some(session_manager.get_session_id().to_string()),
            ..Default::default()
        });

        Self {
            agent: Arc::new(agent),
            message_count: AtomicUsize::new(0),
        }
    }

    /// pi-coding-agent style: add a user message and trigger the agent loop.
    async fn add_user_text(&self, text: &str) {
        let messages = vec![AgentMessage::User {
            content: vec![ContentBlock::text(text)],
            timestamp: 0,
        }];
        let _ = self.agent.process(messages).await;
    }

    /// pi-coding-agent style: get all messages from the session.
    async fn get_messages(&self) -> Vec<AgentMessage> {
        self.agent.state().await.messages
    }

    /// pi-coding-agent style: subscribe to agent events.
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
        self.agent.subscribe(listener).await
    }

    fn message_count(&self) -> usize {
        self.message_count.load(Ordering::Relaxed)
    }

    fn reset_message_count(&self) {
        self.message_count.store(0, Ordering::Relaxed);
    }
}

use openatrading_agent::agent::UnsubscribeHandle;

// ── AgentRuntime ─────────────────────────────────────────────────────────

/// Manages the lifecycle of agent sessions.
///
/// Uses pi-coding-agent-style API internally (`add_user_text`, `get_messages`).
/// Provides two execution modes:
/// - `stream()` — real-time event streaming for frontend rendering
/// - `run()` — blocking wait for final result (backend/decision use)
pub struct AgentRuntime {
    session_manager: SessionManager,
    _model_registry: ModelRegistry,
    sessions: Mutex<HashMap<String, Arc<AgentSession>>>,
}

impl AgentRuntime {
    pub fn new(session_manager: SessionManager, model_registry: ModelRegistry) -> Self {
        Self {
            session_manager,
            _model_registry: model_registry,
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
        let session = Arc::new(AgentSession::new(
            llm,
            system_prompt,
            trading_tools,
            skills,
            thinking_level,
            &self.session_manager,
        ));
        self.sessions.lock().await.insert(id.to_string(), session);
        id.to_string()
    }

    /// Destroy a session by ID.
    pub async fn destroy(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
    }

    /// Get or create a session.
    pub(crate) async fn get_or_create(
        &self,
        id: &str,
        llm: &LlmClient,
        system_prompt: &str,
        trading_tools: Vec<Arc<DynTool>>,
        skills: Vec<SkillInfo>,
        thinking_level: &str,
    ) -> Arc<AgentSession> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(id) {
            return Arc::clone(session);
        }
        let session = Arc::new(AgentSession::new(
            llm,
            system_prompt,
            trading_tools,
            skills,
            thinking_level,
            &self.session_manager,
        ));
        let session_clone = Arc::clone(&session);
        sessions.insert(id.to_string(), session);
        session_clone
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
        let new_session = Arc::new(AgentSession::new(
            llm,
            system_prompt,
            trading_tools,
            skills,
            thinking_level,
            &self.session_manager,
        ));
        sessions.insert(session_id.to_string(), new_session);
    }

    // ── Unified execution core (uses add_user_text + get_messages) ───

    /// Execute the agent loop using pi-coding-agent-style API.
    ///
    /// Flow: subscribe → add_user_text → wait_for_idle → get_messages
    async fn execute(
        session: &AgentSession,
        user_message: &str,
        listener: Option<Arc<dyn Fn(AgentEvent) + Send + Sync>>,
    ) -> Result<AgentResult> {
        let (tx, mut rx) = mpsc::channel::<AgentEvent>(256);

        // 1. Subscribe to events (pi-coding-agent style)
        let event_tx = tx.clone();
        let subscribe_listener = Arc::new(
            move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
                let tx = event_tx.clone();
                Box::pin(async move {
                    let _ = tx.send(event).await;
                }) as Pin<Box<dyn Future<Output = ()> + Send>>
            },
        );
        // keep handle alive for the full scope
        let _handle: UnsubscribeHandle = session.subscribe(subscribe_listener).await;

        // 2. Add user message (pi-coding-agent style)
        session.add_user_text(user_message).await;

        // 3. Collect events until Done or Error
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
    /// Uses pi-coding-agent-style subscribe + add_user_text internally.
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
    /// Uses pi-coding-agent-style add_user_text + get_messages internally.
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
