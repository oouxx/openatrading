pub mod app;
pub mod config;
pub mod doctor;
pub mod gateway_cmd;
pub mod mcp_cmd;
pub mod skills_cmd;
pub mod tools_cmd;
pub mod trade_cmd;
pub mod update;

use anyhow::Result;
use app::Cli;
use clap::Parser;
use serde::Serialize;
use std::io::Write;
use std::sync::Arc;

use crate::cli::app::{Commands, GatewayAction, McpAction, SkillAction, ToolsAction, TradeAction};
use crate::cli::config::{cmd_init, AppConfig};
use crate::cli::doctor::cmd_doctor;
use crate::cli::gateway_cmd::{
    cmd_gateway_install, cmd_gateway_run, cmd_gateway_start, cmd_gateway_status, cmd_gateway_stop,
    cmd_gateway_uninstall,
};
use crate::cli::mcp_cmd::{cmd_mcp_call, cmd_mcp_list, cmd_mcp_status};
use crate::cli::skills_cmd::{cmd_skills_install, cmd_skills_list, cmd_skills_uninstall};
use crate::cli::tools_cmd::cmd_tools_doc;
use crate::cli::trade_cmd::{cmd_trade_add, cmd_trade_close, cmd_trade_list};
use crate::cli::update::{cmd_update, cmd_update_check};
use openatrading_core::agents::sentiment::analyze_sentiment;
use openatrading_core::api::xuangubao::XuanguBaoClient;
use openatrading_core::api::Client;
use openatrading_core::cron::run_decision;
use openatrading_core::llm::LlmClient;
use openatrading_core::social::{NoopSender, NotificationSender, StdoutSender};
use openatrading_web::gateway::Gateway;

#[derive(Serialize)]
struct OutputData {
    #[serde(skip_serializing_if = "Option::is_none")]
    overview: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_up: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_down: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    super_stock: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field_item: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plate_abnormal: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    community: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sentiment: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cls_telegraph: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dapan_abnormal: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    market_trend: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_analysis: Option<String>,
}

const ALL_DATA_SOURCES: &[&str] = &[
    "overview",
    "limit_up",
    "limit_down",
    "super_stock",
    "field_item",
    "plate_abnormal",
    "timeline",
    "community",
    "sentiment",
    "cls_telegraph",
    "dapan_abnormal",
    "market_trend",
];

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
        return openatrading_tui::chat::run_chat().await;
    }

    // Handle `init` subcommand
    if let Some(Commands::Init) = cli.command {
        return cmd_init();
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
            } => cmd_gateway_run(cron_only, web_only).await,
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
            SkillAction::Install { name } => cmd_skills_install(&name),
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
        };
    }

    // Load config
    let app_config = AppConfig::load()?;

    // Handle `decision` subcommand
    if let Some(Commands::Decision { dry_run, role }) = cli.command {
        return cmd_decision(&app_config, dry_run, role.as_deref()).await;
    }

    // Handle --list-roles
    if cli.list_roles {
        app_config.list_roles();
        return Ok(());
    }

    let sources: Vec<String> = cli
        .data
        .unwrap_or_else(|| ALL_DATA_SOURCES.iter().map(|s| s.to_string()).collect());

    let client = Client::new();
    let xgb = XuanguBaoClient::new();
    let mut output = OutputData {
        overview: None,
        limit_up: None,
        limit_down: None,
        super_stock: None,
        field_item: None,
        plate_abnormal: None,
        timeline: None,
        community: None,
        sentiment: None,
        cls_telegraph: None,
        dapan_abnormal: None,
        market_trend: None,
        role_name: None,
        ai_analysis: None,
    };

    // ── Fetch requested data sources ────────────────────

    for src in &sources {
        match src.as_str() {
            "overview" => {
                if let Ok(data) = xgb.fetch_market_overview().await {
                    output.overview = Some(serde_json::to_value(&data)?);
                }
            },
            "limit_up" => {
                if let Ok(data) = xgb.fetch_limit_up().await {
                    output.limit_up = Some(serde_json::to_value(&data)?);
                }
            },
            "limit_down" => {
                if let Ok(data) = xgb.fetch_limit_down().await {
                    output.limit_down = Some(serde_json::to_value(&data)?);
                }
            },
            "super_stock" => {
                if let Ok(data) = xgb.fetch_super_stock().await {
                    output.super_stock = Some(serde_json::to_value(&data)?);
                }
            },
            "field_item" => {
                let date = openatrading_core::utils::time::beijing_now()
                    .format("%Y-%m-%d")
                    .to_string();
                if let Ok(data) = client.jiuyan.fetch_field(&date, 1).await {
                    output.field_item = Some(serde_json::to_value(&data)?);
                }
            },
            "plate_abnormal" => {
                if let Ok(data) = xgb.fetch_plate_abnormal(30).await {
                    output.plate_abnormal = Some(serde_json::to_value(&data)?);
                }
            },
            "timeline" => {
                let month = openatrading_core::utils::time::beijing_now()
                    .format("%Y-%m")
                    .to_string();
                if let Ok(data) = client.jiuyan.fetch_timeline(&month).await {
                    output.timeline = Some(serde_json::to_value(&data)?);
                }
            },
            "community" => {
                if let Ok(data) = client.jiuyan.fetch_community("", 1, 1, 20).await {
                    output.community = Some(serde_json::to_value(&data)?);
                }
            },
            "sentiment" => {
                if let Ok(data) = analyze_sentiment(&client).await {
                    output.sentiment = Some(serde_json::to_value(&data)?);
                }
            },
            "cls_telegraph" => {
                let cat = cli.cls_category.as_deref().unwrap_or("");
                if let Ok(data) = client.cls.fetch_telegraph(cat, None, 20).await {
                    output.cls_telegraph = Some(serde_json::to_value(&data)?);
                }
            },
            "dapan_abnormal" => {
                if let Ok(data) = client.xuangubao.fetch_dapan_abnormal().await {
                    output.dapan_abnormal = Some(serde_json::to_value(&data)?);
                }
            },
            "market_trend" => {
                // Default to SH index
                if let Ok(data) = client.xuangubao.fetch_market_trend("000001").await {
                    output.market_trend = Some(serde_json::to_value(&data)?);
                }
            },
            _ => eprintln!("⚠️ 未知数据源: {src}"),
        }
    }

    // ── Output ──────────────────────────────────────────

    let content = if cli.json {
        serde_json::to_string_pretty(&output)?
    } else if cli.html {
        let json = serde_json::to_string_pretty(&output)?;
        format!(
            "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"UTF-8\">\n<title>OpenATrading</title>\n<style>\nbody{{background:#0d0d1a;color:#e8e8ed;font:14px monospace;padding:24px}}\n</style>\n</head>\n<body>\n<pre>{}</pre>\n</body>\n</html>",
            json.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        )
    } else {
        // Markdown: simple key-value dump
        let mut md = String::from("# OpenATrading Report\n\n");
        let fields: Vec<(&str, &Option<serde_json::Value>)> = vec![
            ("概览", &output.overview),
            ("涨停数据", &output.limit_up),
            ("跌停数据", &output.limit_down),
            ("强势股", &output.super_stock),
            ("韭研异动", &output.field_item),
            ("板块异动", &output.plate_abnormal),
            ("时间线", &output.timeline),
            ("社区文章", &output.community),
            ("舆情分析", &output.sentiment),
            ("财联社电报", &output.cls_telegraph),
            ("大盘异动", &output.dapan_abnormal),
            ("市场趋势", &output.market_trend),
        ];
        for (label, val) in &fields {
            if let Some(v) = val {
                md.push_str(&format!(
                    "## {}\n\n```json\n{}\n```\n\n",
                    label,
                    serde_json::to_string_pretty(v)?
                ));
            }
        }
        if let Some(ai) = &output.ai_analysis {
            md.push_str("## AI 分析\n\n");
            md.push_str(ai);
            md.push('\n');
        }
        md
    };

    if let Some(path) = &cli.output {
        std::fs::write(path, content)?;
        eprintln!("✅ 已写入: {path}");
    } else {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(content.as_bytes())?;
        // Add trailing newline if not present
        if !content.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
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
        Arc::new(StdoutSender)
    } else if let Some(sc) = &app_config.social {
        Arc::from(crate::cli::gateway_cmd::create_notification_sender(sc))
    } else {
        eprintln!("⚠️ 未配置社交平台通知，仅输出到终端\n");
        Arc::new(NoopSender)
    };

    let llm = LlmClient::from_config(
        app_config.llm.provider.as_deref(),
        app_config.llm.model.as_deref(),
    );
    let api = Client::new();

    let tool_cfg = openatrading_core::llm::ToolConfig {
        mcp_servers: app_config.mcp_servers.clone(),
    };
    let tools: Vec<Arc<openatrading_core::llm::DynTool>> =
        openatrading_core::llm::build_tools(&tool_cfg)
            .await
            .all_tools()
            .to_vec();

    run_decision(
        gateway.memory.clone(),
        gateway.trade.clone(),
        &*social,
        &llm,
        &api,
        &filtered_roles,
        &tools,
    )
    .await?;

    if !dry_run {
        eprintln!("✅ 决策完成，通知已推送");
    }

    Ok(())
}
