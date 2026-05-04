use crate::api::jiuyangongshe::CommunityArticle;
use crate::api::Client;
use anyhow::Result;
use chrono::Local;

#[derive(Debug, Clone)]
pub struct SentimentResult {
    pub hot_stocks: Vec<(String, String, i32)>,
    pub hot_articles: Vec<(String, i32)>,
    pub trend: String,
    pub summary: String,
    pub news_summary: String,
    pub articles: Vec<CommunityArticle>,
}

pub async fn analyze_sentiment(client: &Client) -> Result<SentimentResult> {
    let mut all_articles = Vec::new();
    for page in 1..=3 {
        let resp = client.jiuyan.fetch_community("", 0, page, 30).await?;
        if resp.result.is_empty() {
            break;
        }
        all_articles.extend(resp.result);
    }

    let today = Local::now().format("%Y-%m-%d").to_string();
    let articles: Vec<CommunityArticle> = all_articles
        .into_iter()
        .filter(|a| a.create_time.starts_with(&today))
        .collect();

    if articles.is_empty() {
        return Ok(SentimentResult {
            hot_stocks: vec![],
            hot_articles: vec![],
            trend: "暂无数据".to_string(),
            summary: "今日暂无社群文章".to_string(),
            news_summary: "今日暂无社群文章更新".to_string(),
            articles: vec![],
        });
    }

    let articles_ref: Vec<&CommunityArticle> = articles.iter().collect();

    let mut stock_mentions: std::collections::HashMap<(String, String), i32> = Default::default();
    for article in &articles_ref {
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

    let mut hot_articles: Vec<_> = articles_ref
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

    let total_engagement: i32 = articles_ref
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
        news_summary: String::new(),
        articles,
    })
}

pub fn analyze_from_articles(mut articles: Vec<CommunityArticle>) -> SentimentResult {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    articles.retain(|a| a.create_time.starts_with(&today));

    if articles.is_empty() {
        return SentimentResult {
            hot_stocks: vec![],
            hot_articles: vec![],
            trend: "暂无数据".to_string(),
            summary: "今日暂无社群文章".to_string(),
            news_summary: "今日暂无社群文章更新".to_string(),
            articles: vec![],
        };
    }

    let articles_ref: Vec<&CommunityArticle> = articles.iter().collect();

    let mut stock_mentions: std::collections::HashMap<(String, String), i32> = Default::default();
    for article in &articles_ref {
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

    let mut hot_articles: Vec<_> = articles_ref
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

    let total_engagement: i32 = articles_ref
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

    SentimentResult {
        hot_stocks,
        hot_articles,
        trend,
        summary,
        news_summary: String::new(),
        articles,
    }
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

### 今日新闻资讯摘要
{}

---
请分析：
1. 市场情绪偏向
2. 需要关注的潜在热点
3. 风险提示

50字以内。"#,
        result.trend, stocks, articles, result.news_summary,
    )
}
