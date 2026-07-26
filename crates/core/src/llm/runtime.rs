//! Thin wrapper layer over pi-coding-agent's [`AgentSession`].
//!
//! [`AgentFacade`] owns a single [`AgentSession`] and re-exposes every
//! session-management capability we use as a method on the facade. The web /
//! Tauri / CLI layers talk to the facade instead of reaching into
//! `pi_coding_agent` directly, so the integration boundary lives in one file
//! and can be adapted when pi-coding-agent's API evolves.
//!
//! ## Ownership
//!
//! Each [`AgentRuntime::create_session`] call produces a freshly-owned
//! `Arc<AgentFacade>` — nothing is cached here. Long-lived ownership (the web
//! transport keeping one facade per session across prompts, the Tauri
//! transport doing the same) is the caller's responsibility. Keeping the
//! facade behind a single `Arc` keeps `Arc::get_mut` usable for the
//! `&mut self` slash-command style operations (set model / name / thinking,
//! fork, new session).

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use futures::Future;
use tokio::sync::mpsc;

use openatrading_agent::types::{AgentEvent, AgentMessage};
use pi_coding_agent::core::agent_session::{
    AgentSession, AgentSessionEventListener, SessionEventUnsubscribeHandle, SessionStats,
};
use pi_coding_agent::core::context_usage::ContextUsage;
use pi_coding_agent::core::extensions::{ExtensionRegistry, ToolDefinition, ToolInfo};
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::prompt_templates::PromptTemplate;
use pi_coding_agent::core::sdk::{self, prelude::SettingsManager, CreateAgentSessionOptions};
use pi_coding_agent::core::session_manager::SessionTreeNode;
use pi_coding_agent::core::{compaction::CompactionSettings, session_manager::SessionManager};
use pi_coding_agent::pi_agent_core::agent::UnsubscribeHandle;
use pi_coding_agent::pi_agent_core::pi_ai_types::{ContentBlock, Model, ThinkingLevel};
use pi_coding_agent::pi_agent_core::types::QueueMode;

// ── AgentRuntime ─────────────────────────────────────────────────────────

/// Factory for agent sessions.
///
/// Each [`AgentRuntime::create_session`] call produces a freshly-owned
/// `Arc<AgentFacade>` — nothing is cached here. Long-lived ownership (e.g. the
/// Tauri transport keeping one facade per session across prompts) is the
/// caller's responsibility; this keeps `Arc::get_mut` usable for slash
/// commands that need `&mut AgentSession`.
#[derive(Clone, Default)]
pub struct AgentRuntime;

/// Find a session JSONL file by session ID in the given directory.
///
/// Session files are named `{timestamp}_{id}.jsonl`. This scans the directory
/// for a file whose name ends with `_{id}.jsonl`. Falls back to `{id}.jsonl`
/// for legacy naming.
fn find_session_file_by_id(session_dir: &str, id: &str) -> Option<String> {
    let dir = PathBuf::from(session_dir);
    // Legacy: direct {id}.jsonl
    let direct = dir.join(format!("{id}.jsonl"));
    if direct.exists() {
        return Some(direct.to_string_lossy().to_string());
    }
    // Current: {timestamp}_{id}.jsonl
    let suffix = format!("_{id}.jsonl");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(&suffix) {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

impl AgentRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Create a new agent session (the only way to invoke LLM).
    ///
    /// If `session_id` is provided, it reuses the existing session file
    /// at `{session_dir}/{session_id}.jsonl` instead of creating a new one.
    /// This allows the chat endpoint to continue a session created by
    /// the sessions API.
    pub async fn create_session(
        &self,
        system_prompt: Option<&str>,
        custom_tools: Option<Vec<ToolDefinition>>,
        persist: Option<bool>,
        session_id: Option<&str>,
        session_name: Option<&str>,
    ) -> Result<Arc<AgentFacade>> {
        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let session_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();

        // If session_id is provided, find the existing session file by ID.
        // Session files are named `{timestamp}_{id}.jsonl`, so we scan the
        // session directory for a matching file instead of constructing the
        // path directly.
        let session_file = session_id.and_then(|id| find_session_file_by_id(&session_dir, id));

        let session_mgr = SessionManager::new(
            &cwd,
            &session_dir,
            session_file.as_deref(),
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
            session_dir: Some(session_dir.clone()),
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
        Ok(Arc::new(AgentFacade {
            session: agent_session,
        }))
    }
}

// ── AgentFacade ──────────────────────────────────────────────────────────

/// Wrapper around pi-coding-agent's [`AgentSession`].
///
/// The underlying session is private; all session-management capabilities
/// are exposed as methods on the facade so the rest of the codebase does not
/// import `pi_coding_agent` directly. Method groups:
///
/// - **Metadata**: [`Self::session_id`], [`Self::session_name`],
///   [`Self::set_session_name`], [`Self::session_file`], [`Self::session_dir`],
///   [`Self::cwd`], [`Self::session_stats`], [`Self::session_tree`]
/// - **Status**: [`Self::is_streaming`], [`Self::is_idle`],
///   [`Self::is_compacting`], [`Self::is_retrying`], [`Self::error_message`],
///   [`Self::context_usage`], [`Self::pending_message_count`]
/// - **Messages**: [`Self::messages`], [`Self::last_assistant_text`],
///   [`Self::load_messages_from_session`], [`Self::send`],
///   [`Self::add_user_message`], [`Self::send_user_message`]
/// - **Prompting / control**: [`Self::prompt`], [`Self::abort`],
///   [`Self::retry`], [`Self::abort_retry`], [`Self::abort_compaction`]
/// - **Model / thinking**: [`Self::model`], [`Self::set_model`],
///   [`Self::cycle_model`], [`Self::thinking_level`],
///   [`Self::set_thinking_level`], [`Self::cycle_thinking_level`],
///   [`Self::available_thinking_levels`], [`Self::supports_thinking`],
///   [`Self::scoped_models`], [`Self::set_scoped_models`], [`Self::model_registry`]
/// - **Compaction**: [`Self::should_compact`], [`Self::check_auto_compact`],
///   [`Self::compact`], [`Self::compaction_settings`],
///   [`Self::set_compaction_settings`]
/// - **Retry**: [`Self::auto_retry_enabled`], [`Self::set_auto_retry_enabled`],
///   [`Self::retry_attempt`]
/// - **Queues**: [`Self::steer`], [`Self::follow_up`],
///   [`Self::steering_messages`], [`Self::follow_up_messages`],
///   [`Self::has_queued_messages`], [`Self::clear_all_queues`],
///   [`Self::steering_mode`], [`Self::follow_up_mode`]
/// - **Tools**: [`Self::active_tool_names`], [`Self::set_active_tools_by_name`],
///   [`Self::all_tools`], [`Self::tool_definition`]
/// - **Events**: [`Self::subscribe`], [`Self::subscribe_session_events`],
///   [`Self::subscribe_to_channel`]
/// - **Session switching / resume**: [`Self::switch_session`],
///   [`Self::new_session`], [`Self::fork_session`]
/// - **System prompt / extensions**: [`Self::system_prompt`],
///   [`Self::extension_registry`], [`Self::prompt_templates`]
pub struct AgentFacade {
    session: AgentSession,
}

/// Async listener for [`AgentFacade::subscribe`].
///
/// Mirrors pi-coding-agent's (private) `AgentEventListener` type alias so
/// callers can name the type without importing from `pi_coding_agent`
/// internals. Wrap your closure in `Arc::new(...)` and it coerces here.
pub type AgentEventListener = Arc<
    dyn Fn(
            AgentEvent,
            Option<tokio::sync::watch::Receiver<bool>>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

impl AgentFacade {
    /// Borrow the underlying [`AgentSession`] directly.
    ///
    /// This escape hatch exists for the rare case where a capability is not
    /// (yet) wrapped. Prefer adding a wrapper method here instead of relying
    /// on this accessor long-term.
    #[must_use]
    pub fn raw(&self) -> &AgentSession {
        &self.session
    }

    /// Mutably borrow the underlying [`AgentSession`] directly (e.g. for
    /// `&mut self` operations not yet wrapped). See [`Self::raw`] for the
    /// rationale.
    #[must_use]
    pub fn raw_mut(&mut self) -> &mut AgentSession {
        &mut self.session
    }

    /// Borrow the underlying [`pi_coding_agent`] `Agent` (the LLM driving
    /// this session). Exposed for low-level prompts that need the returned
    /// message vector rather than the higher-level [`Self::prompt`] flow.
    #[must_use]
    pub fn agent(&self) -> &openatrading_agent::agent::Agent {
        self.session.get_agent()
    }

    // ── Metadata ─────────────────────────────────────────────────────────

    #[must_use]
    pub fn session_id(&self) -> String {
        self.session.get_session_id()
    }

    /// The session's persisted name, if any was set.
    #[must_use]
    pub fn session_name_raw(&self) -> Option<String> {
        self.session.get_session_name()
    }

    /// The session's name, falling back to `"default"` when none is set.
    #[must_use]
    pub fn session_name(&self) -> String {
        self.session
            .get_session_name()
            .unwrap_or_else(|| "default".to_string())
    }

    /// Set the session name. Requires unique access (e.g. via
    /// `Arc::get_mut`), matching pi-coding-agent's `&mut self` signature.
    pub fn set_session_name(&mut self, name: &str) {
        self.session.set_session_name(name);
    }

    #[must_use]
    pub fn session_file(&self) -> Option<PathBuf> {
        self.session.get_session_file()
    }

    #[must_use]
    pub fn session_dir(&self) -> PathBuf {
        self.session.get_session_dir()
    }

    #[must_use]
    pub fn cwd(&self) -> String {
        self.session.get_cwd().to_string()
    }

    #[must_use]
    pub fn session_stats(&self) -> SessionStats {
        self.session.get_session_stats()
    }

    /// Borrow the underlying `SessionManager` (guarded by an internal
    /// `Mutex`). The returned guard holds the lock; drop it as soon as
    /// possible to avoid blocking other session operations.
    pub fn session_manager(&self) -> std::sync::MutexGuard<'_, SessionManager> {
        self.session.get_session_manager()
    }

    /// Current agent state snapshot (owned `AgentState`).
    pub async fn state(&self) -> openatrading_agent::types::AgentState {
        self.session.get_state().await
    }

    #[must_use]
    pub fn session_tree(&self) -> Vec<SessionTreeNode> {
        self.session.get_tree()
    }

    // ── Status / state ───────────────────────────────────────────────────

    pub async fn is_streaming(&self) -> bool {
        self.session.is_streaming().await
    }

    pub async fn is_idle(&self) -> bool {
        self.session.is_idle().await
    }

    #[must_use]
    pub fn is_compacting(&self) -> bool {
        self.session.is_compacting()
    }

    #[must_use]
    pub fn is_retrying(&self) -> bool {
        self.session.is_retrying()
    }

    pub async fn error_message(&self) -> Option<String> {
        self.session.get_error_message().await
    }

    pub async fn context_usage(&self) -> Option<ContextUsage> {
        self.session.get_context_usage().await
    }

    #[must_use]
    pub fn pending_message_count(&self) -> usize {
        self.session.pending_message_count()
    }

    // ── Messages ─────────────────────────────────────────────────────────

    pub async fn messages(&self) -> Vec<AgentMessage> {
        self.session.get_messages().await
    }

    pub async fn last_assistant_text(&self) -> Option<String> {
        self.session.get_last_assistant_text().await
    }

    /// Reload messages from the session JSONL into agent state.
    /// Returns the number of messages restored.
    pub async fn load_messages_from_session(&self) -> usize {
        self.session.load_messages_from_session().await
    }

    /// Send a user text message and start processing (fire-and-forget).
    ///
    /// Maps to `AgentSession::add_user_text` — appends the message and runs
    /// one agent turn. Events are delivered via the subscribed listeners.
    pub async fn send(&self, message: &str) {
        self.session.add_user_text(message).await;
    }

    /// Append a structured user message (no automatic agent run).
    pub async fn add_user_message(&mut self, content: Vec<ContentBlock>) {
        self.session.add_user_message(content).await;
    }

    /// Send a user message with delivery options (maps to
    /// `AgentSession::send_user_message`).
    ///
    /// `deliver_as` is the streaming delivery mode: `"steer"` (interrupt the
    /// running turn) or `"followUp"` (queue after the current turn). Pass
    /// `None` for the default.
    pub async fn send_user_message(&self, content: &str, deliver_as: Option<&str>) {
        use pi_coding_agent::core::agent_session::SendUserMessageOptions;
        self.session
            .send_user_message(
                content,
                Some(SendUserMessageOptions {
                    deliver_as: deliver_as.map(str::to_owned),
                }),
            )
            .await;
    }

    // ── Prompting / control ──────────────────────────────────────────────

    /// Run a full prompt turn (appends the text, runs the agent, then the
    /// post-agent retry/compaction loop). Returns when the turn completes.
    ///
    /// `AgentSession::prompt` holds a `std::sync::MutexGuard` across `.await`
    /// points, making the future `!Send`; callers that need `Send` should use
    /// [`Self::process_chat`] which isolates it via `spawn_blocking`.
    pub async fn prompt(&self, text: &str) {
        self.session.prompt(text, None).await;
    }

    /// Abort the running agent turn.
    pub async fn abort(&self) {
        self.session.abort().await;
    }

    /// Retry the last turn. Returns the new messages produced.
    pub async fn retry(&self) -> std::result::Result<Vec<AgentMessage>, String> {
        self.session.retry().await.map_err(|e| e.to_string())
    }

    /// Abort an in-progress retry backoff.
    pub fn abort_retry(&self) {
        self.session.abort_retry();
    }

    /// Cancel an in-progress (manual or auto) compaction.
    pub fn abort_compaction(&self) {
        self.session.abort_compaction();
    }

    // ── Model / thinking ──────────────────────────────────────────────────

    pub async fn model(&self) -> Model {
        self.session.get_model().await
    }

    /// Set the active model. Requires unique access.
    pub async fn set_model(&mut self, model: Model) -> std::result::Result<(), String> {
        self.session.set_model(model).await
    }

    /// Cycle to the next/previous scoped model. Requires unique access.
    /// Returns `(model, thinking_level, supports_thinking)` on success.
    pub async fn cycle_model(
        &mut self,
        direction: &str,
    ) -> Option<(Model, Option<ThinkingLevel>, bool)> {
        self.session.cycle_model(direction).await
    }

    pub async fn thinking_level(&self) -> ThinkingLevel {
        self.session.get_thinking_level().await
    }

    /// Set the thinking level (e.g. `"low"`/`"medium"`/`"high"`).
    pub async fn set_thinking_level(&mut self, level: &str) {
        self.session.set_thinking_level(level).await;
    }

    /// Cycle to the next thinking level. Requires unique access.
    pub async fn cycle_thinking_level(&mut self) -> Option<String> {
        self.session.cycle_thinking_level().await
    }

    pub async fn available_thinking_levels(&self) -> Vec<&'static str> {
        self.session.get_available_thinking_levels().await
    }

    pub async fn supports_thinking(&self) -> bool {
        self.session.supports_thinking().await
    }

    /// The configured scoped models (cloned).
    #[must_use]
    pub fn scoped_models(&self) -> Vec<(Model, Option<ThinkingLevel>)> {
        self.session.get_scoped_models().to_vec()
    }

    /// Replace the scoped models. Requires unique access.
    pub fn set_scoped_models(&mut self, models: Vec<(Model, Option<ThinkingLevel>)>) {
        self.session.set_scoped_models(models);
    }

    /// Borrow the model registry (used for resolving model specs from CLI/UI).
    #[must_use]
    pub fn model_registry(&self) -> &ModelRegistry {
        self.session.get_model_registry()
    }

    // ── Compaction ───────────────────────────────────────────────────────

    pub async fn should_compact(&self) -> bool {
        self.session.should_compact().await
    }

    pub async fn check_auto_compact(&self) -> bool {
        self.session.check_auto_compact().await
    }

    /// Compact the session context. Returns the compaction summary text.
    pub async fn compact(
        &self,
        custom_instructions: Option<&str>,
    ) -> std::result::Result<String, String> {
        self.session.compact(custom_instructions).await
    }

    #[must_use]
    pub fn compaction_settings(&self) -> CompactionSettings {
        self.session.get_compaction_settings().clone()
    }

    pub fn set_compaction_settings(&mut self, settings: CompactionSettings) {
        self.session.set_compaction_settings(settings);
    }

    // ── Auto-retry ───────────────────────────────────────────────────────

    #[must_use]
    pub fn auto_retry_enabled(&self) -> bool {
        self.session.auto_retry_enabled()
    }

    pub fn set_auto_retry_enabled(&self, enabled: bool) {
        self.session.set_auto_retry_enabled(enabled);
    }

    #[must_use]
    pub fn retry_attempt(&self) -> u32 {
        self.session.retry_attempt()
    }

    // ── Steering / follow-up queues ──────────────────────────────────────

    pub async fn steer(&self, text: &str, images: Option<Vec<ContentBlock>>) {
        self.session.steer(text, images).await;
    }

    pub async fn follow_up(&self, text: &str, images: Option<Vec<ContentBlock>>) {
        self.session.follow_up(text, images).await;
    }

    #[must_use]
    pub fn steering_messages(&self) -> Vec<String> {
        self.session.get_steering_messages()
    }

    #[must_use]
    pub fn follow_up_messages(&self) -> Vec<String> {
        self.session.get_follow_up_messages()
    }

    pub async fn has_queued_messages(&self) -> bool {
        self.session.has_queued_messages().await
    }

    pub async fn clear_all_queues(&self) -> (Vec<String>, Vec<String>) {
        self.session.clear_all_queues().await
    }

    pub async fn steering_mode(&self) -> QueueMode {
        self.session.steering_mode().await
    }

    pub async fn follow_up_mode(&self) -> QueueMode {
        self.session.follow_up_mode().await
    }

    // ── Tools ────────────────────────────────────────────────────────────

    pub async fn active_tool_names(&self) -> Vec<String> {
        self.session.get_active_tool_names().await
    }

    pub async fn set_active_tools_by_name(&self, tool_names: &[String]) {
        self.session.set_active_tools_by_name(tool_names).await;
    }

    #[must_use]
    pub fn all_tools(&self) -> Vec<ToolInfo> {
        self.session.get_all_tools()
    }

    #[must_use]
    pub fn tool_definition(&self, name: &str) -> Option<&ToolDefinition> {
        self.session.get_tool_definition(name)
    }

    // ── Event subscription ───────────────────────────────────────────────

    /// Subscribe a raw async `AgentEvent` listener. Returns an unsubscribe
    /// handle.
    ///
    /// The listener must be an `Arc`-wrapped closure matching
    /// pi-coding-agent's async listener shape:
    /// `Arc<dyn Fn(AgentEvent, Option<watch::Receiver<bool>>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>`.
    /// pi-coding-agent does not export the type alias, so the equivalent
    /// `Arc<dyn ...>` signature is spelled out here; it is structurally
    /// identical to the underlying `AgentEventListener` type.
    pub async fn subscribe(&self, listener: AgentEventListener) -> UnsubscribeHandle {
        self.session.subscribe(listener).await
    }

    /// Subscribe a sync `AgentSessionEvent` listener. Returns an unsubscribe
    /// handle.
    pub fn subscribe_session_events(
        &self,
        listener: AgentSessionEventListener,
    ) -> SessionEventUnsubscribeHandle {
        self.session.subscribe_session_events(listener)
    }

    /// Convenience: subscribe to `AgentEvent` and forward every event into an
    /// mpsc channel. The returned handle keeps the listener alive while held.
    pub async fn subscribe_to_channel(&self, tx: mpsc::Sender<AgentEvent>) -> UnsubscribeHandle {
        let event_tx = tx;
        self.session
            .subscribe(Arc::new(
                move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
                    let tx = event_tx.clone();
                    Box::pin(async move {
                        let _ = tx.send(event).await;
                    }) as Pin<Box<dyn Future<Output = ()> + Send>>
                },
            ))
            .await
    }

    // ── Session switching / resume ───────────────────────────────────────

    /// Switch to an existing session JSONL file (resume). Requires unique
    /// access.
    pub async fn switch_session(
        &mut self,
        session_path: &str,
        cwd_override: Option<&str>,
    ) -> std::result::Result<(), String> {
        self.session
            .switch_session(session_path, cwd_override)
            .await
    }

    /// Start a fresh session (optionally parented to `parent_session`).
    /// Requires unique access.
    pub async fn new_session(&mut self, parent_session: Option<&str>) {
        self.session.new_session(parent_session).await;
    }

    /// Fork the session at `entry_id`. Returns the new session file path.
    /// Requires unique access.
    pub async fn fork_session(&mut self, entry_id: &str) -> std::result::Result<String, String> {
        self.session.fork_session(entry_id).await
    }

    // ── System prompt / extensions ───────────────────────────────────────

    pub async fn system_prompt(&self) -> String {
        self.session.get_system_prompt().await
    }

    #[must_use]
    pub fn extension_registry(&self) -> Option<Arc<ExtensionRegistry>> {
        self.session.get_extension_registry()
    }

    #[must_use]
    pub fn prompt_templates(&self) -> Vec<PromptTemplate> {
        self.session.prompt_templates()
    }

    // ── High-level run helpers (Send-safe) ────────────────────────────────

    /// Run the agent and return the final text reply.
    ///
    /// `prompt()` on `AgentSession` holds `std::sync::MutexGuard` across
    /// `.await` points, making the future `!Send`. We isolate it via
    /// `spawn_blocking` so callers can use this in `tokio::spawn` contexts.
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
