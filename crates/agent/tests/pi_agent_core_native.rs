//! Smoke tests for the agent crate's pi-agent-core native API.
//!
//! These tests verify that the agent crate can construct a `pi_agent_core::Agent`
//! from an `LlmClient` and a `Vec<Arc<AgentTool>>`, using only the native
//! `pi-agent-core` types — no custom tool abstractions.

use std::sync::Arc;

use openatrading_agent::runner::create_agent;
use openatrading_provider::llm::LlmClient;
use pi_agent_core::pi_ai_types::{ContentBlock, Model};
use pi_agent_core::types::{AgentMessage, AgentTool, AgentToolResult};

fn dummy_model() -> Model {
    Model {
        id: "mock".into(),
        name: "mock".into(),
        api: "openai-completions".into(),
        provider: "mock".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: Default::default(),
        context_window: 8192,
        max_tokens: 2048,
        headers: None,
        compat: None,
    }
}

fn dummy_llm() -> LlmClient {
    LlmClient {
        model: dummy_model(),
        api_key: "sk-dummy".into(),
    }
}

fn echo_tool() -> Arc<AgentTool<serde_json::Value, serde_json::Value>> {
    Arc::new(AgentTool {
        name: "echo".into(),
        description: "Returns its argument verbatim.".into(),
        label: "echo".into(),
        parameters_schema: serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        }),
        execution_mode: None,
        prepare_arguments: None,
        execute: Arc::new(|_id, args, _signal, _on_update| {
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::Text {
                        text,
                        text_signature: None,
                    }],
                    details: serde_json::Value::Null,
                    terminate: None,
                })
            })
        }),
    })
}

#[test]
fn create_agent_uses_pi_agent_core_types() {
    let llm = dummy_llm();
    let tools = vec![echo_tool()];

    let agent = create_agent(&llm, tools, "You are helpful.");

    let state = futures::executor::block_on(agent.state());
    assert_eq!(state.system_prompt, "You are helpful.");
    assert_eq!(state.model.id, "mock");
    assert_eq!(state.tools.len(), 1);
    assert_eq!(state.tools[0].name, "echo");
    assert_eq!(state.messages.len(), 0);
    assert_eq!(state.thinking_level, "off");
}

#[test]
fn agent_message_user_constructs_via_pi_agent_core() {
    use pi_agent_core::pi_ai_types::ContentBlock;
    let msg = AgentMessage::User {
        content: vec![ContentBlock::text("hello")],
        timestamp: 0,
    };
    assert_eq!(msg.role(), "user");
}
