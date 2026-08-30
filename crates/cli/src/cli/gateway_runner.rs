//! Gateway 前台运行逻辑。

use anyhow::Result;
use std::sync::Arc;

use openatrading_core::channels::adapter::launch_channels;
use openatrading_core::context::AppContext;
use openatrading_core::cron::{self, CronConfig};
use openatrading_core::social::NotificationSender;
use openatrading_web::gateway::{Gateway, GatewayConfig};

use crate::cli::config::AppConfig;
use crate::cli::service_cmd::{build_roles, create_notification_sender};

/// 前台运行 gateway（debug/直接使用），Ctrl+C 停止。
pub async fn cmd_gateway_run(cron_only: bool, web_only: bool, no_mcp: bool) -> Result<()> {
    let app_config = AppConfig::load()?;
    let home = openatrading_core::atrading_home();

    // 初始化 tracing
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
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    let memory = gateway.memory.clone();
    let trade = gateway.trade.clone();
    let web_roles = roles.clone();
    let mcp_servers = app_config.mcp_servers.clone();
    let cron_mcp = mcp_servers.clone();
    let web_mcp = mcp_servers.clone();

    // ── 通知发送器 ──────────────────────────────────
    let social: Arc<dyn NotificationSender> = match &app_config.social {
        Some(sc) => create_notification_sender(sc),
        None => Arc::new(openatrading_core::social::NoopChannel),
    };

    // ── Cron ────────────────────────────────────────
    if do_cron {
        let cron_cfg = app_config.cron.as_ref().map_or_else(
            || CronConfig {
                enabled: true,
                collection_schedule: "0 0 18 * * 1-5".to_string(),
                news_schedule: "0 */5 9-15 * * 1-5".to_string(),
            },
            |c| CronConfig {
                enabled: c.enabled,
                collection_schedule: c.collection_schedule.clone(),
                news_schedule: c.news_schedule.clone(),
            },
        );

        let cron_ctx = Arc::new(
            AppContext::build(&cron_mcp, memory.clone(), trade.clone(), social.clone()).await,
        );
        handles.push(tokio::spawn(async move {
            if let Err(e) = cron::start_scheduler(cron_ctx, cron_cfg).await {
                openatrading_core::log::error!("cron error: {e}");
            }
        }));
    }

    // ── Web ─────────────────────────────────────────
    if do_web {
        let web_bind = app_config.service.web_bind.clone();
        let web_ctx =
            AppContext::build(&web_mcp, memory.clone(), trade.clone(), social.clone()).await;
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

    // ── MCP Server ──────────────────────────────────
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

        let registry = std::sync::Arc::new(openatrading_core::llm::create_registry(
            openatrading_core::llm::RegistryOptions::market_only(),
        ));
        handles.push(tokio::spawn(async move {
            if let Err(e) = openatrading_core::llm::serve_http(registry, mcp_bind, mcp_rx).await {
                openatrading_core::log::error!("[gateway] mcp error: {e}");
            }
        }));
        openatrading_core::log::info!("mcp: enabled at http://{mcp_bind}/mcp");
    }

    // ── Telegram 交互 Bot（多轮会话 + /model 内联菜单） + 平台适配器 ──
    // bot_polling 启用时由 TelegramBot 接管 Telegram 轮询，
    // 同时从适配器列表移除 telegram，避免双 polling（409 冲突）。
    let mut adapters = openatrading_core::social::build_adapters(&app_config.social);
    if let Some(tg) = app_config.social.as_ref().and_then(|s| s.telegram.as_ref()) {
        let tg_ready = tg.enabled
            && tg.bot_polling
            && tg.bot_token.as_deref().is_some_and(|t| !t.is_empty())
            && tg.chat_id.as_deref().is_some_and(|c| !c.is_empty());
        if tg_ready {
            adapters.retain(|a| a.name() != "telegram");
            let bot_ctx =
                AppContext::build(&mcp_servers, memory.clone(), trade.clone(), social.clone())
                    .await;
            let bot = openatrading_core::social::TelegramBot::new(
                openatrading_core::social::TelegramBotConfig {
                    bot_token: tg.bot_token.clone().unwrap_or_default(),
                    chat_id: tg.chat_id.clone().unwrap_or_default(),
                    mcp_servers: mcp_servers.clone(),
                },
                roles.clone(),
                bot_ctx,
            );
            handles.push(tokio::spawn(async move {
                if let Err(e) = bot.start_polling().await {
                    openatrading_core::log::error!("telegram bot error: {e}");
                }
            }));
            openatrading_core::log::info!("telegram bot: enabled (interactive /model menu)");
        }
    }

    // ── 平台适配器（Discord 等） ────────────────
    if !adapters.is_empty() {
        let adapter_ctx = Arc::new(
            AppContext::build(&mcp_servers, memory.clone(), trade.clone(), social.clone()).await,
        );
        let (rx, adapter_refs, adapter_handles, _cancel) = launch_channels(adapters, 1024);
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

    // 启动通知
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
