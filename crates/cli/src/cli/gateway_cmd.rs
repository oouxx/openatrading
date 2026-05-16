use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use crate::cli::config::AppConfig;
use trending_agent_core::cron::CronConfig;
use trending_agent_core::gateway::Gateway;
use trending_agent_core::social::telegram::TelegramSender;
use trending_agent_core::social::{NoopSender, NotificationSender};

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

/// 前台运行（debug/直接使用），Ctrl+C 停止
pub async fn cmd_gateway_run(project_root: &PathBuf) -> Result<()> {
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
        _ => Arc::new(NoopSender),
    };

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
