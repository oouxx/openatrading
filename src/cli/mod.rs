pub mod app;

use anyhow::Result;
use app::{Cli, Commands};
use clap::Parser;

use crate::api::Client;
use crate::llm::openrouter::LlmClient;
use crate::report::report::{Report, Reporter, RoleAnalysis};
use crate::roles::analyst::{build_prompt, Role};
use crate::web::{start_web_server, WebState};
use futures::StreamExt;

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
        Some(Commands::Sentiment(opts)) => run_sentiment(client, opts, api_key).await?,
        Some(Commands::Technical(opts)) => run_technical(client, opts, api_key).await?,
        Some(Commands::SaveUser(opts)) => run_save_user(client, opts).await?,
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
        None,
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

    let report = Report::new(
        overview.clone(),
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ra()],
        None,
    );
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

    let report = Report::new(
        overview.clone(),
        vec![],
        vec![],
        vec![],
        vec![],
        vec![ra()],
        None,
    );
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
        None,
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
        None,
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
        None,
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
    _api_key: Option<&String>,
) -> Result<()> {
    println!("📡 拉取社群文章...");

    let community_data = client.jiuyan.fetch_community("", 0, 1, 50).await?;
    let articles = community_data.result;
    println!("✅ 获取到 {} 篇文章", articles.len());

    if articles.is_empty() {
        println!("⚠️ 未获取到文章");
        return Ok(());
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let output_dir = std::path::Path::new(output)
        .join("jiuyan")
        .join("community")
        .join(&today);
    std::fs::create_dir_all(&output_dir)?;
    println!("📁 保存目录: {}", output_dir.display());

    let mut index = format!("# 社群文章\n\n共 {} 篇\n\n## 列表\n\n", articles.len());

    for (i, article) in articles.iter().enumerate() {
        println!(
            "📝 获取第 {}/{} 篇: {}",
            i + 1,
            articles.len(),
            article.title
        );

        let detail = match client
            .jiuyan
            .fetch_article_detail(&article.article_id)
            .await
        {
            Ok(Some(content)) => content,
            Ok(None) => {
                eprintln!("  文章 {} 无正文", article.article_id);
                continue;
            }
            Err(e) => {
                eprintln!("  获取文章详情失败: {}", e);
                continue;
            }
        };

        let safe_title: String = article
            .title
            .chars()
            .filter(|c| {
                c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_' || *c == '(' || *c == ')'
            })
            .take(50)
            .collect();

        if safe_title.is_empty() {
            continue;
        }

        let filename_base = format!("{}_{:03}", safe_title, i + 1);

        if format == "markdown" || format == "both" {
            let md_path = output_dir.join(format!("{}.md", filename_base));
            let md_content = format!(
                "# {}\n\n**作者:** {} | **时间:** {} | **点赞:** {} | **评论:** {} | **收藏:** {} | **转发:** {}\n\n{}\n",
                article.title,
                article.user.nickname,
                article.create_time,
                article.like_count,
                article.comment_count,
                article.collect_count,
                article.forward_count,
                detail
            );
            std::fs::write(&md_path, md_content)?;
            println!("  ✅ 已保存: {}", md_path.display());
        }

        if format == "html" || format == "both" {
            let html_path = output_dir.join(format!("{}.html", filename_base));
            let meta = format!(
                "作者: {} | 时间: {} | 点赞: {} | 评论: {} | 收藏: {} | 转发: {}",
                article.user.nickname,
                article.create_time,
                article.like_count,
                article.comment_count,
                article.collect_count,
                article.forward_count
            );
            let html_content = markdown_to_html(&article.title, &meta, &detail);
            std::fs::write(&html_path, html_content)?;
            println!("  ✅ 已保存: {}", html_path.display());
        }

        index.push_str(&format!(
            "{}. [{}]({})\n",
            i + 1,
            article.title,
            filename_base
        ));
    }

    let index_path = output_dir.join("index.md");
    std::fs::write(&index_path, &index)?;
    println!("✅ 索引文件已保存: {}", index_path.display());

    Ok(())
}

use app::RunOpts;

async fn run_sentiment_cmd(client: &Client) -> Result<()> {
    use crate::agents::sentiment::analyze_sentiment;

    println!("📊 拉取舆情数据...");
    let result = analyze_sentiment(client).await?;
    println!("✅ 舆情分析完成");
    println!("\n📊 热度趋势: {}", result.trend);
    println!("📊 热门股票:");
    for (code, name, cnt) in result.hot_stocks.iter().take(5) {
        println!("  {} {} - {}次", code, name, cnt);
    }
    println!("📊 热门文章:");
    for (title, eng) in result.hot_articles.iter().take(3) {
        println!("  {} (热度:{})", title, eng);
    }
    println!("\n📊 总结: {}", result.summary);
    Ok(())
}

async fn run_technical_cmd(client: &Client) -> Result<()> {
    use crate::agents::technical::analyze_technical;

    println!("📈 拉取技术数据...");
    let result = analyze_technical(client).await?;
    println!("✅ 技术分析完成");
    println!("\n📈 连板梯队:");
    for level in result.levels.iter().take(3) {
        println!("  {}板: {}只", level.days, level.stocks.len());
    }
    println!("📈 热门板块:");
    for (name, cnt) in result.hot_sectors.iter().take(3) {
        println!("  {} - {}只", name, cnt);
    }
    if !result.risk_signals.is_empty() {
        println!("📈 风险信号:");
        for sig in &result.risk_signals {
            println!("  ⚠️ {}", sig);
        }
    }
    println!("\n📈 总结: {}", result.summary);
    Ok(())
}

async fn run_sentiment(
    client: Client,
    _opts: app::CommonOpts,
    _api_key: Option<String>,
) -> Result<()> {
    run_sentiment_cmd(&client).await
}

async fn run_technical(
    client: Client,
    _opts: app::CommonOpts,
    _api_key: Option<String>,
) -> Result<()> {
    run_technical_cmd(&client).await
}

async fn run_save_user(client: Client, opts: app::UserOpts) -> Result<()> {
    use crate::api::taoguba::save_user_posts;

    save_user_posts(&client.taoguba, &opts.user_id, &opts.output).await?;
    Ok(())
}

async fn run_ai_all(client: &Client, api_key: &Option<String>) -> Result<()> {
    if let Some(key) = api_key {
        let roles = [Role::Trader, Role::Quant, Role::RiskOfficer, Role::Zarili];
        let overview = client.xuangubao.fetch_market_overview().await?;
        let limit_up = client.xuangubao.fetch_limit_up().await.unwrap_or_default();

        for &role in &roles {
            let llm = LlmClient::new(key.clone());
            let prompt = build_prompt(role, &overview, &limit_up);

            print!("【{}】", role.name());

            match llm.chat_stream(&prompt).await {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(text) => {
                                print!("{}", text);
                                std::io::Write::flush(&mut std::io::stdout()).unwrap_or_default();
                            }
                            Err(e) => eprint!("{}", e),
                        }
                    }
                    println!("\n───────────────────────────────────\n");
                }
                Err(e) => eprintln!("【{}】分析失败: {}", role.name(), e),
            }
        }
    } else {
        println!("⚠️ 未配置 OPENROUTER_API_KEY，无法使用 AI 分析");
    }
    Ok(())
}

async fn run_ai_role(client: &Client, api_key: &Option<String>, role: Role) -> Result<()> {
    if let Some(key) = api_key {
        let overview = client.xuangubao.fetch_market_overview().await?;
        let limit_up = client.xuangubao.fetch_limit_up().await.unwrap_or_default();
        let llm = LlmClient::new(key.clone());
        let prompt = build_prompt(role, &overview, &limit_up);

        print!("【{}】", role.name());

        match llm.chat_stream(&prompt).await {
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(text) => {
                            print!("{}", text);
                            std::io::Write::flush(&mut std::io::stdout()).unwrap_or_default();
                        }
                        Err(e) => eprint!("{}", e),
                    }
                }
                println!("\n");
            }
            Err(e) => eprintln!("【{}】分析失败: {}", role.name(), e),
        }
    } else {
        println!("⚠️ 未配置 OPENROUTER_API_KEY，无法使用 AI 分析");
    }
    Ok(())
}

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
        "sentiment",
        "technical",
        "ai",
        "ai trader",
        "ai quant",
        "ai risk",
        "ai zarili",
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
        " - 舆情分析",
        " - 技术分析",
        " - AI 综合分析(四种角色)",
        " - 游资角色分析",
        " - 量化角色分析",
        " - 风控角色分析",
        " - zarili量价派分析",
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
            "sentiment" => run_sentiment_cmd(&client).await?,
            "technical" => run_technical_cmd(&client).await?,
            "ai" => {
                run_ai_all(&client, &api_key).await?;
            }
            "ai trader" => run_ai_role(&client, &api_key, Role::Trader).await?,
            "ai quant" => run_ai_role(&client, &api_key, Role::Quant).await?,
            "ai risk" => run_ai_role(&client, &api_key, Role::RiskOfficer).await?,
            "ai zarili" => run_ai_role(&client, &api_key, Role::Zarili).await?,
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
        None,
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

    for &role in &roles {
        let llm = LlmClient::new(api_key.clone());
        let prompt = build_prompt(role, overview, limit_up);

        print!("【{}】", role.name());

        match llm.chat_stream(&prompt).await {
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(text) => {
                            print!("{}", text);
                            std::io::Write::flush(&mut std::io::stdout()).unwrap_or_default();
                        }
                        Err(e) => eprint!("{}", e),
                    }
                }
                println!("\n───────────────────────────────────\n");
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

fn markdown_to_html(title: &str, meta: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 40px auto; padding: 20px; line-height: 1.6; color: #333; }}
        h1 {{ color: #2c3e50; border-bottom: 2px solid #3498db; padding-bottom: 10px; }}
        .meta {{ color: #666; font-size: 14px; margin-bottom: 20px; padding: 10px; background: #f8f9fa; border-radius: 4px; }}
        .content {{ white-space: pre-wrap; font-family: inherit; }}
    </style>
</head>
<body>
    <h1>{}</h1>
    <div class="meta">{}</div>
    <div class="content">{}</div>
</body>
</html>"#,
        title, title, meta, content
    )
}
