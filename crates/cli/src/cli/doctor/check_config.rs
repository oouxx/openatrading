use async_trait::async_trait;

use super::{Check, CheckContext, CheckResult, CheckStatus, FixResult, FixRisk, FixStatus, Result};

pub struct ConfigCheck;

#[async_trait]
impl Check for ConfigCheck {
    fn id(&self) -> &'static str {
        "config"
    }
    fn name(&self) -> &'static str {
        "配置文件"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("生成默认配置".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        ctx.app_config.as_ref().map_or_else(
            || CheckResult {
                status: CheckStatus::Error("无法加载".into()),
                summary: "config.toml 加载失败".into(),
                details: vec!["请检查 ~/.atrading/config.toml 是否存在且格式正确".into()],
                duration_ms: 0,
            },
            |cfg| {
                let roles_count = cfg.roles.len();
                // MCP 服务器不再来自 config.toml：配置在 pi-rs 的 mcp.json 里
                // （见 check_mcp）。
                let mcp_count = openatrading_core::llm::McpConfigStore::from_process()
                    .map_or(0, |store| store.loaded().0.len());
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: format!("{roles_count} 个角色, {mcp_count} 个 MCP 服务"),
                    details: vec![],
                    duration_ms: 0,
                }
            },
        )
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let config_path = ctx.atrading_home.join("config.toml");
        if config_path.exists() {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: "配置文件已存在".into(),
            });
        }
        std::fs::write(
            &config_path,
            include_str!("../../../../../config.example.toml"),
        )?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已生成 {}", config_path.display()),
        })
    }
}
