//! End-to-end integration test: stream chat that calls a real tool.
//!
//! This test requires a live LLM API. It is annotated `#[ignore]` so it
//! does not run by default. To run it:
//!
//! ```sh
//! export LLM_API_KEY=sk-...
//! export LLM_PROVIDER=openrouter    # or deepseek
//! export LLM_MODEL=openai/gpt-4o    # or deepseek-chat
//! cargo test -p openatrading-core --test agent_stream_e2e -- --ignored --nocapture
//! ```
//!
//! What the test does:
//! 1. Builds an `LlmClient` from env.
//! 2. Wraps the real `get_limit_up` tool in a `Vec<Arc<DynTool>>`.
//! 3. Sends a user message asking the model to call `get_limit_up`.
//! 4. Streams events from `agent_loop_stream`, prints each one.
//! 5. Asserts at least one `tool_call` event with `name == "get_limit_up"`
//!    and a matching `tool_result` event with non-empty result.

use std::sync::Arc;

use futures::StreamExt;
use openatrading_core::llm::agent_loop_stream;
use openatrading_core::llm::tools::get_limit_up;
use openatrading_core::llm::DynTool;
use openatrading_core::llm::LlmClient;

fn api_key_available() -> bool {
    std::env::var("LLM_API_KEY").is_ok()
        || std::env::var("DEEPSEEK_API_KEY").is_ok()
        || std::env::var("OPENROUTER_API_KEY").is_ok()
}

#[tokio::test]
#[ignore = "requires live LLM API; run with --ignored"]
async fn stream_chat_invokes_get_limit_up() {
    if !api_key_available() {
        eprintln!(
            "[skip] no LLM_API_KEY / DEEPSEEK_API_KEY / OPENROUTER_API_KEY in env; \
             set one to run this test"
        );
        return;
    }

    let llm = LlmClient::from_env();
    let tools: Vec<Arc<DynTool>> = vec![get_limit_up()];

    let system_prompt = "你是一个A股短线交易AI助手，擅长分析涨停板行情。\
                        当用户问涨停板时，调用 get_limit_up 工具获取数据并基于结果回答。\
                        不要编造数据。".to_string();

    let user_message = "请用 get_limit_up 工具查询今天的涨停股票，并简要总结前 5 只。".to_string();

    eprintln!(
        "[test] starting stream: provider={}, model={}",
        llm.model.provider, llm.model.id
    );

    let mut stream = agent_loop_stream(llm, tools, system_prompt, vec![], user_message)
        .await
        .expect("agent_loop_stream setup failed");

    let mut saw_tool_call = false;
    let mut saw_tool_result = false;
    let mut saw_done = false;
    let mut last_text = String::new();

    while let Some(chunk) = stream.next().await {
        let raw = match chunk {
            Ok(s) => s,
            Err(e) => panic!("stream error: {e}"),
        };

        let event: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[test] non-JSON chunk: {raw} ({e})");
                continue;
            }
        };

        let kind = event["type"].as_str().unwrap_or("?");
        match kind {
            "text" => {
                if let Some(t) = event["content"].as_str() {
                    eprint!("{t}");
                    last_text.push_str(t);
                }
            }
            "tool_call" => {
                let name = event["name"].as_str().unwrap_or("");
                let args = event["args"].as_str().unwrap_or("");
                eprintln!("\n[test] tool_call name={name} args={args}");
                assert_eq!(name, "get_limit_up", "unexpected tool called: {name}");
                saw_tool_call = true;
            }
            "tool_result" => {
                let name = event["name"].as_str().unwrap_or("");
                let result = event["result"].as_str().unwrap_or("");
                eprintln!(
                    "[test] tool_result name={} len={} preview={}",
                    name,
                    result.len(),
                    &result.chars().take(200).collect::<String>()
                );
                if name == "get_limit_up" {
                    assert!(!result.is_empty(), "get_limit_up returned empty result");
                    assert!(
                        result.len() > 20,
                        "get_limit_up result too short to be real data: {result}"
                    );
                    saw_tool_result = true;
                }
            }
            "error" => {
                let msg = event["message"].as_str().unwrap_or("");
                eprintln!("\n[test] error: {msg}");
                panic!("agent stream reported error: {msg}");
            }
            "done" => {
                eprintln!("\n[test] done");
                saw_done = true;
                break;
            }
            other => {
                eprintln!("[test] unhandled event type: {other} -> {event}");
            }
        }
    }

    eprintln!("\n[test] final text length: {} chars", last_text.len());
    eprintln!("[test] saw_tool_call={saw_tool_call} saw_tool_result={saw_tool_result} saw_done={saw_done}");

    assert!(saw_done, "stream did not emit done event");
    assert!(saw_tool_call, "model never called get_limit_up");
    assert!(saw_tool_result, "model never received a tool_result for get_limit_up");
}
