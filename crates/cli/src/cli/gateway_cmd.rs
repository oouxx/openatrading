use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::cli::config::{AppConfig, DiscordConfig, LarkConfig, SlackConfig, TelegramConfig};
use openatrading_core::social::channel_to_sender;
use openatrading_core::social::channels::discord::DiscordChannel;
use openatrading_core::social::channels::lark::LarkChannel;
use openatrading_core::social::channels::slack::SlackChannel;
use openatrading_core::social::channels::telegram::TelegramChannel;
use openatrading_core::social::{FilteredSender, NoopSender, NotificationSender};
use openatrading_web::gateway::{Gateway, GatewayConfig};
use openatrading_web::RoleEntry;

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

fn generate_unit_file(project_root: &Path) -> Result<String> {
    let current_exe = std::env::current_exe().context("无法获取当前二进制路径")?;
    let work_dir = if project_root.is_absolute() {
        project_root.to_path_buf()
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
pub fn cmd_gateway_install(project_root: &Path) -> Result<()> {
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
    Some(channel_to_sender(Box::new(TelegramChannel::new(
        token.to_string(),
        chat.to_string(),
    ))))
}

fn create_discord_sender(dc: &DiscordConfig) -> Option<Box<dyn NotificationSender>> {
    if !dc.enabled {
        return None;
    }
    let bot_token = dc.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let channel_id = dc.channel_id.as_deref().filter(|c| !c.is_empty())?;
    Some(channel_to_sender(Box::new(DiscordChannel::new(
        bot_token.to_string(),
        channel_id.to_string(),
    ))))
}

fn create_slack_sender(sl: &SlackConfig) -> Option<Box<dyn NotificationSender>> {
    if !sl.enabled {
        return None;
    }
    let bot_token = sl.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let channel = sl.channel.as_deref().filter(|c| !c.is_empty())?;
    Some(channel_to_sender(Box::new(SlackChannel::new(
        bot_token.to_string(),
        channel.to_string(),
    ))))
}

fn create_lark_sender(lk: &LarkConfig) -> Option<Box<dyn NotificationSender>> {
    if !lk.enabled {
        return None;
    }
    let app_id = lk.app_id.as_deref().filter(|a| !a.is_empty())?;
    let app_secret = lk.app_secret.as_deref().filter(|s| !s.is_empty())?;
    let receive_id = lk.receive_id.as_deref().filter(|r| !r.is_empty())?;
    Some(channel_to_sender(Box::new(LarkChannel::new(
        app_id.to_string(),
        app_secret.to_string(),
        receive_id.to_string(),
    ))))
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
        },
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
    let skills_dir = openatrading_core::roles::default_skills_dir();
    let mut roles: Vec<RoleEntry> = Vec::new();

    // Config-toml roles: mark as readonly
    for entry in &app_config.roles {
        let skill_path = skills_dir.join(&entry.name).join("SKILL.md");
        if let Ok(raw) = std::fs::read_to_string(&skill_path) {
            let (fm, body) = openatrading_core::roles::parse_frontmatter(&raw);
            if !body.is_empty() {
                let display_name = fm
                    .get("description")
                    .cloned()
                    .unwrap_or_else(|| entry.name.clone());
                if let Some(existing) = roles.iter_mut().find(|r| r.name == entry.name) {
                    existing.readonly = entry.readonly;
                } else {
                    roles.push(RoleEntry {
                        name: entry.name.clone(),
                        display_name,
                        strategy: body.to_string(),
                        readonly: entry.readonly,
                    });
                }
            }
        }
    }

    // Disk-discovered roles (editable, but config-toml readonly already set above)
    let disk_roles = openatrading_core::roles::list_roles_from_disk();
    for disk_role in disk_roles {
        if !roles.iter().any(|r| r.name == disk_role.name) {
            roles.push(disk_role);
        }
    }

    if roles.is_empty() {
        eprintln!("[gateway] 警告: skills/ 目录中未发现任何角色");
    }

    roles
}

/// 前台运行（debug/直接使用），Ctrl+C 停止
pub async fn cmd_gateway_run(project_root: &Path, cron_only: bool, web_only: bool) -> Result<()> {
    use openatrading_core::cron::{self, CronConfig};
    use openatrading_core::llm::LlmClient;
    use openatrading_core::social::telegram_bot::{TelegramBot, TelegramBotConfig};

    let app_config = AppConfig::load(project_root)?;

    let db_path = project_root
        .join(".atrading")
        .join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let do_cron = !web_only || cron_only;
    let do_web = !cron_only || web_only;

    let roles = build_roles(&app_config);
    let mut handles = Vec::new();

    // Clone for cron/telegram/web use before gateway is moved
    let memory = gateway.memory.clone();
    let trade = gateway.trade.clone();
    let web_roles = roles.clone();

    // ── Cron (CLI layer) ──────────────────────────
    let social: Arc<dyn NotificationSender> = match &app_config.social {
        Some(sc) => Arc::from(create_notification_sender(sc)),
        None => Arc::new(NoopSender),
    };

    if do_cron {
        let cron_cfg = match &app_config.cron {
            Some(c) => CronConfig {
                enabled: c.enabled,
                collection_schedule: c.collection_schedule.clone(),
                decision_schedule: c.decision_schedule.clone(),
                news_schedule: c.news_schedule.clone(),
            },
            None => CronConfig {
                enabled: true,
                collection_schedule: "0 */30 9-15 * * 1-5".to_string(),
                decision_schedule: "0 0 0 * * 1-5".to_string(),
                news_schedule: "0 */5 9-15 * * 1-5".to_string(),
            },
        };

        let cron_social = social.clone();
        let cron_roles = roles.clone();
        let cron_memory = memory.clone();
        let cron_trade = trade.clone();
        let cron_llm = LlmClient::from_config(
            app_config.llm.provider.as_deref(),
            app_config.llm.model.as_deref(),
        );
        handles.push(tokio::spawn(async move {
            if let Err(e) = cron::start_scheduler(
                &cron_cfg,
                cron_memory,
                cron_trade,
                cron_social,
                cron_llm,
                openatrading_core::api::Client::new(),
                cron_roles,
            )
            .await
            {
                eprintln!("[gateway] cron error: {e}");
            }
        }));
    }

    // ── Web (web crate) ───────────────────────────
    if do_web {
        let web_llm_provider = app_config.llm.provider.clone();
        let web_llm_model = app_config.llm.model.clone();
        let gh = tokio::spawn(async move {
            if let Err(e) = gateway
                .start(GatewayConfig {
                    web_port: 8080,
                    roles: web_roles.clone(),
                    llm_provider: web_llm_provider,
                    llm_model: web_llm_model,
                })
                .await
            {
                eprintln!("[gateway] web error: {e}");
            }
        });
        handles.push(gh);
    }

    // ── Telegram Bot (CLI layer) ──────────────────
    if let Some(sc) = &app_config.social {
        if let Some(tg) = &sc.telegram {
            if tg.enabled && tg.bot_polling {
                if let (Some(token), Some(chat_id)) = (&tg.bot_token, &tg.chat_id) {
                    if !token.is_empty() && !chat_id.is_empty() {
                        let config = TelegramBotConfig {
                            bot_token: token.clone(),
                            chat_id: chat_id.clone(),
                            llm_provider: app_config.llm.provider.clone(),
                            llm_model: app_config.llm.model.clone(),
                        };
                        let bot = TelegramBot::new(config, roles, memory.clone(), trade.clone());
                        handles.push(tokio::spawn(async move {
                            if let Err(e) = bot.start_polling().await {
                                eprintln!("[gateway] telegram bot error: {e}");
                            }
                        }));
                        eprintln!("[gateway] telegram bot: enabled");
                    }
                }
            }
        }
    }

    if handles.is_empty() {
        eprintln!("[gateway] nothing to run");
        return Ok(());
    }

    // 所有服务已 spawn，发送启动通知
    if let Some(sc) = &app_config.social {
        let channel = sc.notify_channel.as_deref().unwrap_or("telegram");
        match social.send_text("✅ OpenATrading Gateway 已启动").await {
            Ok(()) => eprintln!("[gateway] {channel}: ✅ connected"),
            Err(e) => eprintln!("[gateway] {channel}: ❌ connection failed: {e}"),
        }
    }

    eprintln!("[gateway] running (Ctrl+C to stop)");
    tokio::signal::ctrl_c().await?;
    eprintln!("\n[gateway] shutting down...");
    for h in handles {
        h.abort();
    }

    Ok(())
}
