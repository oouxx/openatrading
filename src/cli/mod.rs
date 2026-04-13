pub mod app;

use anyhow::Result;
use app::{Cli, Commands};
use clap::Parser;

use crate::api::Client;
use crate::llm::openrouter::LlmClient;
use crate::report::report::{Report, Reporter, RoleAnalysis};
use crate::roles::analyst::{build_prompt, Role};
use crate::web::{start_web_server, WebState};

pub async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let api_key = std::env::var("OPENROUTER_API_KEY").ok();
    let client = Client::new();

    match cli.command {
        Some(Commands::LimitUp(opts)) => {
            run_limit_up(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::LimitDown(opts)) => {
            run_limit_down(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::Overview(opts)) => {
            run_overview(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::PlateAbnormal(opts)) => {
            run_plate_abnormal(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::Field(opts)) => {
            run_field(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::Timeline(opts)) => {
            run_timeline(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::Community(opts)) => {
            run_community(client, &opts.output, &opts.format, api_key.as_ref()).await?
        }
        Some(Commands::Run(opts)) => run_all(client, opts, api_key).await?,
        Some(Commands::Agent(opts)) => run_interactive(opts, api_key).await?,
        None => {
            run_all(
                client,
                app::RunOpts {
                    ai: false,
                    format: "markdown".to_string(),
                    output: "./reports".to_string(),
                    web: false,
                    port: 8080,
                },
                api_key,
            )
            .await?
        }
    }

    Ok(())
}

async fn run_limit_up(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取涨停数据...");
    let (limit_up, overview) = tokio::join!(
        client.xuangubao.fetch_limit_up(),
        client.xuangubao.fetch_market_overview(),
    );
    let limit_up = limit_up?;
    let overview = overview?;
    println!("✅ 涨停: {} 只", limit_up.len());

    let report = Report::new(
        overview.clone(),
        limit_up,
        vec![],
        vec![],
        vec![],
        vec![ra()],
    );
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_limit_down(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取跌停数据...");
    let (limit_down, overview) = tokio::join!(
        client.xuangubao.fetch_limit_down(),
        client.xuangubao.fetch_market_overview(),
    );
    let limit_down = limit_down?;
    let overview = overview?;
    println!("✅ 跌停: {} 只", limit_down.len());

    let report = Report::new(overview.clone(), vec![], vec![], vec![], vec![], vec![ra()]);
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_overview(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取市场概览...");
    let overview = client.xuangubao.fetch_market_overview().await?;
    println!(
        "✅ 市场概览: 上涨 {} / 下跌 {} / 涨停 {} / 跌停 {} / 炸板率 {:.1}%",
        overview.rise_count,
        overview.fall_count,
        overview.limit_up_count,
        overview.limit_down_count,
        overview.bomb_rate * 100.0
    );

    let report = Report::new(overview.clone(), vec![], vec![], vec![], vec![], vec![ra()]);
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_plate_abnormal(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取板块异动...");

    let (plate_abnormal, overview) = tokio::join!(
        client.xuangubao.fetch_plate_abnormal(30),
        client.xuangubao.fetch_market_overview(),
    );

    let plate_abnormal = plate_abnormal?;
    let overview = overview?;
    println!("✅ 板块异动: {} 个板块", plate_abnormal.len());

    let report = Report::new(
        overview.clone(),
        vec![],
        plate_abnormal,
        vec![],
        vec![],
        vec![ra()],
    );
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_field(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    println!("📡 拉取异动数据...");

    let (field_result, overview) = tokio::join!(
        client.jiuyan.fetch_field(&today, 1),
        client.xuangubao.fetch_market_overview(),
    );

    let field_items = field_result?;
    let overview = overview?;
    let total: usize = field_items.iter().map(|p| p.list.len()).sum();
    println!(
        "✅ 异动数据: {} 个板块，{} 只股票",
        field_items.len(),
        total
    );

    let report = Report::new(
        overview.clone(),
        vec![],
        vec![],
        field_items,
        vec![],
        vec![ra()],
    );
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_timeline(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    let this_month = chrono::Local::now().format("%Y-%m").to_string();
    println!("📡 拉取时间线数据...");

    let (timeline_result, overview) = tokio::join!(
        client.jiuyan.fetch_timeline(&this_month),
        client.xuangubao.fetch_market_overview(),
    );

    let timeline = timeline_result?;
    let overview = overview?;
    println!("✅ 时间线数据: {} 天", timeline.len());

    let report = Report::new(
        overview.clone(),
        vec![],
        vec![],
        vec![],
        timeline,
        vec![ra()],
    );
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

async fn run_community(
    client: Client,
    output: &str,
    format: &str,
    api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取社群文章...");

    let (community_result, overview) = tokio::join!(
        client.jiuyan.fetch_community("", 0, 1, 30),
        client.xuangubao.fetch_market_overview(),
    );

    let community = community_result?;
    let overview = overview?;
    println!("✅ 社群文章: {} 篇", community.result.len());

    let report = Report::new(overview.clone(), vec![], vec![], vec![], vec![], vec![ra()]);
    save_report_formatted(&report, output, format)?;

    if let Some(key) = api_key {
        run_ai(key.to_string(), &overview, &vec![]).await;
    }

    Ok(())
}

use app::RunOpts;

async fn run_interactive(opts: app::CommonOpts, _api_key: Option<String>) -> Result<()> {
    use rustyline::error::ReadlineError;
    use rustyline::Editor;

    let api_key = std::env::var("OPENROUTER_API_KEY").ok();
    let output = opts.output.clone();
    let format = opts.format.clone();

    let mut rl: Editor<(), rustyline::history::FileHistory> = Editor::new().unwrap();

    println!("🤖 进入交互模式（输入 help 查看命令）...");
    println!("可用命令: limit-up, limit-down, overview, plate, field, timeline, community, ai, help, exit");
    println!("提示: 支持 Tab 补全\n");

    let commands = [
        "limit-up",
        "limit-down",
        "overview",
        "plate",
        "field",
        "timeline",
        "community",
        "ai",
        "help",
        "exit",
        "quit",
    ];
    let hints = [
        " - 获取涨停数据",
        " - 获取跌停数据",
        " - 获取市场概览",
        " - 获取板块异动",
        " - 获取异动数据",
        " - 获取时间线",
        " - 获取社群文章",
        " - AI 分析",
        " - 查看帮助",
        " - 退出交互模式",
        " - 退出交互模式",
    ];

    loop {
        let readline = rl.readline("> ");
        let input: String = match readline {
            Ok(s) => s.trim().to_string(),
            Err(ReadlineError::Interrupted) => {
                println!("\n👋 退出交互模式");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("👋 退出交互模式");
                break;
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                break;
            }
        };

        if input.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(&input);

        if input == "exit" || input == "quit" {
            println!("👋 退出交互模式");
            break;
        }

        if input == "help" {
            for (cmd, hint) in commands.iter().zip(hints.iter()) {
                println!("  {} {}", cmd, hint);
            }
            continue;
        }

        let cmd = if input.starts_with('/') {
            input.trim_start_matches('/').to_string()
        } else {
            input
        };

        let client = Client::new();

        match cmd.as_str() {
            "limit-up" => run_limit_up(client, &output, &format, api_key.as_ref()).await?,
            "limit-down" => run_limit_down(client, &output, &format, api_key.as_ref()).await?,
            "overview" => run_overview(client, &output, &format, api_key.as_ref()).await?,
            "plate" => run_plate_abnormal(client, &output, &format, api_key.as_ref()).await?,
            "field" => run_field(client, &output, &format, api_key.as_ref()).await?,
            "timeline" => run_timeline(client, &output, &format, api_key.as_ref()).await?,
            "community" => run_community(client, &output, &format, api_key.as_ref()).await?,
            "ai" => {
                let roles = [Role::Trader, Role::Quant, Role::RiskOfficer];
                if let Some(key) = api_key.as_ref() {
                    let client = Client::new();
                    let overview = client.xuangubao.fetch_market_overview().await?;
                    let limit_up = client.xuangubao.fetch_limit_up().await.unwrap_or_default();
                    let results = futures::future::join_all(roles.iter().map(|&role| {
                        let llm = LlmClient::new(key.clone());
                        let prompt = build_prompt(role, &overview, &limit_up);
                        async move { (role, llm.chat(&prompt).await) }
                    }))
                    .await;
                    for (role, result) in results {
                        match result {
                            Ok(text) => println!("【{}】\n{}\n", role.name(), text),
                            Err(e) => eprintln!("【{}】分析失败: {}", role.name(), e),
                        }
                    }
                } else {
                    println!("⚠️ 未配置 OPENROUTER_API_KEY，无法使用 AI 分析");
                }
            }
            _ => {
                println!("❌ 未知命令: {}（输入 help 查看可用命令）", cmd);
            }
        }
    }
    Ok(())
}

async fn run_all(client: Client, opts: RunOpts, api_key: Option<String>) -> Result<()> {
    println!("📡 拉取选股宝数据...");

    let (limit_up, limit_down, overview) = tokio::join!(
        client.xuangubao.fetch_limit_up(),
        client.xuangubao.fetch_limit_down(),
        client.xuangubao.fetch_market_overview(),
    );

    let limit_up = limit_up?;
    let _ = limit_down?;
    let overview = overview?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let this_month = chrono::Local::now().format("%Y-%m").to_string();

    let (field_items, timeline) = if std::env::var("JIUYAN_PHONE").is_ok() {
        println!("📡 拉取韭研公社数据...");
        let (field_result, timeline_result) = tokio::join!(
            client.jiuyan.fetch_field(&today, 1),
            client.jiuyan.fetch_timeline(&this_month),
        );

        let field_items = field_result
            .inspect(|d| {
                println!(
                    "✅ 异动数据: {} 个板块，{} 只股票",
                    d.len(),
                    d.iter().map(|p| p.list.len()).sum::<usize>()
                );
            })
            .unwrap_or_else(|e| {
                eprintln!("⚠️ 异动数据拉取失败: {}", e);
                vec![]
            });

        let timeline = timeline_result
            .inspect(|d| {
                println!("✅ 时间线数据: {} 天", d.len());
            })
            .unwrap_or_else(|e| {
                eprintln!("⚠️ 时间线拉取失败: {}", e);
                vec![]
            });

        (field_items, timeline)
    } else {
        println!("⚠️ 未配置 JIUYAN_PHONE，跳过韭研公社数据");
        (vec![], vec![])
    };

    let report = Report::new(
        overview.clone(),
        limit_up.clone(),
        vec![],
        field_items,
        timeline,
        vec![ra()],
    );

    // 保存报告
    let path = save_report_formatted(&report, &opts.output, &opts.format)?;
    println!("✅ 报告已保存: {}", path);

    // 启动 Web UI
    if opts.web {
        let state = WebState::new();
        state.set_report(report).await;
        start_web_server(opts.port, state).await;
    }

    if opts.ai {
        if let Some(key) = api_key {
            run_ai(key, &overview, &limit_up).await;
        } else {
            eprintln!("⚠️ 未配置 OPENROUTER_API_KEY");
        }
    }

    Ok(())
}

async fn run_ai(
    api_key: String,
    overview: &crate::api::xuangubao::MarketOverview,
    limit_up: &Vec<crate::api::xuangubao::Stock>,
) {
    let roles = [Role::Trader, Role::Quant, Role::RiskOfficer];

    let results = futures::future::join_all(roles.iter().map(|&role| {
        let llm = LlmClient::new(api_key.clone());
        let prompt = build_prompt(role, overview, limit_up);
        async move { (role, llm.chat(&prompt).await) }
    }))
    .await;

    println!("═══════════════════════════════════\n");
    for (role, result) in results {
        match result {
            Ok(text) => {
                println!(
                    "【{}】\n{}\n───────────────────────────────────\n",
                    role.name(),
                    text
                );
            }
            Err(e) => eprintln!("【{}】分析失败: {}", role.name(), e),
        }
    }
}

fn empty_overview() -> crate::api::xuangubao::MarketOverview {
    crate::api::xuangubao::MarketOverview {
        limit_up_count: 0,
        limit_down_count: 0,
        rise_count: 0,
        fall_count: 0,
        bomb_rate: 0.0,
    }
}

fn ra() -> RoleAnalysis {
    RoleAnalysis {
        role: Role::Trader,
        content: "".to_string(),
        elapsed_ms: 0,
    }
}

fn save_report_formatted(report: &Report, output: &str, format: &str) -> Result<String> {
    let reporter = Reporter::new(report.clone());

    match format {
        "markdown" => reporter.save_markdown(output),
        "html" => reporter.save_html(output),
        "both" => {
            let md_path = reporter.save_markdown(output)?;
            let html_path = reporter.save_html(output)?;
            Ok(format!("{}\n{}", md_path, html_path))
        }
        _ => reporter.save_markdown(output),
    }
}
