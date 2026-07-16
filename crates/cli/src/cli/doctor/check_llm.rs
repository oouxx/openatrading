use async_trait::async_trait;
use std::time::Duration;

use super::*;

/// Network timeout for a single LLM probe request.
///
/// LLM completions can legitimately take a while; 30s leaves headroom for a
/// slow provider while still bounding a hung endpoint so `doctor` doesn't
/// stall indefinitely.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);

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

        let client = openatrading_core::llm::LlmClient::from_config(provider, model);
        let start = std::time::Instant::now();
        match tokio::time::timeout(
            NETWORK_TIMEOUT,
            client.chat("Respond with exactly one word: ok"),
        )
        .await
        {
            Ok(Ok(_)) => {
                let elapsed = start.elapsed();
                let ms = elapsed.as_millis();
                CheckResult {
                    status: CheckStatus::Ok,
                    // `start` 在 `chat()` 之前采样，故耗时含 TLS 握手与连接建立，
                    // 用"往返"而非"响应"以如实反映测量内容。
                    summary: format!("往返 {ms}ms"),
                    details: vec![],
                    duration_ms: ms as u64,
                }
            },
            Ok(Err(e)) => CheckResult {
                status: CheckStatus::Error("请求失败".into()),
                summary: "LLM API 调用失败".into(),
                details: vec![format!("{e:#}")],
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(_elapsed) => CheckResult {
                status: CheckStatus::Error("请求超时".into()),
                summary: format!("LLM API 调用超时 (>{NETWORK_TIMEOUT:?})"),
                details: vec![],
                duration_ms: NETWORK_TIMEOUT.as_millis() as u64,
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
