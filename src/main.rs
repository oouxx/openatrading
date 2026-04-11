mod api;
mod llm;
mod report;
mod roles;

use anyhow::Result;
use futures::future::join_all;

use api::xuangubao::XuanguBaoClient;
use chrono::Local;
use llm::openrouter::LlmClient;
use report::report::{Report, RoleAnalysis};
use roles::analyst::{build_prompt, Role};

use crate::report::report::Reporter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("OPENROUTER_API_KEY")?;

    let xgb = XuanguBaoClient::new();
    let llm = LlmClient::new(api_key);

    println!("📡 拉取选股宝数据...");

    // 并发拉取
    let (limit_up, limit_down, overview) = tokio::join!(
        xgb.fetch_limit_up(),
        xgb.fetch_limit_down(),
        xgb.fetch_market_overview(),
    );

    let limit_up = limit_up?;
    let _ = limit_down?;
    let overview = overview?;

    // 拉取九言公社异动数据
    let today = Local::now().format("%Y-%m-%d").to_string();
    let field_items = if std::env::var("JIUYAN_PHONE").is_ok() {
        println!("📡 拉取九言公社异动数据...");
        let jy = api::jiuyangongshe::JiuYanSession::new();
        match jy.fetch_field(&today, 1).await {
            Ok(data) => {
                let total: usize = data.iter().map(|p| p.list.len()).sum();
                println!(
                    "✅ 九言公社异动数据: {} 个板块，{} 只股票",
                    data.len(),
                    total
                );
                data
            }
            Err(e) => {
                eprintln!("⚠️ 九言公社拉取失败: {}", e);
                vec![]
            }
        }
    } else {
        println!("⚠️ 未配置 JIUYAN_PHONE，跳过九言公社数据");
        vec![]
    };

    // 生成report
    let ra = RoleAnalysis {
        role: Role::Trader,
        content: "".to_string(),
        elapsed_ms: 0,
    };
    let report = Report::new(overview.clone(), limit_up.clone(), field_items, vec![ra]);
    let reporter = Reporter::new(report);
    _ = reporter.save_markdown("./reports");

    let ai_enabled = std::env::var("AI_ENABLED")?.parse().unwrap_or(false);
    // 三个角色并发分析
    //
    if ai_enabled {
        let roles = [Role::Trader, Role::Quant, Role::RiskOfficer];

        let tasks: Vec<_> = roles
            .iter()
            .map(|&role| {
                let prompt = build_prompt(role, &overview, &limit_up);
                let llm = &llm;
                async move {
                    let result = llm.chat(&prompt).await;
                    (role, result)
                }
            })
            .collect();

        let results = join_all(tasks).await;

        println!("═══════════════════════════════════\n");
        for (role, result) in results {
            match result {
                Ok(text) => {
                    println!("【{}】", role.name());
                    println!("{}\n", text);
                    println!("───────────────────────────────────\n");
                }
                Err(e) => eprintln!("【{}】分析失败: {}", role.name(), e),
            }
        }
    }

    Ok(())
}
