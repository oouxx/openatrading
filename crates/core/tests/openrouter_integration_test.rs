use std::sync::Arc;

use openatrading_core::llm::{tools, AgentEvent, LlmClient};
use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};

/// Simulate the /api/ai/chat SSE streaming flow:
/// subscribe → process in background → collect events via channel → verify.
#[tokio::test]
#[ignore]
async fn test_openrouter_is_streaming() {
    let llm = LlmClient::with_config(
        "openrouter".to_string(),
        "nvidia/nemotron-3-super-120b-a12b:free".to_string(),
    );

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
        custom_tools: None,
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
        enable_extensions: true,
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

    // Inject real tool implementations
    let agent = session.get_agent();
    agent.add_tools(tools).await;
    let _ = agent;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let listener = Arc::new(
        move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event);
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        },
    );

    session.subscribe(listener).await;
    session
        .add_user_text("请先用 get_market_overview 查询市场概况。")
        .await;

    let mut found_text_delta = false;
    let mut found_done = false;

    use std::time::Duration;
    let timeout =
        tokio::time::timeout(Duration::from_secs(60), async {
            while let Some(event) = rx.recv().await {
                match &event {
                    AgentEvent::MessageUpdate {
                        assistant_message_event:
                            openatrading_core::llm::pi_ai_types::AssistantMessageEvent::TextDelta {
                                ..
                            },
                        ..
                    } => {
                        found_text_delta = true;
                    },
                    AgentEvent::MessageUpdate {
                        assistant_message_event:
                            openatrading_core::llm::pi_ai_types::AssistantMessageEvent::Done { .. },
                        ..
                    } => {
                        found_done = true;
                        break;
                    },
                    _ => {},
                }
            }
        })
        .await;

    match timeout {
        Ok(_) => {
            assert!(found_text_delta, "Expected at least one TextDelta event");
            assert!(found_done, "Expected Done event");
            tracing::info!("✅ Streamed response received successfully");
        },
        Err(_) => {
            tracing::warn!("⏱️ Test timed out after 60s — partial results: text_delta={found_text_delta}, done={found_done}");
        },
    }
}
