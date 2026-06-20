use async_trait::async_trait;

use super::*;

pub struct EnvCheck;

/// 环境变量白名单：仅列出代码实际会读取的变量。
///
/// 历史上此列表曾包含一些代码库无任何消费者的变量（提示用户设置系统从不
/// 读取的变量属于误导），已移除，仅保留代码实际在 `crates/api` 与
/// `crates/web` 中读取的 `JIUYAN_*`。
const REQUIRED_VARS: &[(&str, &str)] = &[
    ("JIUYAN_PHONE", "韭研公社手机号"),
    ("JIUYAN_PASSWORD", "韭研公社密码"),
    ("JIUYAN_INIT_TOKEN", "韭研公社初始 Token"),
];

#[async_trait]
impl Check for EnvCheck {
    fn id(&self) -> &'static str {
        "env"
    }
    fn name(&self) -> &'static str {
        "环境变量"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("生成 .env 模板".into())
    }

    async fn check(&self, _ctx: &CheckContext) -> CheckResult {
        let mut details = Vec::new();
        let mut missing = 0usize;

        for (var, desc) in REQUIRED_VARS {
            if std::env::var(var).ok().filter(|v| !v.is_empty()).is_none() {
                details.push(format!("{} ({})", var, desc));
                missing += 1;
            }
        }

        if missing == 0 {
            CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{} 个变量均已设置", REQUIRED_VARS.len()),
                details: vec![],
                duration_ms: 0,
            }
        } else {
            CheckResult {
                status: CheckStatus::Warn(format!("{} 个变量未设置", missing)),
                summary: "部分环境变量缺失".into(),
                details,
                duration_ms: 0,
            }
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let env_path = ctx.atrading_home.join(".env");
        if env_path.exists() {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: ".env 文件已存在".into(),
            });
        }
        let content = REQUIRED_VARS
            .iter()
            .map(|(var, desc)| format!("# {}={}", desc, var))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&env_path, content)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("已生成模板 {}", env_path.display()),
        })
    }
}
