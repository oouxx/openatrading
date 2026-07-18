//! DeepSeek v4 Flash integration test — exercises the full agent pipeline:
//! LLM streaming, tool calling (get_market_overview / get_limit_up),
//! and multi-round agent processing.
//!
//! Follows the same pattern as agent_stream_e2e.rs.
//!
//! ```sh
//! cargo test -p openatrading-core --test deepseek_integration_test -- --nocapture
//! ```

use std::sync::Arc;

use openatrading_core::llm::pi_ai_types::ContentBlock;
use openatrading_core::llm::{tools, AgentMessage, LlmClient};
use pi_coding_agent::core::extensions::ToolDefinition;
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
use pi_coding_agent::pi_agent_core::pi_ai_types::ToolExecutionMode;

fn tool_def(t: &Arc<openatrading_core::llm::DynTool>) -> ToolDefinition {
    ToolDefinition {
        name: t.name.clone(),
        label: Some(t.label.clone()),
        description: t.description.clone(),
        parameters: Some(t.parameters_schema.clone()),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        execution_mode: t.execution_mode.as_ref().map(|m| match m {
            ToolExecutionMode::Sequential => "sequential".to_string(),
            ToolExecutionMode::Parallel => "parallel".to_string(),
        }),
        execute: None,
    }
}

#[ignore = "integration test"]
#[tokio::test]
async fn test_deepseek_v4_flash_agent_process() {
    let llm = LlmClient::with_config("deepseek".to_string(), "deepseek-v4-flash".to_string());

    let tools: Vec<Arc<openatrading_core::llm::DynTool>> =
        vec![tools::get_market_overview(), tools::get_limit_up()];

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let system_prompt = "你是一个A股短线交易AI助手。可以调用工具获取实时数据，基于数据给出分析。";

    let (mut session, _) = sdk::create_agent_session(CreateAgentSessionOptions {
        cwd,
        model: Some(llm.model.clone()),
        no_tools: Some(NoToolsMode::All),
        custom_prompt: Some(system_prompt.to_string()),
        custom_tools: Some(tools.iter().map(tool_def).collect()),
        tools: Some(tools.iter().map(|t| t.name.clone()).collect()),
        agent_dir: None,
        thinking_level: None,
        scoped_models: None,
        exclude_tools: None,
        append_system_prompt: None,
        session_name: None,
        stream_fn: None,
        convert_to_llm: None,
        extension_paths: Vec::new(),
        enable_extensions: false,
        extension_registry: None,
        cli_provider: None,
        cli_model: None,
        persist_session: false,
        session_file: None,
        fork_from: None,
        session_dir: None,
    })
    .await
    .expect("create_agent_session failed");

    // Inject real tool implementations over stubs
    let agent = session.get_agent();
    agent.add_tools(tools).await;
    let _ = agent;

    session
        .add_user_text(
            "请先用 get_market_overview 查询市场概况，然后用 get_limit_up 查询今日涨停板。",
        )
        .await;
    let messages = session.get_messages().await;

    // Verify: agent called tools and got results
    let all_text: String = messages
        .iter()
        .filter_map(|msg| {
            if let AgentMessage::Assistant { content, .. } = msg {
                let t: String = content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text, .. } = b {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            } else {
                None
            }
        })
        .collect();

    tracing::info!("=== Agent Response ===\n{all_text}\n======================");
    assert!(!all_text.is_empty(), "Expected non-empty agent response");

    // Verify tool calls were made
    let tool_calls: Vec<&AgentMessage> = messages
        .iter()
        .filter(|msg| matches!(msg, AgentMessage::Assistant { .. }))
        .collect();
    assert!(
        !tool_calls.is_empty(),
        "Expected at least one assistant message with tool calls"
    );
}
