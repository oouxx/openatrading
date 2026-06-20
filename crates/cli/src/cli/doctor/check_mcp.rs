use async_trait::async_trait;
use std::time::Duration;

use super::*;
use openatrading_core::llm::{McpClient, McpTransportConfig};

/// Network timeout for a single MCP server connect handshake.
///
/// A healthy MCP server should complete the initialize handshake quickly;
/// 10s is generous for a slow cold start (e.g. a stdio child) while still
/// failing fast on a hung endpoint.
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

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let servers = match &ctx.app_config {
            Some(cfg) => &cfg.mcp_servers,
            None => {
                return CheckResult {
                    status: CheckStatus::Error("无法加载配置".into()),
                    summary: "配置加载失败".into(),
                    details: vec![],
                    duration_ms: 0,
                }
            },
        };

        if servers.is_empty() {
            return CheckResult {
                status: CheckStatus::Warn("无 MCP 服务".into()),
                summary: "未配置任何 MCP 服务器".into(),
                details: vec![],
                duration_ms: 0,
            };
        }

        let enabled = servers.iter().filter(|s| s.enabled).count();
        let disabled = servers.len() - enabled;
        let mut details = Vec::new();
        let mut failures = 0usize;

        for server in servers {
            let label = if server.enabled {
                format!("  {}  ", server.name)
            } else {
                format!("  {} (已禁用)", server.name)
            };

            if !server.enabled {
                details.push(format!("{} ⤚ 跳过（已禁用）", label));
                continue;
            }

            let transport_desc = match &server.transport {
                McpTransportConfig::StreamableHttp { url, .. } => {
                    format!("streamable-http → {url}")
                },
                McpTransportConfig::Sse { url, .. } => {
                    format!("sse → {url}")
                },
                McpTransportConfig::Stdio { command, args, .. } => {
                    format!("stdio → {command} {}", args.join(" "))
                },
            };

            let connect_result = match &server.transport {
                McpTransportConfig::StreamableHttp { url, .. } => {
                    tokio::time::timeout(NETWORK_TIMEOUT, McpClient::connect(url.clone()))
                        .await
                        .map_err(|_| format!("连接超时 (>{NETWORK_TIMEOUT:?})"))
                },
                McpTransportConfig::Sse { url, .. } => {
                    tokio::time::timeout(NETWORK_TIMEOUT, McpClient::connect(url.clone()))
                        .await
                        .map_err(|_| format!("连接超时 (>{NETWORK_TIMEOUT:?})"))
                },
                McpTransportConfig::Stdio { command, args, env } => tokio::time::timeout(
                    NETWORK_TIMEOUT,
                    McpClient::connect_stdio(command, args, env),
                )
                .await
                .map_err(|_| format!("连接超时 (>{NETWORK_TIMEOUT:?})")),
            };

            match connect_result {
                Ok(_client) => {
                    details.push(format!("{} {}  ✓", label, transport_desc));
                },
                Err(e) => {
                    details.push(format!("{} {}  ✗ ({})", label, transport_desc, e));
                    failures += 1;
                },
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
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请在 config.toml [[mcp_servers]] 中检查 MCP 服务器配置".into(),
        })
    }
}
