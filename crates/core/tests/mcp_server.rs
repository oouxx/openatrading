use openatrading_core::llm::build_market_tools;

#[test]
fn build_market_tools_returns_twenty() {
    let tools = build_market_tools();
    assert_eq!(tools.len(), 20, "expected 20 public market tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "get_limit_up",
        "get_market_overview",
        "get_kline",
        "get_telegraph",
        "get_field_items",
        "get_lhb_detail",
    ] {
        assert!(names.contains(&required), "missing tool: {required}");
    }
}

#[test]
fn build_tool_descriptors_returns_all_twenty() {
    use openatrading_core::llm::MarketToolsServer;

    let server = MarketToolsServer::new(build_market_tools());
    let descriptors = server.build_tool_descriptors();
    assert_eq!(descriptors.len(), 20);
    for tool in &descriptors {
        let schema_type = tool
            .input_schema
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(schema_type, "object", "{} schema not object", tool.name);
        assert!(tool.description.is_some());
        assert!(!tool.description.as_deref().unwrap_or("").is_empty());
    }
}

#[tokio::test]
async fn call_tool_routes_to_dyn_tool_and_serializes_text() {
    use openatrading_agent::pi_ai_types::ContentBlock;
    use openatrading_agent::types::{AgentTool, AgentToolResult};
    use openatrading_core::llm::MarketToolsServer;
    use std::sync::Arc;

    let echo: Arc<openatrading_agent::types::DynTool> = Arc::new(AgentTool {
        name: "echo".into(),
        description: "echo back".into(),
        label: "echo".into(),
        parameters_schema: serde_json::json!({"type":"object","properties":{},"required":[]}),
        execution_mode: None,
        prepare_arguments: None,
        execute: Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text("hello-mcp")],
                    details: serde_json::Value::Null,
                    terminate: None,
                })
            })
        }),
    });

    let server = MarketToolsServer::new(vec![echo]);
    let result = server
        .call_tool_inherent("echo", serde_json::json!({}))
        .await
        .expect("echo failed");
    assert_eq!(result.content.len(), 1);
    let raw_text = result.content[0].clone();
    // Content is Annotated<RawContent>; RawContent::as_text returns Some(&RawTextContent)
    let text_content = raw_text.as_text().expect("expected text content");
    assert_eq!(text_content.text, "hello-mcp");
}

#[tokio::test]
async fn call_tool_unknown_returns_method_not_found() {
    use openatrading_core::llm::MarketToolsServer;

    let server = MarketToolsServer::new(build_market_tools());
    let err = server
        .call_tool_inherent("does_not_exist", serde_json::json!({}))
        .await
        .expect_err("expected error");
    assert_eq!(err.code.0, -32601);
}

#[tokio::test]
async fn call_tool_propagates_business_error_as_internal() {
    use openatrading_agent::types::AgentTool;
    use openatrading_core::llm::MarketToolsServer;
    use std::sync::Arc;

    let bad: Arc<openatrading_agent::types::DynTool> = Arc::new(AgentTool {
        name: "bad".into(),
        description: "always fails".into(),
        label: "bad".into(),
        parameters_schema: serde_json::json!({"type":"object","properties":{},"required":[]}),
        execution_mode: None,
        prepare_arguments: None,
        execute: Arc::new(|_id, _args, _signal, _progress| {
            Box::pin(async move { Err("boom".into()) })
        }),
    });
    let server = MarketToolsServer::new(vec![bad]);
    let err = server
        .call_tool_inherent("bad", serde_json::json!({}))
        .await
        .expect_err("expected error");
    assert_eq!(err.code.0, -32603);
    assert!(err.message.contains("boom"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_end_to_end_list_tools() {
    use openatrading_core::llm::McpClient;
    use std::net::SocketAddr;
    use tokio::sync::watch;

    let bind: SocketAddr = "127.0.0.1:18923".parse().unwrap();
    let (tx, rx) = watch::channel(false);

    let tools = openatrading_core::llm::build_market_tools();
    let handle =
        tokio::spawn(async move { openatrading_core::llm::serve_http(tools, bind, rx).await });

    // 给 server 200ms 起来
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let client = McpClient::connect(format!("http://{bind}/mcp"))
        .await
        .expect("connect");
    let tools_json = client.list_tools().await.expect("list");
    let arr = tools_json.as_array().expect("tools array");
    assert_eq!(arr.len(), 20);

    // 关闭
    let _ = tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), handle).await;
}
