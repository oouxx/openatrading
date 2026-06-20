use async_trait::async_trait;

use super::*;

pub struct SkillsCheck;

#[async_trait]
impl Check for SkillsCheck {
    fn id(&self) -> &'static str {
        "skills"
    }
    fn name(&self) -> &'static str {
        "技能"
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
        let registry = openatrading_core::skills::SkillRegistry::from_disk();
        let skills = registry.skills();

        if skills.is_empty() {
            return CheckResult {
                status: CheckStatus::Warn("无已安装技能".into()),
                summary: "未安装任何技能".into(),
                details: vec!["可运行 `openatrading skills install` 安装技能".into()],
                duration_ms: 0,
            };
        }

        let details: Vec<String> = skills
            .iter()
            .map(|s| {
                let desc = if s.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", s.description)
                };
                format!("  {}{}", s.name, desc)
            })
            .collect();

        CheckResult {
            status: CheckStatus::Ok,
            summary: format!("{} 个已安装", skills.len()),
            details,
            duration_ms: 0,
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        // `has_fix()` 返回 false，正常流程不会到达这里；返回 Skipped 而非
        // panic，与其余无破坏性修复项保持一致。
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "可运行 `openatrading skills install` 安装技能".into(),
        })
    }
}
