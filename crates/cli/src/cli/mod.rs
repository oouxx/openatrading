pub mod app;
pub mod config;
pub mod config_cmd;
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
use crate::cli::config_cmd::cmd_config;
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
pub async fn run() -> Result<()> {
    dotenvy::from_path(openatrading_core::atrading_home().join(".env")).ok();

    let cli = Cli::parse();

    // Handle --version
    if cli.version {
        println!("openatrading {}", env!("GIT_VERSION"));
        return Ok(());
    }

    // Handle `doctor` subcommand
    if let Some(Commands::Doctor {
        json,
        fix,
        skip_network,
    }) = cli.command
    {
        return cmd_doctor(json, fix, skip_network).await;
    }

    // Handle `chat` subcommand — temporarily disabled (TUI under refactor)
    if matches!(cli.command, Some(Commands::Chat)) {
        tracing::info!("chat subcommand is disabled during refactoring");
        return Ok(());
    }

    // Handle `init` subcommand
    if matches!(cli.command, Some(Commands::Init)) {
        return cmd_init();
    }

    // Handle `uninstall` subcommand
    if matches!(cli.command, Some(Commands::Uninstall)) {
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
            }
            TradeAction::List { role } => {
                return cmd_trade_list(role.as_deref());
            }
            TradeAction::Close { id, price } => {
                return cmd_trade_close(id, price);
            }
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
            }
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

    // Handle `config` subcommand
    if let Some(Commands::Config { action }) = cli.command {
        return cmd_config(action);
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
