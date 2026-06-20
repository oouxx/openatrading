use async_trait::async_trait;
use std::str::FromStr;

use super::*;

pub struct CronCheck;

/// Validate a cron expression as 6-field format.
/// The `cron` crate's Schedule::from_str parses both 6-field and 7-field;
/// we explicitly reject 7-field by counting tokens.
fn validate_cron(expr: &str) -> Result<(), String> {
    let field_count = expr.split_whitespace().count();
    if field_count != 6 {
        return Err(format!("字段数应为 6 (当前 {field_count}): {expr}"));
    }
    cron::Schedule::from_str(expr).map_err(|e| format!("解析失败: {e} ({expr})"))?;
    Ok(())
}

#[async_trait]
impl Check for CronCheck {
    fn id(&self) -> &'static str {
        "cron"
    }
    fn name(&self) -> &'static str {
        "定时任务"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("写入默认值".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        match &ctx.app_config {
            Some(cfg) => match &cfg.cron {
                Some(c) if c.enabled => {
                    let mut details = Vec::new();
                    let schedules = [
                        ("采集", &c.collection_schedule),
                        ("决策", &c.decision_schedule),
                        ("新闻", &c.news_schedule),
                    ];
                    let mut errors = 0usize;

                    for (name, expr) in &schedules {
                        match validate_cron(expr) {
                            Ok(()) => details.push(format!("{}: {} ✓", name, expr)),
                            Err(e) => {
                                details.push(format!("{}: {} ✗ ({})", name, expr, e));
                                errors += 1;
                            },
                        }
                    }

                    if errors == 0 {
                        CheckResult {
                            status: CheckStatus::Ok,
                            summary: format!("{} 个调度均有效", schedules.len()),
                            details,
                            duration_ms: 0,
                        }
                    } else {
                        CheckResult {
                            status: CheckStatus::Error(format!("{} 个表达式无效", errors)),
                            summary: "定时任务调度表达式校验失败".into(),
                            details,
                            duration_ms: 0,
                        }
                    }
                },
                _ => CheckResult {
                    status: CheckStatus::Warn("cron 未启用".into()),
                    summary: "定时任务调度未启用".into(),
                    details: vec![],
                    duration_ms: 0,
                },
            },
            None => CheckResult {
                status: CheckStatus::Error("无法加载配置".into()),
                summary: "无法加载配置".into(),
                details: vec![],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let config_path = ctx.atrading_home.join("config.toml");
        let content = std::fs::read_to_string(&config_path)?;
        let has_cron = content.contains("[cron]");
        if has_cron {
            return Ok(FixResult {
                status: FixStatus::Skipped,
                message: "[cron] 段已存在，请手动修正 cron 表达式".into(),
            });
        }
        let cron_section = r#"
[cron]
enabled = true
collection_schedule = "0 0 18 * * 1-5"
decision_schedule = "0 */30 9-15 * * 1-5"
news_schedule = "0 */5 9-15 * * 1-5"
"#;
        std::fs::write(&config_path, content + cron_section)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: "已追加 [cron] 配置段".into(),
        })
    }
}
