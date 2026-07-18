use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::Future;
use tokio::sync::{mpsc, watch};

use openatrading_agent::pi_ai_types::ContentBlock;
use openatrading_agent::pi_ai_types::ToolExecutionMode;
use openatrading_agent::types::{AgentEvent, AgentMessage, DynTool};
use pi_coding_agent::core::extensions::ToolDefinition;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_extension_api::ToolExecuteFn;

// ── AgentResult ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentResult {
    pub final_text: String,
    pub tool_outputs: Vec<serde_json::Value>,
}

// ── Tool definition helper ──────────────────────────────────────────────

fn build_tool_defs(trading_tools: &[Arc<DynTool>]) -> Vec<ToolDefinition> {
    trading_tools
        .iter()
        .map(|t| {
            let exec: ToolExecuteFn = {
                let inner = t.execute.clone();
                Arc::new(move |id, params, signal| {
                    let inner = inner.clone();
                    Box::pin(async move {
                        let result = inner(id, params, signal, None).await?;
                        let content: Vec<serde_json::Value> = result
                            .content
                            .iter()
                            .filter_map(|cb| serde_json::to_value(cb).ok())
                            .collect();
                        Ok(pi_extension_api::ToolCallOutput {
                            content,
                            details: Some(result.details),
                            is_error: false,
                        })
                    })
                })
            };
            ToolDefinition {
                name: t.name.clone(),
                label: Some(t.label.clone()),
                description: t.description.clone(),
                prompt_snippet: None,
                prompt_guidelines: None,
                parameters: Some(t.parameters_schema.clone()),
                render_shell: None,
                execution_mode: t.execution_mode.as_ref().map(|m| match m {
                    ToolExecutionMode::Sequential => "sequential".to_string(),
                    ToolExecutionMode::Parallel => "parallel".to_string(),
                }),
                execute: Some(exec),
            }
        })
        .collect()
}

// ── create_session ──────────────────────────────────────

/// Create a new empty pi-coding-agent session and return its info.
pub fn create_session() -> Result<(String, String)> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_dir = format!("{cwd}/.pi-rs/sessions");

    use pi_coding_agent::core::session_manager::SessionManager;
    let sm = SessionManager::new(&cwd, &session_dir, None, true, None);
    let sid = sm.get_session_id().to_string();
    let file_stem = sm
        .get_session_file()
        .and_then(|p| {
            let p: &std::path::Path = p;
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| sid.clone());
    Ok((file_stem, sid))
}

// ── process_messages ────────────────────────────────────────────────────

/// Process a user message through a pi-coding-agent session.
///
/// ## Session lifecycle
/// - `session_id = None`  → creates a new session, persists to JSONL, returns its ID
/// - `session_id = Some(id)` → loads existing session from JSONL, appends the message
///
/// The session_id is the **file stem** of the JSONL file on disk (e.g. `2026-07-18T12-34-56_uuid`).
/// Callers should persist it and pass it back on subsequent messages to continue the conversation.
pub async fn process_messages(
    system_prompt: &str,
    trading_tools: Vec<Arc<DynTool>>,
    messages: Vec<AgentMessage>,
    tx: mpsc::Sender<AgentEvent>,
    session_id: Option<String>,
) -> Result<(AgentResult, String)> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let tool_defs = build_tool_defs(&trading_tools);
    let ext_registry = openatrading_extension::create_registry();

    let session_dir = format!("{cwd}/.pi-rs/sessions");

    // Sanitize user-provided session_id: reject empty, too long, path separators, null bytes
    let sid = match session_id {
        Some(ref id)
            if id.is_empty()
                || id.len() > 256
                || id.contains('/')
                || id.contains('\\')
                || id.contains("..")
                || id.contains('\0') =>
        {
            return Err(anyhow::anyhow!("invalid session_id"))
        },
        Some(id) => id,
        None => pi_coding_agent::core::session_manager::derive_short_session_id(),
    };

    // session_file: when the file already exists, pi-coding-agent loads it.
    // When it doesn't, pi-coding-agent creates a new session with auto-generated
    // filename `{timestamp}_{uuid}.jsonl`. We query the actual path after creation
    // and return the real file stem.
    let session_file = format!("{session_dir}/{sid}.jsonl");

    let (session, _result) = sdk::create_agent_session(CreateAgentSessionOptions {
        cwd: cwd.clone(),
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
        custom_tools: Some(tool_defs),
        extension_paths: Vec::new(),
        enable_extensions: false,
        extension_registry: Some(ext_registry),
        cli_provider: None,
        cli_model: None,
        persist_session: true,
        session_file: Some(session_file.clone()),
        fork_from: None,
        session_dir: Some(session_dir.clone()),
    })
    .await
    .map_err(|e| anyhow::anyhow!("create_agent_session failed: {e}"))?;

    // Get the actual session file stem (pi-coding-agent may have renamed the file
    // to `{timestamp}_{uuid}.jsonl`). Return the stem so the caller can pass it
    // back on the next request and we can reconstruct the correct file path.
    let actual_id = session
        .get_session_file()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or(sid);

    // Subscribe → forward all AgentEvents to caller's channel
    let event_tx = tx.clone();
    let listener = Arc::new(
        move |event: AgentEvent, _signal: Option<watch::Receiver<bool>>| {
            let tx = event_tx.clone();
            Box::pin(async move {
                let _ = tx.send(event).await;
            }) as Pin<Box<dyn Future<Output = ()> + Send>>
        },
    );

    let _handle = {
        let agent = session.get_agent();
        agent.subscribe(listener).await
    };

    // Process with 300s timeout
    let process_result = {
        let agent = session.get_agent();
        let process_fut = agent.process(messages);
        tokio::time::timeout(Duration::from_secs(300), process_fut).await
    };

    match process_result {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(anyhow::anyhow!("agent process failed: {e}")),
        Err(_elapsed) => {
            tracing::warn!("process_messages timed out after 300s, aborting");
            {
                let agent = session.get_agent();
                agent.abort().await;
            }
            return Err(anyhow::anyhow!("agent processing timed out after 300s"));
        },
    }

    let all_messages = session.get_messages().await;

    let mut final_text = String::new();
    let mut tool_outputs: Vec<serde_json::Value> = Vec::new();

    for msg in &all_messages {
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

    Ok((
        AgentResult {
            final_text,
            tool_outputs,
        },
        actual_id,
    ))
}
