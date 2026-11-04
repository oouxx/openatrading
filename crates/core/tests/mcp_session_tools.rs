//! 端到端：`mcp.json` 里配置的外部 server，其工具必须真的进入会话的工具列表。
//!
//! 链路为 `create_factory` → `create_agent_session_from_services` → pi-rs
//! `sdk::create_agent_session`，后者读 `{agent_dir}/mcp.json` 并连接。
//!
//! 被测 server 是本项目自己的暴露服务（`serve_http`），但注册表里只放一个
//! **测试专属**工具 `mcp_e2e_probe`：会话侧本地扩展没有同名工具，因此断言不可能
//! 被本地工具“碰巧”满足。`exposure: "direct"` 让 pi-rs 在首个 prompt 前等待
//! 连接完成，连接失败会让断言直接失败。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use openatrading_core::llm::{
    create_factory, CreateSessionParams, FactoryOptions, McpConfigError, McpConfigStore,
};
use pi_coding_agent::core::session_manager::SessionManager;
use pi_extension_api::{
    create_synthetic_source_info, ExtensionContext, ExtensionRegistry, HookHandler, ToolCallOutput,
    ToolDefinition, ToolInvocation, ToolRegistry,
};

/// 只暴露一个测试专属工具的扩展。
struct ProbeExtension;

#[async_trait::async_trait]
impl HookHandler for ProbeExtension {
    fn name(&self) -> &str {
        "mcp-e2e-probe"
    }

    fn register_tools(&self, tools: &mut ToolRegistry) {
        let definition = ToolDefinition {
            name: "mcp_e2e_probe".to_string(),
            description: "E2E probe tool served over MCP".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })),
            ..ToolDefinition::default()
        };
        tools.register(&definition.name.clone(), definition);
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        _params: serde_json::Value,
        _ctx: &ExtensionContext,
        _invocation: &ToolInvocation,
    ) -> Option<ToolCallOutput> {
        (tool_name == "mcp_e2e_probe").then(|| ToolCallOutput::text("ok"))
    }
}

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
    // ── 1. 起一个只含测试专属工具的 MCP server ──
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut registry = ExtensionRegistry::new();
    registry.register(
        Box::new(ProbeExtension),
        create_synthetic_source_info("/tmp/mcp-e2e".into(), "probe".into(), None, None, None),
    );
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        let _ = openatrading_core::llm::serve_http(Arc::new(registry), addr, shutdown_rx).await;
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
                "e2e": {
                    "url": format!("http://127.0.0.1:{port}/mcp"),
                    // pi-rs 默认 exposure=codemode（deferred，需要 tool_search 才可见）；
                    // 本应用的管理界面默认写 direct，这里复现该配置。
                    "exposure": "direct"
                }
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

    // 该工具只存在于远端 server，本地扩展不可能提供它。pi-rs 会按
    // `mcp__<server>__<tool>` 给 MCP 工具加命名空间。
    assert!(
        tool_names.iter().any(|n| n == "mcp__e2e__mcp_e2e_probe"),
        "mcp.json 里配置的 MCP 工具没有进入会话: {tool_names:?}"
    );

    // ── 4. 管理界面的“测试连接”走同一条 pi-rs 连接路径 ──
    let store = McpConfigStore::new(
        cwd.to_string_lossy().to_string(),
        agent_dir.to_string_lossy().to_string(),
    );
    let tested = store.test("e2e").await.expect("测试连接");
    assert!(tested.ok, "测试连接失败: {:?}", tested.error);
    assert_eq!(tested.tool_count, 1, "应枚举到测试专属工具");
    assert!(matches!(
        store.test("missing").await,
        Err(McpConfigError::NotFound)
    ));

    let _ = shutdown_tx.send(true);
    server.abort();
}
