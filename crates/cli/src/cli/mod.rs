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

use crate::cli::app::{Commands, GatewayAction, McpAction, SkillAction, ToolsAction, TradeAction};
use crate::cli::config::cmd_init;
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

/// Entry point for the `openatrading` CLI.
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

    Ok(())
}
