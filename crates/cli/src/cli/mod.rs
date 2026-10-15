pub mod app;
pub mod channels_cmd;
pub mod config;
pub mod config_cmd;
pub mod doctor;
pub mod gateway_runner;
pub mod mcp_cmd;
pub mod service_cmd;
pub mod skills_cmd;
pub mod tools_cmd;
pub mod uninstall_cmd;
pub mod update;

use anyhow::Result;
use app::Cli;
use clap::Parser;

use crate::cli::app::{
    ChannelsAction, Commands, GatewayAction, McpAction, SkillAction, ToolsAction,
};
use crate::cli::channels_cmd::cmd_channels_list;
use crate::cli::config::cmd_init;
use crate::cli::config_cmd::cmd_config;
use crate::cli::doctor::cmd_doctor;
use crate::cli::gateway_runner::cmd_gateway_run;
use crate::cli::mcp_cmd::{cmd_mcp_call, cmd_mcp_list, cmd_mcp_serve, cmd_mcp_status};
use crate::cli::service_cmd::{
    cmd_gateway_install, cmd_gateway_start, cmd_gateway_status, cmd_gateway_stop,
    cmd_gateway_uninstall,
};
use crate::cli::skills_cmd::{
    cmd_skills_install, cmd_skills_install_url, cmd_skills_list, cmd_skills_uninstall,
};
use crate::cli::tools_cmd::cmd_tools_doc;
use crate::cli::uninstall_cmd::cmd_uninstall;
use crate::cli::update::{cmd_update, cmd_update_check};

/// 进程级前置准备：加载 `.env`，并把品牌根绑定为 pi-rs 的家。
///
/// **必须由 `main` 在 async runtime 起来之前调用**：`.env` 与 `PI_RS_HOME` 都是进程
/// 环境变量的写入，要在单线程阶段做完（详见 `init::bind_pi_rs_home`）。
pub fn preflight() {
    // Layer 2: 全局 .env
    dotenvy::from_path(openatrading_core::atrading_home().join(".env")).ok();

    // Layer 3: 项目 .env（覆盖全局）
    if let Ok(cwd) = std::env::current_dir() {
        let project_env = cwd.join(".atrading").join(".env");
        if project_env.exists() {
            dotenvy::from_path(&project_env).ok();
        }
    }

    // 品牌根即 pi-rs 的家：agent 目录落在 `{品牌根}/agent`（可用 PI_RS_HOME /
    // PI_CODING_AGENT_DIR 覆盖，见该函数文档）。
    openatrading_core::init::bind_pi_rs_home();
}

/// Entry point for the `openatrading` CLI.
///
/// 前置准备见 [`preflight`]（由 `main` 在 runtime 之前调用）。
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Doctor {
            json,
            fix,
            skip_network,
        }) => cmd_doctor(json, fix, skip_network).await,

        Some(Commands::Init) => cmd_init(),

        Some(Commands::Uninstall) => cmd_uninstall(),

        Some(Commands::Gateway { action }) => match action {
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
        },

        Some(Commands::Update { check }) => {
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
            Ok(())
        }

        Some(Commands::Skills { action }) => match action {
            SkillAction::List => cmd_skills_list(),
            SkillAction::Install { name, source, url } => {
                if url {
                    cmd_skills_install_url(&name, &source).await
                } else {
                    cmd_skills_install(&name, &source)
                }
            }
            SkillAction::Uninstall { name } => cmd_skills_uninstall(&name),
        },

        Some(Commands::Tools { action }) => match action {
            ToolsAction::Doc { endpoint } => cmd_tools_doc(&endpoint).await,
        },

        Some(Commands::Mcp { action }) => match action {
            McpAction::List { endpoint } => cmd_mcp_list(&endpoint).await,
            McpAction::Status { endpoint } => cmd_mcp_status(&endpoint).await,
            McpAction::Call {
                name,
                args,
                endpoint,
            } => cmd_mcp_call(&endpoint, &name, &args).await,
            McpAction::Serve { transport, bind } => cmd_mcp_serve(transport, &bind).await,
        },

        Some(Commands::Config { action }) => cmd_config(action),

        Some(Commands::Channels { action }) => match action {
            ChannelsAction::List => cmd_channels_list(),
        },

        None => Ok(()),
    }
}
