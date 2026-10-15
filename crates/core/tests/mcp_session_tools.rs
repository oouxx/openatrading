//! 端到端：`mcp.json` 里配置的外部 server，其工具必须真的进入会话的工具列表。
//!
//! 这是本次「MCP 客户端改用 pi-rs SDK」的核心不变量。链路为
//! `create_factory` → `create_agent_session_from_services` → pi-rs
//! `sdk::create_agent_session`，后者读 `{agent_dir}/mcp.json` 并连接。
//!
//! 被测 server 就是本项目自己的暴露服务（`serve_http`），因此测试完全自洽：不依赖
//! 外部进程、网络或 API Key。会话用 `FactoryOptions::market_only`（不注册交易扩展工具），
//! 因此 `get_limit_up` 只可能来自 MCP —— 断言是尖锐的。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use openatrading_core::llm::{
    create_factory, create_registry, serve_http, CreateSessionParams, FactoryOptions,
};
use pi_coding_agent::core::session_manager::SessionManager;

/// 取一个当前空闲的端口：绑定 0 端口让内核分配后立即释放。
///
/// 释放到被测服务重新绑定之间存在理论上的竞争窗口，测试场景下可接受。
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

/// 等到端口可连接（或超时放弃，让后续断言给出更有意义的失败）。
async fn wait_until_listening(addr: SocketAddr) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_servers_from_mcp_json_reach_the_session() {
    // ── 1. 起一个真的 MCP server（本项目自己的行情工具暴露服务）──
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let registry = Arc::new(create_registry());
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        let _ = serve_http(registry, addr, shutdown_rx).await;
    });
    wait_until_listening(addr).await;

    // ── 2. 临时 PI_RS_HOME：全局 mcp.json 指向该 server ──
    let tmp = tempfile::tempdir().expect("tempdir");
    let agent_dir = tmp.path().join("agent");
    std::fs::create_dir_all(&agent_dir).expect("create agent dir");
    std::fs::write(
        agent_dir.join("mcp.json"),
        serde_json::json!({
            "mcpServers": {
                "e2e": { "url": format!("http://127.0.0.1:{port}/mcp") }
            }
        })
        .to_string(),
    )
    .expect("write mcp.json");
    std::env::set_var("PI_RS_HOME", tmp.path());

    let cwd = tmp.path().join("proj");
    std::fs::create_dir_all(&cwd).expect("create project dir");
    std::env::set_current_dir(&cwd).expect("set cwd");

    // ── 3. 建会话，断言 MCP 工具进了工具列表 ──
    let factory = create_factory(FactoryOptions::new(agent_dir.to_string_lossy().to_string()));
    let created = factory(CreateSessionParams {
        cwd: cwd.to_string_lossy().to_string(),
        agent_dir: agent_dir.to_string_lossy().to_string(),
        exclude_tools: None,
        session_manager: SessionManager::new(
            &cwd.to_string_lossy(),
            &pi_coding_agent::config::get_sessions_dir().to_string_lossy(),
            None,
            true,
            Some(pi_coding_agent::core::session_manager::NewSessionOptions {
                id: Some("mcp-e2e".to_string()),
                parent_session: None,
            }),
        ),
        session_start_event: None,
    })
    .await
    .expect("创建 agent session 失败");

    let tool_names: Vec<String> = created
        .session
        .get_state()
        .await
        .tools
        .iter()
        .map(|t| t.name.clone())
        .collect();

    // market_only 不注册交易扩展工具，故这 28 个行情工具只能来自 MCP。
    assert!(
        tool_names.iter().any(|n| n == "get_limit_up"),
        "mcp.json 里配置的 MCP 工具没有进入会话: {tool_names:?}"
    );

    let _ = shutdown_tx.send(true);
    server.abort();
}
