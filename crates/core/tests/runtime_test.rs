// ============================================================================
// Tests
// ============================================================================

use openatrading_core::llm::AgentRuntime;
use openatrading_core::platform::cli::{NoopPersister, StdoutEmitter};
use openatrading_extension::create_registry;
use std::sync::Arc;

#[tokio::test]
async fn test_create_agent_runtime_with_extension() {
    let ext_registry = create_registry();
    // Arc::new(StdoutEmitter::new(false)),
    // Arc::new(NoopPersister),

    let runtime = AgentRuntime::new(Arc::new(StdoutEmitter::new(false)), Arc::new(NoopPersister));
    let (session, _result) = create_agent_session(CreateAgentSessionOptions {
        cwd: ".".to_string(),
        agent_dir: None,
        model: None,
        thinking_level: None,
        scoped_models: None,
        no_tools: None,
        tools: None,
        exclude_tools: None,
        custom_prompt: None,
        append_system_prompt: None,
        session_name: None,
        stream_fn: None,
        convert_to_llm: None,
        custom_tools: None,
        extension_paths: Vec::new(),
        enable_extensions: true,
        extension_registry: Some(ext_registry),
        cli_provider: None,
        cli_model: None,
        persist_session: true,
        session_file: None,
        fork_from: None,
        session_dir: None,
        auth_storage: None,
        model_registry: None,
        resource_loader: None,
        session_manager: None,
        settings_manager: None,
        session_start_event: None,
    })
    .await
    .expect("create_agent_session failed");
    assert!(session.get_extension_registry().is_some());
    let active_tool_names = session.get_active_tool_names().await;
    println!("[dbg] active tools: {:#?}", active_tool_names);
    let all_tools = session.get_all_tools();
    println!(
        "[dbg] all tools: {:#?}",
        all_tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // Find and invoke the `create_goal` extension tool through the active tool list.
    let agent_state = session.get_agent().state().await;
    let create_goal = agent_state
        .tools
        .iter()
        .find(|t| t.name == "create_goal")
        .expect("create_goal tool should be active");
    let params =
        serde_json::json!({ "objective": "Write a test that exercises the goal extension" });
    let result = (create_goal.execute)("call-create-goal-1".to_string(), params, None, None)
        .await
        .expect("create_goal execute failed");
    let goal_text = result
        .content
        .iter()
        .filter_map(|c| {
            if let pi_agent_core::pi_ai_types::ContentBlock::Text { text, .. } = c {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    println!("[dbg] create_goal result: {}", goal_text);
}
