use async_trait::async_trait;

use super::*;

pub struct PortCheck;

/// Try to bind to the given address to check if the port is free.
fn check_port(addr: &str) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(addr);
    match listener {
        Ok(l) => {
            // Dropping the listener releases the bind immediately
            drop(l);
            Ok(())
        },
        Err(e) => Err(format!("地址 {addr} 被占用: {e}")),
    }
}

#[async_trait]
impl Check for PortCheck {
    fn id(&self) -> &'static str {
        "port"
    }
    fn name(&self) -> &'static str {
        "端口"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Dangerous
    }
    fn fix_label(&self) -> Option<String> {
        Some("提示占用进程".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut details = Vec::new();
        let mut errors = 0usize;

        // Check web_bind port
        let web_bind = ctx
            .app_config
            .as_ref()
            .map(|c| c.service.web_bind.clone())
            .unwrap_or_else(|| "0.0.0.0:8080".to_string());

        match check_port(&web_bind) {
            Ok(()) => details.push(format!("web: {}  ✓", web_bind)),
            Err(e) => {
                details.push(format!("web: {}  ✗ ({})", web_bind, e));
                errors += 1;
            },
        }

        // Check mcp_server bind port
        let mcp_bind = ctx
            .app_config
            .as_ref()
            .map(|c| c.mcp_server.bind.clone())
            .unwrap_or_else(|| "0.0.0.0:8082".to_string());

        match check_port(&mcp_bind) {
            Ok(()) => details.push(format!("mcp: {}  ✓", mcp_bind)),
            Err(e) => {
                details.push(format!("mcp: {}  ✗ ({})", mcp_bind, e));
                errors += 1;
            },
        }

        if errors == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: "端口均可用".into(),
                details,
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Warn(format!("{errors} 个端口被占用")),
                summary: "部分端口被占用".into(),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请手动停止占用端口的进程，或在配置中修改 bind 地址".into(),
        })
    }
}
