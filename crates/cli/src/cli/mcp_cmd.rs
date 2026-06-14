use anyhow::Result;

use crate::cli::app::McpTransport;

pub async fn cmd_mcp_serve(transport: McpTransport, bind: &str) -> Result<()> {
    use openatrading_core::llm::{build_market_tools, serve_http, serve_stdio};
    use tokio::sync::watch;

    let tools = build_market_tools();
    let (tx, rx) = watch::channel(false);

    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = tx.send(true);
        }
    });

    let result = match transport {
        McpTransport::Http => {
            let bind: std::net::SocketAddr = bind
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid bind {bind}: {e}"))?;
            tracing::info!("[mcp] http server starting on http://{bind}/mcp");
            serve_http(tools, bind, rx).await
        },
        McpTransport::Stdio => {
            tracing::info!("[mcp] stdio server starting");
            serve_stdio(tools, rx).await
        },
    };

    signal_task.abort();
    result
}

pub async fn cmd_mcp_list(endpoint: &str) -> Result<()> {
    let client = openatrading_core::llm::McpClient::connect(endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let tools = client
        .list_tools()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let arr = tools
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("返回格式异常"))?;

    if arr.is_empty() {
        eprintln!("MCP 服务端无注册工具");
        return Ok(());
    }

    println!("MCP 端点: {}\n已注册工具 ({}):\n", endpoint, arr.len());
    for (i, tool) in arr.iter().enumerate() {
        let name = tool["name"].as_str().unwrap_or("?");
        let desc = tool["description"].as_str().unwrap_or("");
        let input_schema = &tool["inputSchema"];
        let props = input_schema["properties"].as_object();
        let prop_count = props.map(|p| p.len()).unwrap_or(0);

        println!("  {}. {:<30} {}", i + 1, name, desc);
        if prop_count > 0 {
            let required = input_schema["required"]
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("     参数({}) {}", prop_count, required);
        }
        println!();
    }

    Ok(())
}

pub async fn cmd_mcp_status(endpoint: &str) -> Result<()> {
    let client = openatrading_core::llm::McpClient::connect(endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    match client.list_tools().await {
        Ok(tools) => {
            let count = tools.as_array().map(|a| a.len()).unwrap_or(0);
            println!("✅ MCP 服务器可达 ({})", endpoint);
            println!("   已注册工具: {count}");
        },
        Err(e) => {
            anyhow::bail!("❌ MCP 服务器不可达 ({}): {e}", endpoint);
        },
    }
    Ok(())
}

pub async fn cmd_mcp_call(endpoint: &str, tool_name: &str, arguments: &str) -> Result<()> {
    let client = openatrading_core::llm::McpClient::connect(endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let args: serde_json::Value = if arguments.is_empty() || arguments == "{}" {
        serde_json::json!({})
    } else {
        serde_json::from_str::<serde_json::Value>(arguments)
            .map_err(|e| anyhow::anyhow!("参数解析失败: {e} (应为 JSON 对象)"))?
    };

    let result = client
        .call(tool_name, args)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
