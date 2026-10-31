//! Regression tests for extension registry wiring.
//!
//! `create_registry()` is the single builder shared by the web gateway, the
//! MCP server and the session factory. These tests pin the two halves of the
//! contract that the settings UI depends on:
//!
//! 1. [`create_registry_collects_extension_tools`] — the registry exposes the
//!    tools of every built-in extension.
//! 2. [`factory_produces_session_with_extension_tools`] — a session created
//!    via `create_factory()` (the exact path the session pool uses) carries
//!    the same registry, so the toggles in `settings.json` act on real tools.

use openatrading_agent::session::{create_factory, CreateSessionParams, FactoryOptions};
use pi_coding_agent::core::session_manager::SessionManager;

#[test]
fn create_registry_collects_extension_tools() {
    let mut registry = openatrading_extension::create_registry();
    let names: Vec<String> = registry
        .collect_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();

    for expected in ["subagent", "web_search", "web_fetch"] {
        assert!(
            names.contains(&expected.to_string()),
            "missing extension tool {expected}: {names:?}"
        );
    }
    assert!(
        !names.iter().any(|name| name.starts_with("goal_")),
        "pi-rs v1.87.10 removed the goal extension: {names:?}"
    );
}

#[test]
fn factory_produces_session_with_extension_tools() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => panic!("failed to build runtime: {e}"),
    };
    runtime.block_on(async {
        // Use a throwaway agent dir under /tmp so the test never writes into
        // the repo (pi-rs creates auth.json under the agent dir).
        let agent_dir = std::env::temp_dir()
            .join(format!("pi-rs-agent-{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        let factory = create_factory(FactoryOptions::new(agent_dir.clone()));
        let cwd = std::env::temp_dir().to_string_lossy().to_string();
        let sessions_dir = pi_coding_agent::config::get_sessions_dir()
            .to_string_lossy()
            .to_string();
        let params = CreateSessionParams {
            cwd: cwd.clone(),
            agent_dir,
            exclude_tools: None,
            session_manager: SessionManager::new(&cwd, &sessions_dir, None, false, None),
            session_start_event: None,
        };
        let created = match factory(params).await {
            Ok(c) => c,
            Err(e) => panic!("failed to create session: {e}"),
        };
        let registry = match created.session.get_extension_registry() {
            Some(r) => r,
            None => panic!("session has no extension registry"),
        };
        let names: Vec<String> = registry.tools().iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"subagent".to_string()), "tools: {names:?}");
    });
}
