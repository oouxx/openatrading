use anyhow::Result;

// ── create_session ──────────────────────────────────────

/// Create a new empty pi-coding-agent session and return its info.
///
/// Returns `(file_stem, session_id)` where `file_stem` is the JSONL file stem
/// used as the session identifier by callers that persist conversations.
///
/// Note: for new streaming flows, prefer [`crate::llm::AgentRuntime::create_session`]
/// which returns a [`crate::llm::SessionHandle`] and supports `subscribe`/`send`.
/// This function is retained for the legacy "create empty session" HTTP/IPC
/// endpoints that only need a session id up front.
pub fn create_session() -> Result<(String, String)> {
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_dir = format!("{cwd}/.pi-rs/agent/sessions");

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
