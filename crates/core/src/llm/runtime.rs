use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use futures::Future;
use openatrading_extension::create_registry;
use tokio::sync::{mpsc, watch};

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::types::{AgentEvent, AgentMessage};
use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::sdk::{self, prelude::SettingsManager, CreateAgentSessionOptions};
use pi_extension_api::ToolDefinition;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

use pi_coding_agent::core::agent_session_runtime::{
    create_agent_session_runtime, AgentSessionRuntime, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeParams, CreateAgentSessionRuntimeResult,
};
use pi_coding_agent::core::agent_session_services::{
    create_agent_session_from_services, create_agent_session_services,
    CreateAgentSessionFromServicesOptions, CreateAgentSessionServicesOptions,
};
use serde_json::json;

use super::cwd::{decide_cwd_action, resolve_session_cwd, CwdAction};
use super::transcript::{build_display_transcript, serialize_event};
use super::types::{DesktopState, FrontendEvent, GlobalModelSettings, SessionRecord};

// ── Platform abstraction ────────────────────────────────────────────────
//
// Everything that talks to a specific runtime environment (Tauri window /
// HTTP+SSE connection / whatever) goes through these two traits. Desktop and
// web each provide their own implementation; AgentRuntimeCore never touches
// tauri::AppHandle or any transport type directly.

#[async_trait]
pub trait EventEmitter: Send + Sync {
    async fn emit_agent_event(&self, event: FrontendEvent);
    async fn emit_state_changed(&self, state: &DesktopState);
    async fn emit_transcript_changed(&self, session_id: &str, transcript: Vec<serde_json::Value>);
}

#[async_trait]
pub trait StatePersister: Send + Sync {
    async fn persist(&self, state: &DesktopState);
}

// ── ActiveSession ────────────────────────────────────────────────────────
//
// One entry per live agent session. Replacing the old single
// `Arc<Mutex<Option<AgentSessionRuntime>>>` + generation counter with a map
// keyed by session_id means switching the "selected" session in the UI no
// longer needs to abort/discard/rebuild anything — the previous session's
// runtime just stays parked in the map and keeps streaming if it was.

struct ActiveSession {
    runtime: Option<AgentSessionRuntime>,
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

// ── AgentRuntimeCore ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentRuntime {
    pub state: Arc<Mutex<DesktopState>>,
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    emitter: Arc<dyn EventEmitter>,
    persister: Arc<dyn StatePersister>,
}

impl AgentRuntime {
    pub fn new(emitter: Arc<dyn EventEmitter>, persister: Arc<dyn StatePersister>) -> Self {
        Self {
            state: Arc::new(Mutex::new(DesktopState {
                revision: 1,
                sessions: vec![],
                selected_session_id: String::new(),
                global_model_settings: GlobalModelSettings {
                    enabled_model_patterns: vec![],
                    default_provider: None,
                    default_model_id: None,
                    default_thinking_level: None,
                },
            })),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            emitter,
            persister,
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
            custom_tools: custom_tools,
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

    /// Build the runtime factory closure that creates AgentSessions.
    /// No longer captures AppHandle — captures the emitter instead.
    fn build_runtime_factory(self: &Arc<Self>) -> CreateAgentSessionRuntimeFactory {
        let store = self.clone();
        let emitter = self.emitter.clone();
        Box::new(move |params: CreateAgentSessionRuntimeParams| {
            let store = store.clone();
            let emitter = emitter.clone();
            Box::pin(async move {
                let mut services =
                    create_agent_session_services(CreateAgentSessionServicesOptions {
                        cwd: params.cwd.clone(),
                        agent_dir: Some(params.agent_dir.clone()),
                        auth_storage: None,
                        settings_manager: None,
                        model_registry: None,
                        resource_loader_options: None,
                    })
                    .await;

                services.model_registry = pi_coding_agent::core::model_registry::ModelRegistry::new(
                    pi_coding_agent::core::model_registry::ModelRegistry::builtin_models_list(),
                );

                let (provider, model_id, thinking_level) = {
                    let settings = services.settings_manager.get_settings();
                    (
                        settings.default_provider.clone(),
                        settings.default_model.clone(),
                        settings.thinking_level.clone(),
                    )
                };

                let registry = &services.model_registry;
                let initial_model = provider
                    .as_ref()
                    .and_then(|p| model_id.as_ref().and_then(|m| registry.find(p, m)));

                let extension_registry = create_registry();
                let model = initial_model.unwrap_or_else(|| {
                    let available = registry.get_available();
                    available.into_iter().next().expect("No models available")
                });

                let result_cwd = services.cwd.clone();
                let result_agent_dir = services.agent_dir.clone();

                let (mut session, result) =
                    create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                        services,
                        session_manager: params.session_manager,
                        model: Some(model),
                        thinking_level,
                        scoped_models: None,
                        tools: None,
                        no_tools: None,
                        extension_registry: Some(extension_registry),
                        fallback_message: None,
                    })
                    .await
                    .expect("Failed to create agent session");

                let result_services =
                    create_agent_session_services(CreateAgentSessionServicesOptions {
                        cwd: result_cwd,
                        agent_dir: Some(result_agent_dir),
                        auth_storage: None,
                        settings_manager: None,
                        model_registry: None,
                        resource_loader_options: None,
                    })
                    .await;

                let sid = session.get_session_id();
                let store2 = store.clone();
                let emitter2 = emitter.clone();
                let sid2 = sid.clone();
                session
                    .subscribe(Arc::new(move |event: AgentEvent, _signal| {
                        let store = store2.clone();
                        let emitter = emitter2.clone();
                        let sid = sid2.clone();
                        Box::pin(async move {
                            let (et, data) = serialize_event(&event);
                            if et == "agent_start" || et == "turn_start" {
                                store.mutate(|s| set_sess_status(s, &sid, "running")).await;
                            } else if et == "agent_end" || et == "turn_end" {
                                store.mutate(|s| set_sess_status(s, &sid, "idle")).await;
                            }
                            if et == "tool_execution_start" {
                                eprintln!(
                                    "[TOOL] start sid={} id={} name={}",
                                    sid,
                                    data.get("tool_call_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    data.get("tool_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?"),
                                );
                            } else if et == "tool_execution_end" {
                                let is_error = data
                                    .get("is_error")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let result_str = data
                                    .get("result")
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .unwrap_or_else(|| {
                                        data.get("result")
                                            .map(|v| v.to_string())
                                            .unwrap_or_default()
                                    });
                                let snippet: String = result_str.chars().take(160).collect();
                                eprintln!(
                                    "[TOOL] end   sid={} id={} name={} is_error={} result={:?}",
                                    sid,
                                    data.get("tool_call_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    data.get("tool_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?"),
                                    is_error,
                                    snippet,
                                );
                            }
                            emitter
                                .emit_agent_event(FrontendEvent {
                                    event_type: et,
                                    session_id: sid,
                                    data,
                                })
                                .await;
                        })
                    }))
                    .await;

                CreateAgentSessionRuntimeResult {
                    session,
                    services: result_services,
                    diagnostics: vec![],
                    model_fallback_message: result.model_fallback_message,
                }
            })
        })
    }

    pub async fn mutate<F>(self: &Arc<Self>, f: F) -> DesktopState
    where
        F: FnOnce(&mut DesktopState),
    {
        let mut state = self.state.lock().await;
        f(&mut state);
        state.revision += 1;
        let result = state.clone();
        drop(state);
        self.emitter.emit_state_changed(&result).await;
        self.persister.persist(&result).await;
        result
    }

    /// Initialize the AgentSessionRuntime for `session_id`, replacing whatever
    /// (if anything) was already parked there. Only touches that one entry.
    async fn init_runtime(
        self: &Arc<Self>,
        session_id: &str,
        cwd: &str,
        session_file: Option<&str>,
    ) -> Result<(), String> {
        eprintln!(
            "[CWD] init_runtime sid={session_id} cwd={:?} exists={}",
            cwd,
            std::path::Path::new(cwd).exists()
        );
        if cwd.is_empty() {
            eprintln!("[CWD] WARNING cwd is empty — bash tool will fail");
        } else if !std::path::Path::new(cwd).exists() {
            eprintln!("[CWD] WARNING cwd does not exist — bash tool will fail");
        }

        let agent_dir = pi_coding_agent::config::get_agent_dir()
            .to_string_lossy()
            .to_string();
        let session_dir =
            pi_coding_agent::core::session_manager::SessionManager::default_session_dir(
                cwd, &agent_dir,
            );
        let session_manager = pi_coding_agent::core::session_manager::SessionManager::new(
            cwd,
            &session_dir,
            session_file,
            true,
            None,
        );

        let factory = self.build_runtime_factory();
        let runtime = create_agent_session_runtime(
            factory,
            CreateAgentSessionRuntimeParams {
                cwd: cwd.to_string(),
                agent_dir,
                session_manager,
            },
        )
        .await;

        // The runtime may report a different (renamed/derived) session id than
        // the caller's key — store it under the caller's key regardless, since
        // that's what the rest of the app addresses it by.
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .entry(session_id.to_string())
            .or_insert_with(ActiveSession::new);
        entry.runtime = Some(runtime);
        Ok(())
    }

    pub async fn send_message(
        self: &Arc<Self>,
        session_id: &str,
        text: &str,
    ) -> Result<(), String> {
        // Lazily init runtime if not yet created for this session.
        let needs_init = { !self.sessions.lock().await.contains_key(session_id) };
        if needs_init {
            let (cwd, session_file) = {
                let state = self.state.lock().await;
                match state.sessions.iter().find(|s| s.id == session_id) {
                    Some(s) => (
                        resolve_session_cwd(s.cwd.as_deref()),
                        s.session_file.clone().filter(|f| !f.is_empty()),
                    ),
                    None => (resolve_session_cwd(None), None),
                }
            };
            self.init_runtime(session_id, &cwd, session_file.as_deref())
                .await?;
        }

        // Take this session's runtime out of the map so the spawned task owns
        // it exclusively while streaming — other sessions in the map are
        // untouched and can be driven concurrently.
        let (mut runtime, is_streaming, abort_flag) = {
            let mut sessions = self.sessions.lock().await;
            let entry = sessions.get_mut(session_id).ok_or("No session")?;
            let runtime = entry.runtime.take().ok_or("Session already streaming")?;
            (
                runtime,
                entry.is_streaming.clone(),
                entry.abort_flag.clone(),
            )
        };

        abort_flag.store(false, Ordering::SeqCst);
        is_streaming.store(true, Ordering::SeqCst);

        let s = self.clone();
        let emitter = self.emitter.clone();
        let t = text.to_string();
        let sid = session_id.to_string();
        let sid2 = sid.clone();

        emitter
            .emit_agent_event(FrontendEvent {
                event_type: "user_message".into(),
                session_id: sid.clone(),
                data: json!({"text": text, "timestamp": chrono::Utc::now().timestamp_millis()}),
            })
            .await;

        let diag_provider;
        let diag_model;
        {
            let agent_dir = pi_coding_agent::config::get_agent_dir();
            let mgr = pi_coding_agent::core::settings_manager::SettingsManager::create(
                agent_dir.to_string_lossy().as_ref(),
                Some(agent_dir.to_string_lossy().as_ref()),
            );
            let settings = mgr.get_settings();
            diag_provider = settings.default_provider.clone();
            diag_model = settings.default_model.clone();
        }
        eprintln!("[LLM] send: provider={diag_provider:?} model={diag_model:?}");

        tokio::spawn(async move {
            if !abort_flag.load(Ordering::SeqCst) {
                eprintln!("[LLM] <<< {}", &t);
                let agent_fut = runtime.session_mut().add_user_text(&t);
                if tokio::time::timeout(Duration::from_secs(300), agent_fut)
                    .await
                    .is_err()
                {
                    eprintln!("[LLM] add_user_text timed out after 300s, aborting");
                    runtime.session().abort().await;
                }
            }
            eprintln!("[LLM] add_user_text done");

            // Put the runtime back under its own key. Because each session
            // has its own map entry, there's no cross-session clobbering to
            // guard against — no generation counter needed. If the entry was
            // removed entirely in the meantime (e.g. session deleted), just
            // drop the runtime.
            {
                let mut sessions = s.sessions.lock().await;
                if let Some(entry) = sessions.get_mut(&sid) {
                    entry.runtime = Some(runtime);
                }
            }
            is_streaming.store(false, Ordering::SeqCst);

            let msgs2 = s.get_messages(&sid).await;
            let transcript = build_display_transcript(&msgs2);
            if !transcript.is_empty() {
                emitter.emit_transcript_changed(&sid2, transcript).await;
            }
        });
        Ok(())
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
            move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
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
        user_message: &str,
    ) -> Result<crate::roles::prompt::DecisionOutput> {
        todo!("需要重新设计llm结构化输出提取以及重试机制");
    }

    /// Run the agent and return the final text reply.
    pub async fn process_chat(&self, user_message: &str) -> Result<String> {
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

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn set_sess_status(s: &mut DesktopState, sid: &str, status: &str) {
    if let Some(sess) = s.sessions.iter_mut().find(|s| s.id == sid) {
        sess.status = status.to_string();
    }
}
