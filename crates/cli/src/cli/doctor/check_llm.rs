use async_trait::async_trait;
use std::{time::Duration, todo};

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

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        todo!("实现一个简易的llm请求")
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请检查 LLM_API_KEY 环境变量或 [llm] 配置段".into(),
        })
    }
}
