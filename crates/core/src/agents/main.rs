use crate::agents::sentiment::{
    analyze_sentiment, build_prompt as sentiment_prompt, SentimentResult,
};
use crate::agents::technical::{
    analyze_technical, build_prompt as technical_prompt, TechnicalResult,
};
use crate::api::Client;
use crate::llm::openrouter::LlmClient;
use crate::roles::role::Role;
use anyhow::Result;

pub struct MainAgentResult {
    pub sentiment: SentimentResult,
    pub technical: TechnicalResult,
    pub role_analysis: String,
}

pub async fn run_main_agent(role: Role, client: &Client, api_key: &str) -> Result<MainAgentResult> {
    println!("📊 拉取舆情数据...");
    let sentiment = analyze_sentiment(client).await?;
    println!("✅ 舆情分析完成: {}", sentiment.summary);

    println!("📈 拉取技术数据...");
    let technical = analyze_technical(client).await?;
    println!("✅ 技术分析完成: {}", technical.summary);

    let overview = client.xuangubao.fetch_market_overview().await?;

    let sentiment_prompt = sentiment_prompt(&sentiment);
    let technical_prompt = technical_prompt(&technical);

    let combined_prompt = format!(
        r#"{}
        
{}
        
{}

## 大盘数据
- 上涨: {} 下跌: {}
- 涨停: {} 跌停: {}
- 炸板率: {:.1}%

请以【{}】身份，综合舆情和技术面分析，给出：
1. 市场定性
2. 重点关注
3. 操作建议

100字以内。"#,
        sentiment_prompt,
        technical_prompt,
        "-".repeat(30),
        overview.rise_count,
        overview.fall_count,
        overview.limit_up_count,
        overview.limit_down_count,
        overview.bomb_rate * 100.0,
        role.name(),
    );

    let llm = LlmClient::new(api_key.to_string());
    let role_analysis = llm.chat(&combined_prompt).await?;

    Ok(MainAgentResult {
        sentiment,
        technical,
        role_analysis,
    })
}

pub fn print_result(result: &MainAgentResult, role: Role) {
    println!("\n═══════════════════════════════════");
    println!("【{}】综合分析\n", role.name());

    println!("📊 舆情分析:");
    println!("  热度: {}", result.sentiment.trend);
    if !result.sentiment.hot_stocks.is_empty() {
        let stocks: Vec<_> = result
            .sentiment
            .hot_stocks
            .iter()
            .take(3)
            .map(|(code, name, _)| format!("{}({})", name, code))
            .collect();
        println!("  热门: {}", stocks.join(", "));
    }

    println!("\n📈 技术分析:");
    println!("  {}", result.technical.summary);

    println!("\n🤖 分析结论:");
    println!("{}\n", result.role_analysis);
}
