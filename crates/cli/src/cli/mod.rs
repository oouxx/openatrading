pub mod app;
pub mod config;

use anyhow::Result;
use app::Cli;
use clap::Parser;
use serde::Serialize;
use std::io::Write;

use crate::cli::app::Commands;
use crate::cli::config::{cmd_init, AppConfig};
use trending_agent_core::agents::sentiment::analyze_sentiment;
use trending_agent_core::api::xuangubao::XuanguBaoClient;
use trending_agent_core::api::Client;
use trending_agent_core::llm::openrouter::LlmClient;
use trending_agent_core::roles::prompt::build_prompt;

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
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    // Handle `init` subcommand
    if let Some(Commands::Init) = cli.command {
        return cmd_init();
    }

    // Load config
    let project_root = std::env::current_dir()?;
    let app_config = AppConfig::load(&project_root)?;

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
            }
            "limit_up" => {
                if let Ok(data) = xgb.fetch_limit_up().await {
                    output.limit_up = Some(serde_json::to_value(&data)?);
                }
            }
            "limit_down" => {
                if let Ok(data) = xgb.fetch_limit_down().await {
                    output.limit_down = Some(serde_json::to_value(&data)?);
                }
            }
            "super_stock" => {
                if let Ok(data) = xgb.fetch_super_stock().await {
                    output.super_stock = Some(serde_json::to_value(&data)?);
                }
            }
            "field_item" => {
                let date = chrono::Local::now().format("%Y-%m-%d").to_string();
                if let Ok(data) = client.jiuyan.fetch_field(&date, 1).await {
                    output.field_item = Some(serde_json::to_value(&data)?);
                }
            }
            "plate_abnormal" => {
                if let Ok(data) = xgb.fetch_plate_abnormal(30).await {
                    output.plate_abnormal = Some(serde_json::to_value(&data)?);
                }
            }
            "timeline" => {
                let month = chrono::Local::now().format("%Y-%m").to_string();
                if let Ok(data) = client.jiuyan.fetch_timeline(&month).await {
                    output.timeline = Some(serde_json::to_value(&data)?);
                }
            }
            "community" => {
                if let Ok(data) = client.jiuyan.fetch_community("", 1, 1, 20).await {
                    output.community = Some(serde_json::to_value(&data)?);
                }
            }
            "sentiment" => {
                if let Ok(data) = analyze_sentiment(&client).await {
                    output.sentiment = Some(serde_json::to_value(&data)?);
                }
            }
            "cls_telegraph" => {
                let cat = cli.cls_category.as_deref().unwrap_or("");
                if let Ok(data) = client.cls.fetch_telegraph(cat).await {
                    output.cls_telegraph = Some(serde_json::to_value(&data)?);
                }
            }
            "dapan_abnormal" => {
                if let Ok(data) = client.xuangubao.fetch_dapan_abnormal().await {
                    output.dapan_abnormal = Some(serde_json::to_value(&data)?);
                }
            }
            "market_trend" => {
                // Default to SH index
                if let Ok(data) = client.xuangubao.fetch_market_trend("000001").await {
                    output.market_trend = Some(serde_json::to_value(&data)?);
                }
            }
            _ => eprintln!("⚠️ 未知数据源: {src}"),
        }
    }

    // ── AI Analysis ─────────────────────────────────────

    if cli.ai {
        let role_name = cli
            .role
            .or_else(|| app_config.default_role.clone())
            .unwrap_or_else(|| "zarili".to_string());

        let strategy_text = match app_config.find_role(&role_name) {
            Some(entry) => {
                output.role_name = Some(entry.config.display_name.clone());
                AppConfig::load_strategy(entry)?
            }
            None => {
                eprintln!("⚠️ 未找到角色 '{role_name}'，使用默认策略");
                output.role_name = Some("默认分析".to_string());
                trending_agent_core::roles::get_builtin_strategy("zarili")
                    .unwrap_or("")
                    .to_string()
            }
        };

        let api_key = std::env::var("LLM_API_KEY")
            .or_else(|_| std::env::var("DEEPSEEK_API_KEY"))
            .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
            .ok();

        if let Some(key) = api_key {
            let llm = LlmClient::new(key);
            let market_data_json = serde_json::to_string(&output)?;
            let display = output.role_name.as_deref().unwrap_or("AI助手");
            let prompt = build_prompt(&strategy_text, &market_data_json, display);

            let messages = serde_json::json!([
                {"role": "system", "content": prompt.system},
                {"role": "user", "content": prompt.user}
            ])
            .to_string();

            if let Ok(analysis) = llm.chat_with_messages(&messages).await {
                output.ai_analysis = Some(analysis);
            }
        } else {
            eprintln!("⚠️ 未配置 LLM_API_KEY，跳过 AI 分析");
        }
    }

    // ── Output ──────────────────────────────────────────

    let content = if cli.json {
        serde_json::to_string_pretty(&output)?
    } else if cli.html {
        let json = serde_json::to_string_pretty(&output)?;
        format!(
            "<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"UTF-8\">\n<title>Trending Agent</title>\n<style>\nbody{{background:#0d0d1a;color:#e8e8ed;font:14px monospace;padding:24px}}\n</style>\n</head>\n<body>\n<pre>{}</pre>\n</body>\n</html>",
            json.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        )
    } else {
        // Markdown: simple key-value dump
        let mut md = String::from("# Trending Agent Report\n\n");
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
