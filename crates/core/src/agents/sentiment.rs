use crate::api::jiuyangongshe::CommunityArticle;
use crate::api::taoguba::TaogubaRecommend;
use crate::api::Client;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotArticle {
    pub article_id: String,
    pub title: String,
    pub like_count: i32,
    pub comment_count: i32,
    pub forward_count: i32,
    pub heat_score: i32,
    pub nickname: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResult {
    pub hot_stocks: Vec<(String, String, i32)>,
    pub hot_articles: Vec<HotArticle>,
    pub trend: String,
    pub summary: String,
    pub news_summary: String,
    pub articles: Vec<CommunityArticle>,
    pub taoguba_articles: Vec<TaogubaRecommend>,
}

pub async fn analyze_sentiment(client: &Client) -> Result<SentimentResult> {
    // 并行获取韭研数据和淘股吧推荐
    let (jiuyan_articles, taoguba_articles) = tokio::join!(
        async {
            let mut all = Vec::new();
            for page in 1..=3 {
                match client
                    .jiuyan
                    .fetch_community(crate::api::jiuyangongshe::CommunityQuery {
                        category_id: String::new(),
                        article_type: 0,
                        page,
                        page_size: 30,
                    })
                    .await
                {
                    Ok(resp) => {
                        if resp.result.is_empty() {
                            break;
                        }
                        all.extend(resp.result);
                    },
                    Err(e) => {
                        crate::log::warn!("获取韭研数据失败: {}", e);
                        break;
                    },
                }
            }
            all
        },
        async {
            client
                .taoguba
                .fetch_recommend_articles()
                .await
                .unwrap_or_else(|e| {
                    crate::log::warn!("获取淘股吧推荐失败: {}", e);
                    vec![]
                })
        },
    );

    let today = crate::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    let articles: Vec<CommunityArticle> = jiuyan_articles
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
            taoguba_articles,
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
    hot_stocks.sort_by_key(|s| std::cmp::Reverse(s.2));
    let hot_stocks: Vec<_> = hot_stocks.into_iter().take(10).collect();

    let mut hot_articles: Vec<HotArticle> = articles_ref
        .iter()
        .map(|a| {
            let heat = a.like_count + a.comment_count + a.forward_count;
            HotArticle {
                article_id: a.article_id.clone(),
                title: a.title.clone(),
                like_count: a.like_count,
                comment_count: a.comment_count,
                forward_count: a.forward_count,
                heat_score: heat,
                nickname: a.user.nickname.clone(),
                content: a.content.clone().unwrap_or_default(),
            }
        })
        .collect();
    hot_articles.sort_by_key(|a| std::cmp::Reverse(a.heat_score));

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
        taoguba_articles,
    })
}

pub fn analyze_from_articles(mut articles: Vec<CommunityArticle>) -> SentimentResult {
    let today = crate::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();
    articles.retain(|a| a.create_time.starts_with(&today));

    if articles.is_empty() {
        return SentimentResult {
            hot_stocks: vec![],
            hot_articles: vec![],
            trend: "暂无数据".to_string(),
            summary: "今日暂无社群文章".to_string(),
            news_summary: "今日暂无社群文章更新".to_string(),
            articles: vec![],
            taoguba_articles: vec![],
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
    hot_stocks.sort_by_key(|s| std::cmp::Reverse(s.2));
    let hot_stocks: Vec<_> = hot_stocks.into_iter().take(10).collect();

    let mut hot_articles: Vec<HotArticle> = articles_ref
        .iter()
        .map(|a| {
            let heat = a.like_count + a.comment_count + a.forward_count;
            HotArticle {
                article_id: a.article_id.clone(),
                title: a.title.clone(),
                like_count: a.like_count,
                comment_count: a.comment_count,
                forward_count: a.forward_count,
                heat_score: heat,
                nickname: a.user.nickname.clone(),
                content: a.content.clone().unwrap_or_default(),
            }
        })
        .collect();
    hot_articles.sort_by_key(|a| std::cmp::Reverse(a.heat_score));

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
        taoguba_articles: vec![],
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
        .map(|a| format!("{} (热度:{})", a.title, a.heat_score))
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
