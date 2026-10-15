use anyhow::Result;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cli::app::McpTransport;

pub async fn cmd_mcp_serve(transport: McpTransport, bind: &str) -> Result<()> {
    use openatrading_core::llm::{create_registry, serve_http, serve_stdio};
    use tokio::sync::watch;

    let registry = Arc::new(create_registry());
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
            openatrading_core::log::info!("[mcp] http server starting on http://{bind}/mcp");
            serve_http(registry, bind, rx).await
        }
        McpTransport::Stdio => {
            openatrading_core::log::info!("[mcp] stdio server starting");
            serve_stdio(registry, rx).await
        }
    };

    signal_task.abort();
    result
}

/// 连接一个 MCP 端点（`--endpoint`，streamable HTTP）。
///
/// 客户端实现来自 pi-rs SDK；本命令只做参数拼装与结果呈现。
async fn connect_endpoint(endpoint: &str) -> Result<openatrading_core::llm::McpConnection> {
    use openatrading_core::llm::{McpConnection, McpServerSpec, McpTransportSpec};

    let spec = McpServerSpec {
        name: "endpoint".to_string(),
        transport: McpTransportSpec::Http {
            url: endpoint.to_string(),
            headers: BTreeMap::new(),
        },
    };
    McpConnection::connect_spec(&spec)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub async fn cmd_mcp_list(endpoint: &str) -> Result<()> {
    let connection = connect_endpoint(endpoint).await?;
    let tools = connection.tool_definitions();

    if tools.is_empty() {
        eprintln!("MCP 服务端无注册工具");
        return Ok(());
    }

    println!("MCP 端点: {}\n已注册工具 ({}):\n", endpoint, tools.len());
    for (i, tool) in tools.iter().enumerate() {
        let schema = tool.parameters.as_ref();
        let prop_count = schema
            .and_then(|s| s.get("properties"))
            .and_then(serde_json::Value::as_object)
            .map_or(0, serde_json::Map::len);

        println!("  {}. {:<30} {}", i + 1, tool.name, tool.description);
        if prop_count > 0 {
            let required = schema
                .and_then(|s| s.get("required"))
                .and_then(serde_json::Value::as_array)
                .map(|r| {
                    r.iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("     参数({prop_count}) {required}");
        }
        println!();
    }

    Ok(())
}

pub async fn cmd_mcp_status(endpoint: &str) -> Result<()> {
    let connection = connect_endpoint(endpoint).await?;
    println!("✅ MCP 服务器可达 ({endpoint})");
    println!("   已注册工具: {}", connection.tool_count());
    Ok(())
}

pub async fn cmd_mcp_call(endpoint: &str, tool_name: &str, arguments: &str) -> Result<()> {
    let connection = connect_endpoint(endpoint).await?;

    let args: serde_json::Value = if arguments.is_empty() || arguments == "{}" {
        serde_json::json!({})
    } else {
        serde_json::from_str::<serde_json::Value>(arguments)
            .map_err(|e| anyhow::anyhow!("参数解析失败: {e} (应为 JSON 对象)"))?
    };

    let tools = connection.tool_definitions();
    let Some(tool) = tools.iter().find(|t| t.name == tool_name) else {
        let available: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        anyhow::bail!(
            "服务端没有工具 {tool_name}（可用: {}）",
            if available.is_empty() {
                "无".to_string()
            } else {
                available.join(", ")
            }
        );
    };
    let Some(execute) = tool.execute.clone() else {
        anyhow::bail!("工具 {tool_name} 没有可执行实现");
    };

    let output = execute(String::new(), args, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("{}", render_output(&output.content));
    if output.is_error {
        anyhow::bail!("工具 {tool_name} 返回错误");
    }
    Ok(())
}

/// 呈现工具结果。
///
/// pi-rs 的结果是 `ContentBlock` 形状的 JSON 数组；只有一个 text 块时直接打印其
/// 文本（服务端通常返回 JSON 文本），否则打印原始结构。
fn render_output(content: &[serde_json::Value]) -> String {
    if let [only] = content {
        if let Some(text) = only.get("text").and_then(serde_json::Value::as_str) {
            return text.to_string();
        }
    }
    serde_json::to_string_pretty(content).unwrap_or_default()
}
