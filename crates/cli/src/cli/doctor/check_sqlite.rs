use async_trait::async_trait;
use std::path::PathBuf;

use super::*;

pub struct SqliteCheck;

fn resolve_db_path(ctx: &CheckContext) -> PathBuf {
    let db_name = ctx
        .app_config
        .as_ref()
        .map(|c| c.service.db_path.as_str())
        .unwrap_or("data.db");
    let p = PathBuf::from(db_name);
    if p.is_relative() {
        ctx.atrading_home.join(db_name)
    } else {
        p
    }
}

#[async_trait]
impl Check for SqliteCheck {
    fn id(&self) -> &'static str {
        "sqlite"
    }
    fn name(&self) -> &'static str {
        "数据库"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("初始化数据库".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let db_path = resolve_db_path(ctx);
        match openatrading_web::gateway::Gateway::init(&db_path) {
            Ok(_) => CheckResult {
                status: CheckStatus::Ok,
                summary: format!("{}", db_path.display()),
                details: vec![],
                duration_ms: 0,
            },
            Err(e) => CheckResult {
                status: CheckStatus::Error("连接失败".into()),
                summary: format!("{}", db_path.display()),
                details: vec![format!("{e:#}")],
                duration_ms: 0,
            },
        }
    }

    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult> {
        let db_path = resolve_db_path(ctx);
        // Gateway::init creates tables if not exist
        openatrading_web::gateway::Gateway::init(&db_path)?;
        Ok(FixResult {
            status: FixStatus::Fixed,
            message: format!("数据库已初始化: {}", db_path.display()),
        })
    }
}
