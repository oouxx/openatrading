use crate::api::Client;
use anyhow::Result;

pub struct SentimentResult {
    pub hot_stocks: Vec<(String, String, i32)>,
    pub hot_articles: Vec<(String, i32)>,
    pub trend: String,
    pub summary: String,
}

pub async fn analyze_sentiment(client: &Client) -> Result<SentimentResult> {
    let resp = client.jiuyan.fetch_community("", 0, 1, 30).await?;
    let articles = &resp.result;

    let mut stock_mentions: std::collections::HashMap<(String, String), i32> = Default::default();
    for article in articles {
        for stock in &article.stock_list {
            *stock_mentions
                .entry((stock.code.clone(), stock.name.clone()))
                .or_insert(0) += 1;
        }
    }

    let mut hot_stocks: Vec<_> = stock_mentions
        .into_iter()
        .map(|((code, name), cnt)| (code, name, cnt))
        .collect();
    hot_stocks.sort_by(|a, b| b.2.cmp(&a.2));
    let hot_stocks: Vec<_> = hot_stocks.into_iter().take(10).collect();

    let mut hot_articles: Vec<_> = articles
        .iter()
        .map(|a| {
            (
                a.title.clone(),
                a.like_count + a.comment_count + a.forward_count,
            )
        })
        .collect();
    hot_articles.sort_by(|a, b| b.1.cmp(&a.1));
    let hot_articles: Vec<_> = hot_articles.into_iter().take(5).collect();

    let total_engagement: i32 = articles
        .iter()
        .map(|a| a.like_count + a.comment_count)
        .sum();
    let trend = if total_engagement > 500 {
        "偏多".to_string()
    } else if total_engagement > 200 {
        "中性".to_string()
    } else {
        "偏弱".to_string()
    };

    let stock_list: Vec<String> = hot_stocks
        .iter()
        .take(5)
        .map(|(code, name, _)| format!("{}:{}", code, name))
        .collect();
    let summary = format!(
        "今日社群讨论热度{}, 热门股票: {}",
        trend,
        stock_list.join(", ")
    );

    Ok(SentimentResult {
        hot_stocks,
        hot_articles,
        trend,
        summary,
    })
}

pub fn build_prompt(result: &SentimentResult) -> String {
    let stocks = result
        .hot_stocks
        .iter()
        .take(5)
        .map(|(code, name, cnt)| format!("{} {} ({}次)", code, name, cnt))
        .collect::<Vec<_>>()
        .join("\n");

    let articles = result
        .hot_articles
        .iter()
        .take(5)
        .map(|(title, eng)| format!("{} (热度:{})", title, eng))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"你是舆情分析专家，负责分析社群讨论热度。

## 今日社群舆情
热度趋势: {}

### 热门股票
{}

### 热门文章
{}

---
请分析：
1. 市场情绪偏向
2. 需要关注的潜在热点
3. 风险提示

50字以内。"#,
        result.trend, stocks, articles,
    )
}
