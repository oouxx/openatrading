use async_trait::async_trait;

use super::{Check, CheckContext, CheckResult, CheckStatus, FixResult, FixRisk, FixStatus, Result};

/// Network timeout for a single LLM probe request.
///
pub struct LlmCheck;

#[async_trait]
impl Check for LlmCheck {
    fn id(&self) -> &'static str {
        "llm"
    }
    fn name(&self) -> &'static str {
        "LLM API"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("提示检查配置".into())
    }

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        // TODO: 实现一个简易的llm请求
        CheckResult {
            status: CheckStatus::Warn("未实现".into()),
            summary: "LLM 连通性检查尚未实现".into(),
            details: vec![],
            duration_ms: 0,
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: format!(
                "请在 {}/settings.json 配置 defaultProvider/defaultModel，并在同目录的 auth.json 设置 API Key",
                openatrading_core::init::agent_dir().display()
            ),
        })
    }
}
