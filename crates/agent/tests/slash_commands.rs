//! Regression tests for extension slash-command wiring.
//!
//! The frontend slash menu lists commands from `GET /api/ai/commands`, which
//! must match what a live session can actually execute. These tests pin the
//! two halves of that contract:
//!
//! 1. [`registry_has_goal_command`] — `create_registry()` (the same builder
//!    the web gateway and MCP server use) collects pi-extensions commands
//!    such as `/goal`.
//! 2. [`factory_produces_session_with_goal_command`] — a session created via
//!    `create_factory()` (the exact path the session pool uses) carries the
//!    extension registry, so pi-rs `prompt()` will dispatch `/goal` instead
//!    of treating it as a normal user prompt.

use openatrading_agent::session::{create_factory, CreateSessionParams, FactoryOptions};
use pi_coding_agent::core::session_manager::SessionManager;

#[test]
fn create_registry_collects_extension_commands() {
    let mut registry =
        openatrading_extension::create_registry(openatrading_extension::RegistryOptions {
            trading: None,
            mcp: None,
        });
    let resolved = pi_coding_agent::core::slash_commands::resolve_extension_commands(
        &registry.collect_commands(),
    );
    let names: Vec<String> = resolved.into_iter().map(|c| c.invocation_name).collect();
    assert!(
        names.contains(&"goal".to_string()),
        "registry commands: {names:?}"
    );
}

#[test]
fn factory_produces_session_with_goal_command() {
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
        let factory = create_factory(FactoryOptions::market_only(agent_dir.clone()));
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
        let names: Vec<String> = registry.commands().iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"goal".to_string()), "commands: {names:?}");
    });
}
