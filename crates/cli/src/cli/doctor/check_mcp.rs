use async_trait::async_trait;
use std::time::Duration;

use super::{Check, CheckContext, CheckResult, CheckStatus, FixResult, FixRisk, FixStatus, Result};
use openatrading_core::llm::{McpConfigStore, McpConnection, McpTransportSpec};

/// Network timeout for a single MCP server connect handshake.
///
/// `McpConnection::connect_spec` has no timeout of its own (the 10s bound lives in
/// `connect_specs`), so the check wraps it. A healthy MCP server completes the
/// initialize handshake quickly; 10s is generous for a slow stdio cold start while
/// still failing fast on a hung endpoint.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);

pub struct McpCheck;

#[async_trait]
impl Check for McpCheck {
    fn id(&self) -> &'static str {
        "mcp"
    }
    fn name(&self) -> &'static str {
        "MCP 服务"
    }
    fn has_fix(&self) -> bool {
        false
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Impossible
    }
    fn fix_label(&self) -> Option<String> {
        None
    }

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        let Ok(store) = McpConfigStore::from_process() else {
            return CheckResult {
                status: CheckStatus::Error("无法定位 mcp.json".into()),
                summary: "配置加载失败".into(),
                details: vec![],
                duration_ms: 0,
            };
        };

        let (servers, load_errors) = store.loaded();
        let mut details: Vec<String> = load_errors
            .iter()
            .map(|e| format!("  ⤚ 配置项已跳过: {e}"))
            .collect();

        if servers.is_empty() {
            return CheckResult {
                status: CheckStatus::Warn("无 MCP 服务".into()),
                summary: format!("未配置任何 MCP 服务器 ({})", store.global_path().display()),
                details,
                duration_ms: 0,
            };
        }

        let enabled = servers.iter().filter(|s| s.enabled).count();
        let disabled = servers.len() - enabled;
        let mut failures = 0usize;

        for server in &servers {
            let label = if server.enabled {
                format!("  {}  ", server.name)
            } else {
                format!("  {} (已禁用)", server.name)
            };

            if !server.enabled {
                details.push(format!("{label} ⤚ 跳过（已禁用）"));
                continue;
            }

            let transport_desc = describe_transport(&server.transport);
            // 只做连接握手 + 工具枚举：pi-rs 没有 verify()，而调用任意一个远端工具
            // 可能带副作用（写文件、下单……），不适合作为健康检查。
            match tokio::time::timeout(NETWORK_TIMEOUT, McpConnection::connect_spec(&server.spec()))
                .await
            {
                Ok(Ok(connection)) => {
                    details.push(format!(
                        "{label} {transport_desc}  ✓ ({} tools)",
                        connection.tool_count()
                    ));
                }
                Ok(Err(e)) => {
                    details.push(format!("{label} {transport_desc}  ✗ ({e})"));
                    failures += 1;
                }
                Err(_) => {
                    details.push(format!(
                        "{label} {transport_desc}  ✗ (连接超时 (>{NETWORK_TIMEOUT:?}))"
                    ));
                    failures += 1;
                }
            }
        }

        if failures == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{} 个 ({enabled} 启用, {disabled} 禁用)", servers.len()),
                details,
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Error(format!("{failures} 个连接失败")),
                summary: format!("{} 个 ({enabled} 启用, {disabled} 禁用)", servers.len()),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        // `has_fix()` 返回 false，正常流程不会到达这里；返回 Skipped 而非
        // panic，与 PortCheck/TelegramCheck/LlmCheck 等无破坏性修复项一致，
        // 防御性地给出提示。
        let message = match McpConfigStore::from_process() {
            Ok(store) => format!(
                "请在 {} 或 {} 中检查 MCP 服务器配置",
                store.global_path().display(),
                store.project_path().display()
            ),
            Err(_) => "请在 pi-rs 的 mcp.json 中检查 MCP 服务器配置".to_string(),
        };
        Ok(FixResult {
            status: FixStatus::Skipped,
            message,
        })
    }
}

fn describe_transport(transport: &McpTransportSpec) -> String {
    match transport {
        McpTransportSpec::Stdio { command, args, .. } => {
            let mut parts = vec![format!("stdio → {command}")];
            if !args.is_empty() {
                parts.push(args.join(" "));
            }
            parts.join(" ")
        }
        McpTransportSpec::Http { url, .. } => format!("http → {url}"),
        McpTransportSpec::Sse { url, .. } => format!("sse → {url}"),
    }
}
