use anyhow::Result;
use std::sync::Arc;

use super::{LlmClient, ToolSet};

const MAX_ROUNDS: usize = 5;

#[derive(Debug)]
pub struct AgentResult {
    pub final_text: String,
    pub rounds: usize,
}

/// Run a multi-turn conversation where the LLM can call tools.
/// Each round: send messages + tools → if tool_calls, execute and append → repeat.
pub async fn agent_loop(
    llm: &LlmClient,
    tools: &Arc<ToolSet>,
    system_prompt: &str,
    user_message: &str,
) -> Result<AgentResult> {
    let tool_schemas = tools.to_openai_tools();
    let mut messages: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_message}),
    ];

    for round in 0..MAX_ROUNDS {
        let body = serde_json::json!({
            "model": llm.model.clone(),
            "messages": messages.clone(),
            "max_tokens": 4096,
            "temperature": 0.7,
            "tools": tool_schemas.clone(),
        });

        let resp = llm.chat_with_tools(&body.to_string()).await?;

        let choice = match resp.choices.into_iter().next() {
            Some(c) => c,
            None => {
                return Ok(AgentResult {
                    final_text: String::new(),
                    rounds: round + 1,
                });
            }
        };

        // If LLM returned text (no tool calls), we're done
        if let Some(ref content) = choice.message.content {
            if !content.is_empty() {
                return Ok(AgentResult {
                    final_text: content.clone(),
                    rounds: round + 1,
                });
            }
        }

        // If LLM requested tool calls, execute them
        if let Some(ref tc_list) = choice.message.tool_calls {
            messages.push(serde_json::json!({
                "role": "assistant",
                "tool_calls": tc_list,
            }));

            for tc in tc_list {
                let fn_name = tc.function.name.as_str();
                let fn_args: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);

                let result = match tools.call(fn_name, &fn_args).await {
                    Ok(output) => output,
                    Err(e) => format!("Error: {e}"),
                };

                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result,
                }));
            }
            continue;
        }

        // Neither text nor tool calls — shouldn't happen
        return Ok(AgentResult {
            final_text: String::new(),
            rounds: round + 1,
        });
    }

    Ok(AgentResult {
        final_text: "已达到最大轮次，分析未完成。".to_string(),
        rounds: MAX_ROUNDS,
    })
}