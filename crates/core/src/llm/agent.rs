use anyhow::Result;
use futures::stream::StreamExt;
use std::pin::Pin;
use std::sync::Arc;

use super::{LlmClient, StreamEvent, ToolSet};

const MAX_ROUNDS: usize = 5;

#[derive(Debug)]
pub struct AgentResult {
    pub final_text: String,
    pub rounds: usize,
}

#[cfg(not(target_arch = "wasm32"))]
pub type AgentStream = Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;

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

        if let Some(ref content) = choice.message.content {
            if !content.is_empty() {
                return Ok(AgentResult {
                    final_text: content.clone(),
                    rounds: round + 1,
                });
            }
        }

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

/// Streaming version of agent_loop: yields text chunks, tool calls, and results progressively.
pub async fn agent_loop_stream(
    llm: LlmClient,
    tools: Arc<ToolSet>,
    system_prompt: String,
    history: Vec<serde_json::Value>,
    user_message: String,
) -> Result<AgentStream> {
    let tool_schemas = tools.to_openai_tools();
    let mut messages: Vec<serde_json::Value> =
        vec![serde_json::json!({"role": "system", "content": system_prompt})];
    // Append conversation history
    messages.extend(history);
    messages.push(serde_json::json!({"role": "user", "content": user_message}));

    let stream = async_stream::stream! {
        'agent: for _round in 0..MAX_ROUNDS {
            let mut stream_text = String::new();
            let mut reasoning_text = String::new();
            // 收集本轮所有 tool calls（OpenAI 支持一轮多个并行 tool call）
            let mut pending_tcs: Vec<super::ToolCall> = Vec::new();
            let mut current_tc: Option<super::ToolCall> = None;

            match llm.chat_stream_with_tools(
                &serde_json::json!(messages),
                &serde_json::json!(tool_schemas),
            ).await {
                Ok(mut s) => {
                    while let Some(chunk) = s.next().await {
                        match chunk {
                            Ok(events) => {
                                for ev in events {
                                    match ev {
                                        StreamEvent::Text(t) => {
                                            stream_text.push_str(&t);
                                            yield Ok(serde_json::json!({
                                                "type": "text",
                                                "content": t,
                                            }).to_string());
                                        }
                                        StreamEvent::ReasoningText(t) => {
                                            reasoning_text.push_str(&t);
                                        }
                                        StreamEvent::ToolCallStart(tc) => {
                                            // 新 tool call 开始前，把上一个归档
                                            if let Some(prev) = current_tc.take() {
                                                pending_tcs.push(prev);
                                            }
                                            current_tc = Some(tc);
                                        }
                                        StreamEvent::ToolCallDelta(tc) => {
                                            if let Some(ref mut p) = current_tc {
                                                p.arguments.push_str(&tc.arguments);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                yield Ok(serde_json::json!({
                                    "type": "error",
                                    "message": e.to_string(),
                                }).to_string());
                            }
                        }
                    }
                    // stream 结束，归档最后一个 tool call
                    if let Some(last) = current_tc.take() {
                        pending_tcs.push(last);
                    }
                }
                Err(e) => {
                    yield Ok(serde_json::json!({
                        "type": "error",
                        "message": e.to_string(),
                    }).to_string());
                    break 'agent;
                }
            }

            // LLM 直接输出文本，说明已经给出自然语言回答，结束
            if !stream_text.is_empty() {
                messages.push({
                    let mut msg = serde_json::json!({
                        "role": "assistant",
                        "content": stream_text,
                    });
                    if !reasoning_text.is_empty() {
                        msg["reasoning_content"] = serde_json::json!(reasoning_text);
                    }
                    msg
                });
                break 'agent;
            }

            // 本轮没有 tool call 也没有文本，异常退出
            if pending_tcs.is_empty() {
                break 'agent;
            }

            // ── 构造 assistant 消息（OpenAI 格式：所有 tool_calls 在同一条）──
            let tc_list: Vec<serde_json::Value> = pending_tcs.iter().map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function_name,
                        "arguments": tc.arguments,
                    }
                })
            }).collect();

            let mut assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": null,          // OpenAI 要求有 tool_calls 时 content 为 null
                "tool_calls": tc_list,
            });
            if !reasoning_text.is_empty() {
                assistant_msg["reasoning_content"] = serde_json::json!(reasoning_text);
            }
            messages.push(assistant_msg);

            // ── 并行执行所有 tool calls，逐个追加 tool result ──
            for tc in &pending_tcs {
                let fn_name = tc.function_name.clone();
                let fn_args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);

                yield Ok(serde_json::json!({
                    "type": "tool_call",
                    "name": fn_name,
                    "args": tc.arguments,
                }).to_string());

                let raw_result = match tools.call(&fn_name, &fn_args).await {
                    Ok(output) => output,
                    Err(e) => format!("Error calling {fn_name}: {e}"),
                };

                yield Ok(serde_json::json!({
                    "type": "tool_result",
                    "name": fn_name,
                    "result": raw_result,
                }).to_string());

                // tool role 只放原始数据，不加自然语言提示（tool role 不当指令处理）
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": raw_result,
                }));
            }

            // 继续下一轮，让 LLM 决定是否还要调工具
            continue 'agent;
        }

        // ── agent loop 结束后检查：最后一条是 tool result，说明还没有最终回答 ──
        let needs_final_answer = messages.last()
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            == Some("tool");

        if needs_final_answer {
            // 加一条 user 指令，驱动 LLM 给出自然语言结论
            messages.push(serde_json::json!({
                "role": "user",
                "content": "所有数据已获取完毕。请综合以上工具返回的信息，用简洁的中文自然语言给出分析和交易建议。禁止输出 JSON、代码块或任何结构化格式。",
            }));

            match llm.chat_stream_with_tools(
                &serde_json::json!(messages),
                &serde_json::json!(serde_json::Value::Array(vec![])), // 最终回答轮不传 tools，避免再次 tool call
            ).await {
                Ok(mut s) => {
                    while let Some(chunk) = s.next().await {
                        match chunk {
                            Ok(events) => {
                                for ev in events {
                                    if let StreamEvent::Text(t) = ev {
                                        yield Ok(serde_json::json!({
                                            "type": "text",
                                            "content": t,
                                        }).to_string());
                                    }
                                }
                            }
                            Err(e) => {
                                yield Ok(serde_json::json!({
                                    "type": "error",
                                    "message": e.to_string(),
                                }).to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Ok(serde_json::json!({
                        "type": "error",
                        "message": e.to_string(),
                    }).to_string());
                }
            }
        }

        yield Ok(serde_json::json!({"type": "done"}).to_string());
    };

    Ok(Box::pin(stream))
}
