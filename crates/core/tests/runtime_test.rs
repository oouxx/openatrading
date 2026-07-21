// ============================================================================
// Tests
// ============================================================================

use openatrading_core::llm::AgentRuntime;
use openatrading_core::platform::cli::{NoopPersister, StdoutEmitter};
use pi_coding_agent::pi_agent_core::agent::PromptInput;
use pi_coding_agent::pi_agent_core::types::AgentMessage;
use pi_coding_agent::pi_ai_types::ContentBlock;
use std::sync::Arc;

#[tokio::test]
async fn test_create_agent_runtime_with_extension() {
    // let ext_registry = create_registry();

    let runtime = AgentRuntime::new(Arc::new(StdoutEmitter::new(false)), Arc::new(NoopPersister));
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");
    // Find and invoke the `create_goal` extension tool through the active tool list.
    let agent_state = agent_facade.session.get_agent().state().await;
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
            if let ContentBlock::Text { text, .. } = c {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    println!("[dbg] create_goal result: {}", goal_text);
}

#[tokio::test]
async fn test_trading_tools_exec() {
    let runtime = AgentRuntime::new(Arc::new(StdoutEmitter::new(false)), Arc::new(NoopPersister));
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");
    // Find and invoke the `create_goal` extension tool through the active tool list.
    let agent_state = agent_facade.session.get_agent().state().await;
    let get_limit_up = agent_state
        .tools
        .iter()
        .find(|t| t.name == "get_limit_up")
        .expect("create_goal tool should be active");

    let params = serde_json::json!({});
    let result = (get_limit_up.execute)("call-get-limit-up-1".to_string(), params, None, None)
        .await
        .expect("get_limit_up execute failed");
    let goal_text = result
        .content
        .iter()
        .filter_map(|c| {
            if let ContentBlock::Text { text, .. } = c {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    println!("[dbg] get limit up result: {}", goal_text);
}

/// Ask the LLM what tools it has access to and assert the reply mentions the
/// built-in `bash` tool. Requires `OPENROUTER_API_KEY` and network access.
#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY and network"]
async fn test_bash_tool_exec() {
    let runtime = AgentRuntime::new(Arc::new(StdoutEmitter::new(false)), Arc::new(NoopPersister));
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");

    let messages = agent_facade
        .session
        .get_agent()
        .prompt(PromptInput::Text("执行ls命令"))
        .await
        .expect("agent prompt failed");

    // Print all messages for debugging
    println!("[dbg] === All messages ({}) ===", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::Assistant { content, .. } => {
                println!("[dbg]   [{i}] Assistant ({} blocks):", content.len());
                for (j, block) in content.iter().enumerate() {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            println!("[dbg]     [{j}] Text: {text}");
                        },
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            println!(
                                "[dbg]     [{j}] ToolCall: id={id} name={name} args={arguments}"
                            );
                        },
                        ContentBlock::Thinking { thinking, .. } => {
                            println!("[dbg]     [{j}] Thinking: {thinking}");
                        },
                        _ => {
                            println!("[dbg]     [{j}] Other: {block:?}");
                        },
                    }
                }
            },
            AgentMessage::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
                ..
            } => {
                let text = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                println!("[dbg]   [{i}] ToolResult: tool={tool_name} id={tool_call_id} is_error={is_error} text={text}");
            },
            AgentMessage::BashExecution {
                command,
                output,
                exit_code,
                ..
            } => {
                println!(
                    "[dbg]   [{i}] BashExecution: cmd={command} exit={exit_code:?} output={output}"
                );
            },
            _ => {
                println!("[dbg]   [{i}] Other: {msg:?}");
            },
        }
    }

    // Verify that the LLM made at least one tool call (bash with ls)
    let has_tool_call = messages.iter().any(|m| match m {
        AgentMessage::Assistant { content, .. } => content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolCall { .. })),
        _ => false,
    });
    assert!(
        has_tool_call,
        "Expected at least one tool call in assistant messages"
    );

    // Verify that at least one tool result came back
    let has_tool_result = messages
        .iter()
        .any(|m| matches!(m, AgentMessage::ToolResult { .. }));
    assert!(has_tool_result, "Expected at least one tool result");
}
