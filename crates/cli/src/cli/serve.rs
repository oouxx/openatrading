use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::config::AppConfig;
use trending_agent_core::cron::CronConfig;
use trending_agent_core::gateway::Gateway;
use trending_agent_core::social::telegram::TelegramSender;
use trending_agent_core::social::{NoopSender, NotificationSender};

pub async fn cmd_serve(project_root: &PathBuf) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;

    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let social: Arc<dyn NotificationSender> = match &app_config.social {
        Some(sc) if sc.telegram.as_ref().map(|t| t.enabled).unwrap_or(false) => {
            let tg = sc.telegram.as_ref().unwrap();
            if tg.bot_token.as_ref().map(|t| t.is_empty()).unwrap_or(true)
                || tg.chat_id.as_ref().map(|c| c.is_empty()).unwrap_or(true)
            {
                eprintln!("[gateway] Telegram 配置不完整，使用 NoopSender");
                Arc::new(NoopSender)
            } else {
                Arc::new(TelegramSender {
                    bot_token: tg.bot_token.clone().unwrap_or_default(),
                    chat_id: tg.chat_id.clone().unwrap_or_default(),
                    notify_on_collect: tg.notify_on_collect,
                    notify_on_decision: tg.notify_on_decision,
                    notify_on_trade: tg.notify_on_trade,
                })
            }
        }
        _ => {
            eprintln!("[gateway] Telegram 未配置，使用 NoopSender");
            Arc::new(NoopSender)
        }
    };

    // Use cron config from AppConfig or default
    let cron_cfg = match &app_config.cron {
        Some(c) => CronConfig {
            enabled: c.enabled,
            collection_schedule: c.collection_schedule.clone(),
            decision_schedule: c.decision_schedule.clone(),
        },
        None => CronConfig {
            enabled: true,
            collection_schedule: "*/30 9-15 * * 1-5".to_string(),
            decision_schedule: "0 0 * * 1-5".to_string(),
        },
    };

    gateway.start(&cron_cfg, social).await
}
