use async_trait::async_trait;

use super::*;

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

        let subdirs = ["skills", "logs", "pipelines"];
        for name in &subdirs {
            let p = home.join(name);
            if !p.exists() {
                details.push(format!("子目录缺失: {}", name));
                error_count += 1;
            } else if !p.is_dir() {
                details.push(format!("子目录不是目录: {}", name));
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
                status: CheckStatus::Error(format!("{} 个问题", error_count)),
                summary: format!("{}", home.display()),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let home = &ctx.atrading_home;
        std::fs::create_dir_all(home)?;
        for name in &["skills", "logs", "pipelines"] {
            std::fs::create_dir_all(home.join(name))?;
        }
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已创建 {} 及其子目录", home.display()),
        })
    }
}
