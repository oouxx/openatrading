use async_trait::async_trait;

use super::*;

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

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let (provider, model) = match &ctx.app_config {
            Some(cfg) => (cfg.llm.provider.as_deref(), cfg.llm.model.as_deref()),
            None => (None, None),
        };

        let client = openatrading_provider::llm::LlmClient::from_config(provider, model);
        let start = std::time::Instant::now();
        match client.chat("Respond with exactly one word: ok").await {
            Ok(_) => {
                let elapsed = start.elapsed();
                let ms = elapsed.as_millis();
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: format!("响应 {ms}ms"),
                    details: vec![],
                    duration_ms: ms as u64,
                }
            },
            Err(e) => CheckResult {
                status: CheckStatus::Error("请求失败".into()),
                summary: "LLM API 调用失败".into(),
                details: vec![format!("{e:#}")],
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请检查 LLM_API_KEY 环境变量或 [llm] 配置段".into(),
        })
    }
}
