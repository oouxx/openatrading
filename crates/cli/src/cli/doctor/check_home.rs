use async_trait::async_trait;

use super::{Check, CheckContext, CheckResult, CheckStatus, FixResult, FixRisk, FixStatus, Result};

/// 品牌根下应由本应用负责创建的子目录。
///
/// `agent` 是 pi-rs 的家（`init::AGENT_DIR_NAME`）——技能、会话、prompt 模板都在它里面，
/// 所以这里断言的是 `agent/` 而不是早先的 `skills/`（那是 pi-rs 迁移前的遗留布局）。
const SUBDIRS: &[&str] = &[openatrading_core::init::AGENT_DIR_NAME, "logs"];

pub struct HomeCheck;

#[async_trait]
impl Check for HomeCheck {
    fn id(&self) -> &'static str {
        "home"
    }
    fn name(&self) -> &'static str {
        "环境目录"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("自动创建目录".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let home = &ctx.atrading_home;
        let mut details = Vec::new();
        let mut error_count = 0;

        if !home.exists() {
            details.push(format!("目录不存在: {}", home.display()));
            error_count += 1;
        } else if !home.is_dir() {
            details.push(format!("路径不是目录: {}", home.display()));
            error_count += 1;
        }

        for name in SUBDIRS {
            let p = home.join(name);
            if !p.exists() {
                details.push(format!("子目录缺失: {name}"));
                error_count += 1;
            } else if !p.is_dir() {
                details.push(format!("子目录不是目录: {name}"));
                error_count += 1;
            }
        }

        if error_count == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{}", home.display()),
                details: vec![],
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Error(format!("{error_count} 个问题")),
                summary: format!("{}", home.display()),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let home = &ctx.atrading_home;
        std::fs::create_dir_all(home)?;
        for name in SUBDIRS {
            std::fs::create_dir_all(home.join(name))?;
        }
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已创建 {} 及其子目录", home.display()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 布局契约：品牌根里必须能找到 pi-rs 的 `agent/`（技能与会话都在它下面）。
    /// 曾经断言的是迁移前的 `skills/`，那会让 doctor 对正确的布局报"子目录缺失"。
    #[tokio::test]
    async fn missing_agent_dir_is_reported_and_created_by_fix() {
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let home = tmp.path().join(".atrading");
        let ctx = CheckContext {
            atrading_home: home.clone(),
            app_config: None,
        };

        // 空品牌根：agent 与 logs 都缺，应报错并点名。
        let result = HomeCheck.check(&ctx).await;
        assert!(matches!(result.status, CheckStatus::Error(_)), "{result:?}");
        assert!(
            result
                .details
                .iter()
                .any(|d| d.contains(openatrading_core::init::AGENT_DIR_NAME)),
            "应点名缺失的 agent 目录: {:?}",
            result.details
        );
        // 不再把迁移前的 skills/ 当作必需要求。
        assert!(
            !result.details.iter().any(|d| d.contains("skills")),
            "不应再要求 skills/: {:?}",
            result.details
        );

        // fix 后：agent 目录就位，检查应通过。
        HomeCheck
            .fix(&ctx)
            .await
            .unwrap_or_else(|e| panic!("fix: {e}"));
        assert!(home.join(openatrading_core::init::AGENT_DIR_NAME).is_dir());
        let after = HomeCheck.check(&ctx).await;
        assert!(matches!(after.status, CheckStatus::Ok), "{after:?}");
    }
}
