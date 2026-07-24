use anyhow::Result;
use std::ffi::OsString;
use std::sync::Arc;

use service_manager::{ServiceManager, ServiceLabel, ServiceInstallCtx, RestartPolicy, ServiceStopCtx, ServiceUninstallCtx, ServiceStartCtx, ServiceStatusCtx, ServiceStatus};

use crate::cli::config::{AppConfig, DiscordConfig, LarkConfig, SlackConfig, TelegramConfig};
use openatrading_core::channels::adapter::{launch_adapters, MessageAdapter};
use openatrading_core::channels::discord::DiscordAdapter;
use openatrading_core::channels::telegram::TelegramAdapter;
use openatrading_core::social::FilteredSender;
use openatrading_core::social::NotificationSender;
use openatrading_core::social::{
    DiscordChannel, LarkChannel, NoopChannel, SlackChannel, TelegramChannel,
};
use openatrading_web::gateway::{Gateway, GatewayConfig};
use openatrading_web::RoleEntry;

const SERVICE_LABEL: &str = "com.openatrading.gateway";

fn manager() -> Result<Box<dyn ServiceManager>> {
    <dyn ServiceManager>::native().map_err(|e| anyhow::anyhow!("无法检测服务管理器: {e}"))
}

fn label() -> ServiceLabel {
    SERVICE_LABEL.parse().expect("invalid SERVICE_LABEL")
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
        },
        ServiceStatus::NotInstalled => println!("❌ 服务未安装"),
    }
    Ok(())
}

fn create_telegram_sender(tg: &TelegramConfig) -> Option<Arc<dyn NotificationSender>> {
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

fn create_discord_sender(dc: &DiscordConfig) -> Option<Arc<dyn NotificationSender>> {
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

fn create_slack_sender(sl: &SlackConfig) -> Option<Arc<dyn NotificationSender>> {
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

fn create_lark_sender(lk: &LarkConfig) -> Option<Arc<dyn NotificationSender>> {
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
        },
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
                        strategy: body.clone(),
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
        openatrading_core::log::warn!("警告: skills/ 目录中未发现任何角色");
    }

    roles
}

/// 前台运行（debug/直接使用），Ctrl+C 停止
pub async fn cmd_gateway_run(cron_only: bool, web_only: bool, no_mcp: bool) -> Result<()> {
    use openatrading_core::context::AppContext;
    use openatrading_core::cron::{self, CronConfig};

    let app_config = AppConfig::load()?;
    let home = openatrading_core::atrading_home();

    // 初始化 tracing（stderr 输出，RUST_LOG 控制级别）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    openatrading_core::log::info!("home: {}", home.display());

    let db_path = home.join(&app_config.service.db_path);

    let gateway = Gateway::init(&db_path)?;

    let do_cron = !web_only || cron_only;
    let do_web = !cron_only || web_only;
    let do_mcp = !cron_only && !web_only && !no_mcp && app_config.mcp_server.enabled;

    let roles = build_roles(&app_config);
    let mut handles = Vec::new();

    // Clone for cron/telegram/web use before gateway is moved
    let memory = gateway.memory.clone();
    let trade = gateway.trade.clone();
    let web_roles = roles.clone();
    let mcp_servers = app_config.mcp_servers.clone();
    let cron_mcp = mcp_servers.clone();
    let web_mcp = mcp_servers.clone();

    // ── Cron (CLI layer) ──────────────────────────
    let social: Arc<dyn NotificationSender> = match &app_config.social {
        Some(sc) => create_notification_sender(sc),
        None => Arc::new(NoopChannel),
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
                collection_schedule: "0 0 18 * * 1-5".to_string(),
                decision_schedule: "0 */30 9-15 * * 1-5".to_string(),
                news_schedule: "0 */5 9-15 * * 1-5".to_string(),
            },
        };

        let cron_ctx = Arc::new(
            AppContext::build(
                app_config.llm.provider.as_deref(),
                app_config.llm.model.as_deref(),
                &cron_mcp,
                memory.clone(),
                trade.clone(),
                social.clone(),
            )
            .await,
        );
        let cron_roles = roles.clone();
        handles.push(tokio::spawn(async move {
            if let Err(e) = cron::start_scheduler(cron_ctx, cron_cfg, cron_roles).await {
                openatrading_core::log::error!("cron error: {e}");
            }
        }));
    }

    // ── Web (web crate) ───────────────────────────
    if do_web {
        // Clone out the bind string before the spawn: `app_config` is still
        // borrowed below for the MCP server bind and telegram sections.
        let web_bind = app_config.service.web_bind.clone();
        let web_ctx = AppContext::build(
            app_config.llm.provider.as_deref(),
            app_config.llm.model.as_deref(),
            &web_mcp,
            memory.clone(),
            trade.clone(),
            social.clone(),
        )
        .await;
        let gh = tokio::spawn(async move {
            if let Err(e) = gateway
                .start(
                    GatewayConfig {
                        web_bind,
                        roles: web_roles.clone(),
                        mcp_servers: web_mcp.clone(),
                    },
                    web_ctx,
                )
                .await
            {
                openatrading_core::log::error!("web error: {e}");
            }
        });
        handles.push(gh);
    }

    // ── MCP Server (CLI layer) ─────────────────────
    let mut mcp_shutdown: Option<tokio::sync::watch::Sender<bool>> = None;
    if do_mcp {
        let mcp_bind: std::net::SocketAddr = app_config.mcp_server.bind.parse().map_err(|e| {
            anyhow::anyhow!(
                "invalid mcp_server.bind {}: {e}",
                app_config.mcp_server.bind
            )
        })?;
        let (mcp_tx, mcp_rx) = tokio::sync::watch::channel(false);
        mcp_shutdown.replace(mcp_tx);

        let registry = openatrading_core::llm::ToolRegistry::new();
        let tools = registry.all_tools();
        handles.push(tokio::spawn(async move {
            if let Err(e) = openatrading_core::llm::serve_http(tools, mcp_bind, mcp_rx).await {
                openatrading_core::log::error!("[gateway] mcp error: {e}");
            }
        }));
        openatrading_core::log::info!("mcp: enabled at http://{mcp_bind}/mcp");
    }

    // ── 平台适配器 ─────────────────────────────────
    let mut adapters: Vec<Arc<dyn MessageAdapter>> = Vec::new();

    if let Some(sc) = &app_config.social {
        if let Some(tg) = &sc.telegram {
            if tg.enabled && tg.bot_polling {
                if let (Some(token), Some(chat_id)) = (&tg.bot_token, &tg.chat_id) {
                    if !token.is_empty() && !chat_id.is_empty() {
                        adapters.push(Arc::new(TelegramAdapter::new(
                            token.clone(),
                            chat_id.clone(),
                        )));
                        openatrading_core::log::info!("telegram adapter: enabled");
                    }
                }
            }
        }

        if let Some(dc) = &sc.discord {
            if dc.enabled && !adapters.iter().any(|a| a.platform_name() == "discord") {
                if let Some(token) = &dc.bot_token {
                    if !token.is_empty() {
                        let mut da = DiscordAdapter::new(token.clone());
                        if let Some(ch) = &dc.channel_id {
                            da = da.with_channel(ch);
                        }
                        adapters.push(Arc::new(da));
                        openatrading_core::log::info!("discord adapter: enabled");
                    }
                }
            }
        }
    }

    if !adapters.is_empty() {
        let adapter_ctx = Arc::new(
            AppContext::build(
                app_config.llm.provider.as_deref(),
                app_config.llm.model.as_deref(),
                &mcp_servers,
                memory.clone(),
                trade.clone(),
                social.clone(),
            )
            .await,
        );
        let (rx, adapter_refs, adapter_handles) = launch_adapters(adapters, 1024);
        handles.extend(adapter_handles);

        let handler_roles = roles.clone();
        handles.push(tokio::spawn(async move {
            if let Err(e) = openatrading_core::social::adapter_handler::run_adapter_handler(
                rx,
                adapter_refs,
                handler_roles,
                (*adapter_ctx).clone(),
            )
            .await
            {
                openatrading_core::log::error!("adapter handler error: {e}");
            }
        }));
    }

    if handles.is_empty() {
        openatrading_core::log::info!("nothing to run");
        return Ok(());
    }

    // 所有服务已 spawn，发送启动通知
    if let Some(sc) = &app_config.social {
        let channel = sc.notify_channel.as_deref().unwrap_or("telegram");
        match social.send_text("✅ OpenATrading Gateway 已启动").await {
            Ok(()) => openatrading_core::log::info!("{channel}: ✅ connected"),
            Err(e) => openatrading_core::log::warn!("{channel}: ❌ connection failed: {e}"),
        }
    }

    openatrading_core::log::info!("running (Ctrl+C to stop)");
    tokio::signal::ctrl_c().await?;
    openatrading_core::log::info!("shutting down...");

    if let Some(tx) = mcp_shutdown.take() {
        let _ = tx.send(true);
    }

    for h in handles {
        h.abort();
    }

    Ok(())
}
