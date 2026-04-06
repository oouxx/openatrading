mod api;
mod llm;
mod roles;

use anyhow::Result;
use futures::future::join_all;

use api::xuangubao::XuanguBaoClient;
use llm::openrouter::LlmClient;
use roles::analyst::{Role, build_prompt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("OPENROUTER_API_KEY")?;

    let xgb = XuanguBaoClient::new();
    let llm = LlmClient::new(api_key);

    println!("📡 拉取选股宝数据...");

    // 并发拉取
    let (limit_up, overview) = tokio::join!(xgb.fetch_limit_up(), xgb.fetch_market_overview(),);

    let limit_up = limit_up?;
    let overview = overview?;

    println!(
        "✅ 涨停 {} 家，上涨 {} 家\n",
        overview.limit_up_count, overview.rise_count
    );

    // 三个角色并发分析
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

    Ok(())
}
