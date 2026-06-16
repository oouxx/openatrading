pub mod app;
pub mod config;
pub mod doctor;
pub mod gateway_cmd;
pub mod mcp_cmd;
pub mod skills_cmd;
pub mod tools_cmd;
pub mod trade_cmd;
pub mod uninstall_cmd;
pub mod update;

use anyhow::Result;
use app::Cli;
use clap::Parser;
use std::sync::Arc;

use crate::cli::app::{Commands, GatewayAction, McpAction, SkillAction, ToolsAction, TradeAction};
use crate::cli::config::{cmd_init, AppConfig};
use crate::cli::doctor::cmd_doctor;
use crate::cli::gateway_cmd::{
    cmd_gateway_install, cmd_gateway_run, cmd_gateway_start, cmd_gateway_status, cmd_gateway_stop,
    cmd_gateway_uninstall,
};
use crate::cli::mcp_cmd::{cmd_mcp_call, cmd_mcp_list, cmd_mcp_serve, cmd_mcp_status};
use crate::cli::skills_cmd::{
    cmd_skills_install, cmd_skills_install_url, cmd_skills_list, cmd_skills_uninstall,
};
use crate::cli::tools_cmd::cmd_tools_doc;
use crate::cli::trade_cmd::{cmd_trade_add, cmd_trade_close, cmd_trade_list};
use crate::cli::uninstall_cmd::cmd_uninstall;
use crate::cli::update::{cmd_update, cmd_update_check};
use openatrading_core::context::AppContext;
use openatrading_core::cron::run_decision;
use openatrading_core::social::{NoopChannel, NotificationSender, StdoutChannel};
use openatrading_web::gateway::Gateway;

/// Initialize a file-based tracing subscriber for the `chat` TUI.
///
/// TUI 独占终端，任何写到 stdout/stderr 的日志都会破坏 alternate screen 渲染。
/// 这里把日志全部落盘到 `$ATRADING_HOME/logs/tui.log`，默认 level info，
/// 可被 `RUST_LOG` 覆盖。返回的 guard 必须保持在调用栈上以确保日志刷新。
fn init_chat_file_logger() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let log_dir = openatrading_core::atrading_home().join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "tui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .try_init();

    Ok(guard)
}

/// 用 `AppConfig` 装配 chat TUI 的依赖：LLM 客户端、工具集、system prompt。
///
/// 通过 `AppContext::build()` 统一构造 llm + api + tools。
async fn build_chat_args(app_config: &AppConfig) -> openatrading_tui::chat::ChatArgs {
    let db_path = openatrading_core::atrading_home().join(&app_config.service.db_path);
    let gateway = Gateway::init(&db_path).expect("Failed to open database");

    let ctx = AppContext::build(
        app_config.llm.provider.as_deref(),
        app_config.llm.model.as_deref(),
        &app_config.mcp_servers,
        gateway.memory.clone(),
        gateway.trade.clone(),
        Arc::new(NoopChannel),
    )
    .await;

    let system_prompt = openatrading_core::skills::SkillRegistry::build_chat_prompt(
        openatrading_core::prompts::DEFAULT_CHAT_SYSTEM_PROMPT,
    );

    openatrading_tui::chat::ChatArgs {
        llm: ctx.llm,
        tools: ctx.tools,
        system_prompt,
    }
}

pub async fn run() -> Result<()> {
    dotenvy::from_path(openatrading_core::atrading_home().join(".env")).ok();

    let cli = Cli::parse();

    // Handle --version
    if cli.version {
        println!("openatrading {}", env!("GIT_VERSION"));
        return Ok(());
    }

    // Handle `doctor` subcommand
    if let Some(Commands::Doctor { json }) = cli.command {
        return cmd_doctor(json).await;
    }

    // Handle `chat` subcommand
    if let Some(Commands::Chat) = cli.command {
        let _guard = init_chat_file_logger()?;
        let app_config = AppConfig::load()?;
        let chat_args = build_chat_args(&app_config).await;
        return openatrading_tui::chat::run_chat(chat_args).await;
    }

    // Handle `init` subcommand
    if let Some(Commands::Init) = cli.command {
        return cmd_init();
    }

    // Handle `uninstall` subcommand
    if let Some(Commands::Uninstall) = cli.command {
        return cmd_uninstall();
    }

    // Handle `gateway` subcommand
    if let Some(Commands::Gateway { action }) = cli.command {
        return match action {
            GatewayAction::Install => cmd_gateway_install(),
            GatewayAction::Uninstall => cmd_gateway_uninstall(),
            GatewayAction::Start => cmd_gateway_start(),
            GatewayAction::Stop => cmd_gateway_stop(),
            GatewayAction::Status => cmd_gateway_status(),
            GatewayAction::Run {
                cron_only,
                web_only,
                no_mcp,
            } => cmd_gateway_run(cron_only, web_only, no_mcp).await,
        };
    }

    // Handle `update` subcommand
    if let Some(Commands::Update { check }) = cli.command {
        if check {
            let (latest, current, needs_update) = cmd_update_check().await?;
            println!("当前版本: {current}");
            println!("最新版本: {latest}");
            if needs_update {
                println!("⚠ 有新版本可用，运行 `openatrading update` 更新");
            } else {
                println!("✅ 已是最新版本");
            }
        } else {
            cmd_update().await?;
        }
        return Ok(());
    }

    // Handle `trade` subcommand
    if let Some(Commands::Trade { action }) = cli.command {
        match action {
            TradeAction::Add {
                role,
                symbol,
                price,
                shares,
            } => {
                return cmd_trade_add(&role, &symbol, price, shares);
            },
            TradeAction::List { role } => {
                return cmd_trade_list(role.as_deref());
            },
            TradeAction::Close { id, price } => {
                return cmd_trade_close(id, price);
            },
        }
    }

    // Handle `skills` subcommand
    if let Some(Commands::Skills { action }) = cli.command {
        return match action {
            SkillAction::List => cmd_skills_list(),
            SkillAction::Install { name, source, url } => {
                if url {
                    cmd_skills_install_url(&name, &source).await
                } else {
                    cmd_skills_install(&name, &source)
                }
            },
            SkillAction::Uninstall { name } => cmd_skills_uninstall(&name),
        };
    }

    // Handle `tools` subcommand
    if let Some(Commands::Tools { action }) = cli.command {
        return match action {
            ToolsAction::Doc { endpoint } => cmd_tools_doc(&endpoint).await,
        };
    }

    // Handle `mcp` subcommand
    if let Some(Commands::Mcp { action }) = cli.command {
        return match action {
            McpAction::List { endpoint } => cmd_mcp_list(&endpoint).await,
            McpAction::Status { endpoint } => cmd_mcp_status(&endpoint).await,
            McpAction::Call {
                name,
                args,
                endpoint,
            } => cmd_mcp_call(&endpoint, &name, &args).await,
            McpAction::Serve { transport, bind } => cmd_mcp_serve(transport, &bind).await,
        };
    }

    // Load config
    let app_config = AppConfig::load()?;

    // Handle `decision` subcommand
    if let Some(Commands::Decision { dry_run, role }) = cli.command {
        return cmd_decision(&app_config, dry_run, role.as_deref()).await;
    }

    Ok(())
}

async fn cmd_decision(
    app_config: &AppConfig,
    dry_run: bool,
    role_filter: Option<&str>,
) -> Result<()> {
    let db_path = openatrading_core::atrading_home().join(&app_config.service.db_path);
    let gateway = Gateway::init(&db_path)?;

    let roles = crate::cli::gateway_cmd::build_roles(app_config);
    let filtered_roles: Vec<_> = if let Some(name) = role_filter {
        roles.into_iter().filter(|r| r.name == name).collect()
    } else {
        roles
    };

    if filtered_roles.is_empty() {
        if let Some(name) = role_filter {
            eprintln!("❌ 未找到角色: {name}");
        } else {
            eprintln!("❌ 没有可用角色，请先配置 [[roles]]");
        }
        return Ok(());
    }

    let social: Arc<dyn NotificationSender> = if dry_run {
        eprintln!("📋 dry-run 模式，仅输出到终端\n");
        Arc::new(StdoutChannel)
    } else if let Some(sc) = &app_config.social {
        crate::cli::gateway_cmd::create_notification_sender(sc)
    } else {
        eprintln!("⚠️ 未配置社交平台通知，仅输出到终端\n");
        Arc::new(NoopChannel)
    };

    let ctx = AppContext::build(
        app_config.llm.provider.as_deref(),
        app_config.llm.model.as_deref(),
        &app_config.mcp_servers,
        gateway.memory.clone(),
        gateway.trade.clone(),
        social,
    )
    .await;
    run_decision(&ctx, &filtered_roles).await?;

    if !dry_run {
        eprintln!("✅ 决策完成，通知已推送");
    }

    Ok(())
}
