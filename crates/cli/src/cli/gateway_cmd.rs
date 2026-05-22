use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use crate::cli::config::{AppConfig, DiscordConfig, LarkConfig, SlackConfig, TelegramConfig};
use trending_agent_core::cron::CronConfig;
use trending_agent_core::gateway::{Gateway, GatewayConfig};
use trending_agent_core::social::discord::DiscordSender;
use trending_agent_core::social::lark::LarkSender;
use trending_agent_core::social::slack::SlackSender;
use trending_agent_core::social::telegram::TelegramSender;
use trending_agent_core::social::telegram_bot::TelegramBotConfig;
use trending_agent_core::social::{FilteredSender, NoopSender, NotificationSender};
use trending_agent_core::web::RoleEntry;

const SERVICE_NAME: &str = "openatrading";

fn service_file_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".config/systemd/user")
        .join(format!("{SERVICE_NAME}.service"))
}

fn systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .context("systemctl 命令失败，请确认 systemd 已安装")?;
    if !status.success() {
        anyhow::bail!("systemctl --user {} 返回非零退出码", args.join(" "));
    }
    Ok(())
}

fn generate_unit_file(project_root: &PathBuf) -> Result<String> {
    let current_exe = std::env::current_exe().context("无法获取当前二进制路径")?;
    let work_dir = if project_root.is_absolute() {
        project_root.clone()
    } else {
        std::env::current_dir()?.join(project_root)
    };

    Ok(format!(
        r#"[Unit]
Description=OpenATrading - A股短线交易 Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={exe} gateway run
WorkingDirectory={work_dir}
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier={name}

[Install]
WantedBy=default.target
"#,
        exe = current_exe.display(),
        work_dir = work_dir.display(),
        name = SERVICE_NAME,
    ))
}

/// 安装 systemd 服务
pub fn cmd_gateway_install(project_root: &PathBuf) -> Result<()> {
    let unit = generate_unit_file(project_root)?;
    let path = service_file_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &unit)?;

    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", SERVICE_NAME])?;

    println!("✅ systemd 服务已安装: {}", path.display());
    println!("   openatrading gateway start   # 启动");
    println!("   openatrading gateway stop    # 停止");
    println!("   openatrading gateway status  # 查看状态");
    println!("   journalctl --user -u openatrading -f  # 查看日志");
    Ok(())
}

/// 卸载 systemd 服务
pub fn cmd_gateway_uninstall() -> Result<()> {
    let _ = systemctl(&["stop", SERVICE_NAME]);
    let _ = systemctl(&["disable", SERVICE_NAME]);
    let path = service_file_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let _ = systemctl(&["daemon-reload"]);
    println!("✅ systemd 服务已卸载");
    Ok(())
}

/// systemctl start
pub fn cmd_gateway_start() -> Result<()> {
    systemctl(&["start", SERVICE_NAME])?;
    println!("✅ 服务已启动");
    Ok(())
}

/// systemctl stop
pub fn cmd_gateway_stop() -> Result<()> {
    systemctl(&["stop", SERVICE_NAME])?;
    println!("✅ 服务已停止");
    Ok(())
}

/// systemctl status
pub fn cmd_gateway_status() -> Result<()> {
    systemctl(&["status", SERVICE_NAME])?;
    Ok(())
}

fn create_telegram_sender(tg: &TelegramConfig) -> Option<Box<dyn NotificationSender>> {
    if !tg.enabled {
        return None;
    }
    let token = tg.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let chat = tg.chat_id.as_deref().filter(|c| !c.is_empty())?;
    Some(Box::new(TelegramSender {
        bot_token: token.to_string(),
        chat_id: chat.to_string(),
    }))
}

fn create_discord_sender(dc: &DiscordConfig) -> Option<Box<dyn NotificationSender>> {
    if !dc.enabled {
        return None;
    }
    let url = dc.webhook_url.as_deref().filter(|u| !u.is_empty())?;
    Some(Box::new(DiscordSender {
        webhook_url: url.to_string(),
    }))
}

fn create_slack_sender(sl: &SlackConfig) -> Option<Box<dyn NotificationSender>> {
    if !sl.enabled {
        return None;
    }
    let url = sl.webhook_url.as_deref().filter(|u| !u.is_empty())?;
    Some(Box::new(SlackSender {
        webhook_url: url.to_string(),
    }))
}

fn create_lark_sender(lk: &LarkConfig) -> Option<Box<dyn NotificationSender>> {
    if !lk.enabled {
        return None;
    }
    let url = lk.webhook_url.as_deref().filter(|u| !u.is_empty())?;
    Some(Box::new(LarkSender {
        webhook_url: url.to_string(),
    }))
}

pub fn create_notification_sender(
    sc: &crate::cli::config::SocialConfig,
) -> Box<dyn NotificationSender> {
    let channel = sc.notify_channel.as_deref().unwrap_or("telegram");

    let sender: Option<Box<dyn NotificationSender>> = match channel {
        "telegram" => sc.telegram.as_ref().and_then(create_telegram_sender),
        "discord" => sc.discord.as_ref().and_then(create_discord_sender),
        "slack" => sc.slack.as_ref().and_then(create_slack_sender),
        "lark" => sc.lark.as_ref().and_then(create_lark_sender),
        other => {
            eprintln!("[gateway] unknown notify_channel: {other}, falling back to telegram");
            sc.telegram.as_ref().and_then(create_telegram_sender)
        }
    };

    match sender {
        Some(inner) => Box::new(FilteredSender {
            inner,
            notify_on_collect: sc.notify_on_collect,
            notify_on_decision: sc.notify_on_decision,
            notify_on_trade: sc.notify_on_trade,
            notify_on_news: sc.notify_on_news,
        }),
        None => Box::new(NoopSender),
    }
}

pub fn build_roles(app_config: &AppConfig) -> Vec<RoleEntry> {
    let mut roles: Vec<RoleEntry> = Vec::new();

    for entry in &app_config.roles {
        let strategy = AppConfig::load_strategy(entry).unwrap_or_else(|e| {
            eprintln!(
                "[gateway] warning: failed to load strategy for role '{}': {e}",
                entry.config.name
            );
            String::new()
        });

        if strategy.is_empty() {
            continue;
        }

        if let Some(existing) = roles.iter_mut().find(|r| r.name == entry.config.name) {
            existing.strategy = strategy;
        } else {
            roles.push(RoleEntry {
                name: entry.config.name.clone(),
                display_name: entry.config.display_name.clone(),
                strategy,
            });
        }
    }

    if roles.is_empty() {
        for (name, display_name, strategy) in trending_agent_core::roles::BUILTIN_STRATEGIES {
            roles.push(RoleEntry {
                name: name.to_string(),
                display_name: display_name.to_string(),
                strategy: strategy.to_string(),
            });
        }
    }

    roles
}

/// 前台运行（debug/直接使用），Ctrl+C 停止
///
/// - `cron_only`: 仅运行 cron scheduler，不启动 web server
/// - `web_only`: 仅运行 web server（端口 8080），不启动 cron scheduler
/// - 两者均 false（默认）: cron + web 同时运行
/// - 两者均 true: 等同于默认行为
pub async fn cmd_gateway_run(
    project_root: &PathBuf,
    cron_only: bool,
    web_only: bool,
) -> Result<()> {
    let app_config = AppConfig::load(project_root)?;

    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let do_cron = !web_only || cron_only;
    let do_web = !cron_only || web_only;

    let telegram_bot_config = match &app_config.social {
        Some(sc)
            if sc
                .telegram
                .as_ref()
                .map(|t| t.enabled && t.bot_polling)
                .unwrap_or(false) =>
        {
            let tg = sc.telegram.as_ref().unwrap();
            let token = tg.bot_token.clone().unwrap_or_default();
            let chat_id = tg.chat_id.clone().unwrap_or_default();
            if token.is_empty() || chat_id.is_empty() {
                None
            } else {
                Some(TelegramBotConfig {
                    bot_token: token,
                    chat_id,
                })
            }
        }
        _ => None,
    };

    let cron_entry = if do_cron {
        let social: Arc<dyn NotificationSender> = match &app_config.social {
            Some(sc) => Arc::from(create_notification_sender(sc)),
            None => Arc::new(NoopSender),
        };

        let cron_cfg = match &app_config.cron {
            Some(c) => CronConfig {
                enabled: c.enabled,
                collection_schedule: c.collection_schedule.clone(),
                decision_schedule: c.decision_schedule.clone(),
                monitor_schedule: c.monitor_schedule.clone(),
                news_schedule: c.news_schedule.clone(),
            },
            None => CronConfig {
                enabled: true,
                collection_schedule: "0 */30 9-15 * * 1-5 *".to_string(),
                decision_schedule: "0 0 0 * * 1-5 *".to_string(),
                monitor_schedule: "0 */1 9-15 * * 1-5 *".to_string(),
                news_schedule: "0 */5 9-15 * * 1-5 *".to_string(),
            },
        };

        Some((cron_cfg, social))
    } else {
        None
    };

    let web_port = if do_web { Some(8080u16) } else { None };

    // ── Build roles with merged strategies ──────────
    let roles = build_roles(&app_config);

    // ── Notification channel diagnostics ──────────
    if do_cron {
        if let Some(sc) = &app_config.social {
            let channel = sc.notify_channel.as_deref().unwrap_or("telegram");
            let sender = create_notification_sender(sc);
            match sender.send_text("✅ OpenATrading Gateway 已启动").await {
                Ok(()) => eprintln!("[gateway] {channel}: ✅ connected"),
                Err(e) => eprintln!("[gateway] {channel}: ❌ connection failed: {e}"),
            }
        } else {
            eprintln!("[gateway] social: ⚠ not configured");
        }
    }

    let strategies_dir = project_root.join(".atrading").join("strategies");

    gateway
        .start(GatewayConfig {
            cron: cron_entry,
            web_port,
            roles,
            strategies_dir,
            telegram_bot: telegram_bot_config,
        })
        .await
}
