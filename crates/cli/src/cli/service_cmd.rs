//! Gateway 系统服务管理（systemd / launchctl 等）。

use std::ffi::OsString;
use std::sync::Arc;

use anyhow::Result;
use service_manager::{
    RestartPolicy, ServiceInstallCtx, ServiceLabel, ServiceManager, ServiceStartCtx, ServiceStatus,
    ServiceStatusCtx, ServiceStopCtx, ServiceUninstallCtx,
};

use crate::cli::config::AppConfig;
use openatrading_core::social::FilteredSender;
use openatrading_core::social::NotificationSender;
use openatrading_core::social::{
    DiscordChannel, LarkChannel, NoopChannel, SlackChannel, TelegramChannel,
};
use openatrading_web::RoleEntry;

const SERVICE_LABEL: &str = "com.openatrading.gateway";

fn manager() -> Result<Box<dyn ServiceManager>> {
    <dyn ServiceManager>::native().map_err(|e| anyhow::anyhow!("无法检测服务管理器: {e}"))
}

fn label() -> ServiceLabel {
    SERVICE_LABEL
        .parse()
        .unwrap_or_else(|e| panic!("invalid SERVICE_LABEL: {e}"))
}

/// 安装并启用 gateway 系统服务（跨平台：systemd / launchctl 等）
pub fn cmd_gateway_install() -> Result<()> {
    let program = std::env::current_exe()?;
    let label = label();
    let m = manager()?;

    m.install(ServiceInstallCtx {
        label,
        program,
        args: vec![OsString::from("gateway"), OsString::from("run")],
        contents: None,
        username: None,
        working_directory: Some(openatrading_core::atrading_home()),
        environment: None,
        autostart: true,
        restart_policy: RestartPolicy::OnFailure {
            delay_secs: Some(10),
        },
    })?;

    println!("✅ 服务已安装 ({SERVICE_LABEL})");
    println!("   openatrading gateway start   # 启动");
    println!("   openatrading gateway stop    # 停止");
    println!("   openatrading gateway status  # 查看状态");
    Ok(())
}

/// 卸载 gateway 系统服务
pub fn cmd_gateway_uninstall() -> Result<()> {
    let label = label();
    let m = manager()?;

    let _ = m.stop(ServiceStopCtx {
        label: label.clone(),
    });
    m.uninstall(ServiceUninstallCtx { label })?;

    println!("✅ 服务已卸载");
    Ok(())
}

/// 启动服务
pub fn cmd_gateway_start() -> Result<()> {
    manager()?.start(ServiceStartCtx { label: label() })?;
    println!("✅ 服务已启动");
    Ok(())
}

/// 停止服务
pub fn cmd_gateway_stop() -> Result<()> {
    manager()?.stop(ServiceStopCtx { label: label() })?;
    println!("✅ 服务已停止");
    Ok(())
}

/// 查看服务状态
pub fn cmd_gateway_status() -> Result<()> {
    let status = manager()?.status(ServiceStatusCtx { label: label() })?;
    match status {
        ServiceStatus::Running => println!("✅ 服务运行中"),
        ServiceStatus::Stopped(msg) => {
            println!(
                "⏹  服务已停止{}",
                msg.as_deref().map(|s| format!(": {s}")).unwrap_or_default()
            );
        }
        ServiceStatus::NotInstalled => println!("❌ 服务未安装"),
    }
    Ok(())
}

// ── Notification sender 工厂 ────────────────────────────────

fn create_telegram_sender(
    tg: &crate::cli::config::TelegramConfig,
) -> Option<Arc<dyn NotificationSender>> {
    if !tg.enabled {
        return None;
    }
    let token = tg.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let chat = tg.chat_id.as_deref().filter(|c| !c.is_empty())?;
    Some(Arc::new(TelegramChannel::new(
        token.to_string(),
        chat.to_string(),
    )))
}

fn create_discord_sender(
    dc: &crate::cli::config::DiscordConfig,
) -> Option<Arc<dyn NotificationSender>> {
    if !dc.enabled {
        return None;
    }
    let bot_token = dc.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let channel_id = dc.channel_id.as_deref().filter(|c| !c.is_empty())?;
    Some(Arc::new(DiscordChannel::new(
        bot_token.to_string(),
        channel_id.to_string(),
    )))
}

fn create_slack_sender(
    sl: &crate::cli::config::SlackConfig,
) -> Option<Arc<dyn NotificationSender>> {
    if !sl.enabled {
        return None;
    }
    let bot_token = sl.bot_token.as_deref().filter(|t| !t.is_empty())?;
    let channel = sl.channel.as_deref().filter(|c| !c.is_empty())?;
    Some(Arc::new(SlackChannel::new(
        bot_token.to_string(),
        channel.to_string(),
    )))
}

fn create_lark_sender(lk: &crate::cli::config::LarkConfig) -> Option<Arc<dyn NotificationSender>> {
    if !lk.enabled {
        return None;
    }
    let app_id = lk.app_id.as_deref().filter(|a| !a.is_empty())?;
    let app_secret = lk.app_secret.as_deref().filter(|s| !s.is_empty())?;
    let receive_id = lk.receive_id.as_deref().filter(|r| !r.is_empty())?;
    Some(Arc::new(LarkChannel::new(
        app_id.to_string(),
        app_secret.to_string(),
        receive_id.to_string(),
    )))
}

/// 根据配置创建通知发送器。
pub fn create_notification_sender(
    sc: &crate::cli::config::SocialConfig,
) -> Arc<dyn NotificationSender> {
    let channel = sc.notify_channel.as_deref().unwrap_or("telegram");

    let sender: Option<Arc<dyn NotificationSender>> = match channel {
        "telegram" => sc.telegram.as_ref().and_then(create_telegram_sender),
        "discord" => sc.discord.as_ref().and_then(create_discord_sender),
        "slack" => sc.slack.as_ref().and_then(create_slack_sender),
        "lark" => sc.lark.as_ref().and_then(create_lark_sender),
        other => {
            openatrading_core::log::warn!(
                "unknown notify_channel: {other}, falling back to telegram"
            );
            sc.telegram.as_ref().and_then(create_telegram_sender)
        }
    };

    match sender {
        Some(inner) => Arc::new(FilteredSender {
            inner,
            notify_on_collect: sc.notify_on_collect,
            notify_on_decision: sc.notify_on_decision,
            notify_on_trade: sc.notify_on_trade,
            notify_on_news: sc.notify_on_news,
        }),
        None => Arc::new(NoopChannel),
    }
}

/// 构建角色列表（配置 + 磁盘发现）。
pub fn build_roles(app_config: &AppConfig) -> Vec<RoleEntry> {
    let mut roles: Vec<RoleEntry> = Vec::new();

    for entry in &app_config.roles {
        if let Some(existing) = roles.iter_mut().find(|r| r.name == entry.name) {
            existing.readonly = entry.readonly;
        } else {
            roles.push(RoleEntry {
                name: entry.name.clone(),
                display_name: entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| entry.name.clone()),
                readonly: entry.readonly,
                initial_capital: entry
                    .initial_capital
                    .unwrap_or(openatrading_core::roles::DEFAULT_INITIAL_CAPITAL),
            });
        }
    }

    let disk_roles = openatrading_core::roles::list_roles_from_disk();
    for disk_role in disk_roles {
        if !roles.iter().any(|r| r.name == disk_role.name) {
            roles.push(disk_role);
        }
    }

    if roles.is_empty() {
        openatrading_core::log::warn!(
            "警告: 未发现任何角色，可在角色管理或 config.toml [[roles]] 中添加"
        );
    }

    roles
}
