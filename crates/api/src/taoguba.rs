use anyhow::Result;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use tokio::fs;

// 淘股吧推荐 API 响应结构体
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct TgbRecommendResponse {
    dto: TgbRecommendDto,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct TgbRecommendDto {
    list: Vec<TgbRecommendItem>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct TgbRecommendItem {
    topicID: i64,
    subject: Option<String>,
    subinfo: Option<String>,
    userName: Option<String>,
    totalViewNum: Option<i64>,
    totalReplyNum: Option<i64>,
    dateTime: Option<i64>,
    newTopicID: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaogubaRecommend {
    pub topic_id: i64,
    pub new_topic_id: String,
    pub subject: String,
    pub subinfo: String,
    pub user_name: String,
    pub view_count: i64,
    pub reply_count: i64,
    pub date_time: String,
    pub content: String,
}

const BASE_URL: &str = "https://www.tgb.cn";
const MOBILE_URL: &str = "https://m.tgb.cn";

#[derive(Debug, Clone)]
pub struct TgbPost {
    pub id: String,
    pub title: String,
    pub content: String,
    pub create_time: String,
    pub view_count: i32,
    pub reply_count: i32,
    pub like_count: i32,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TgbUserSummary {
    pub user_id: String,
    pub nickname: String,
    pub post_count: usize,
    pub top_post_title: String,
    pub last_update: String,
}

#[derive(Clone)]
pub struct TgbClient {
    client: reqwest::Client,
    cookie: Option<String>,
}

impl TgbClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap();
        Self {
            client,
            cookie: None,
        }
    }

    pub fn with_cookie(cookie: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap();
        Self {
            client,
            cookie: Some(cookie),
        }
    }
}

impl Default for TgbClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TgbClient {
    pub async fn fetch_user_posts(&self, user_id: &str) -> Result<(Vec<TgbPost>, String)> {
        let url = format!("{}/blog/{}", BASE_URL, user_id);

        let mut request = self.client.get(&url);
        if let Some(ref cookie) = self.cookie {
            request = request.header("Cookie", cookie);
        }

        let html = request.send().await?.text().await?;

        let (posts, username) = self.parse_posts_html(&html)?;
        Ok((posts, username))
    }

    fn parse_posts_html(&self, html: &str) -> Result<(Vec<TgbPost>, String)> {
        let document = Html::parse_document(html);

        let title_selector = Selector::parse("title").unwrap();
        let username = document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();
        let username = username.trim_end_matches("_博客_淘股吧").to_string();

        // 遍历 .article_tittle 容器，其中包含 .tittle_data、.tittle_llhf、.tittle_fbshijian
        let article_selector = Selector::parse(".article_tittle").unwrap();
        let a_selector = Selector::parse(".tittle_data a").unwrap();
        let stats_selector = Selector::parse(".tittle_llhf").unwrap();
        let time_selector = Selector::parse(".tittle_fbshijian").unwrap();

        let posts: Vec<TgbPost> = document
            .select(&article_selector)
            .filter_map(|article| {
                let link = article.select(&a_selector).next()?;
                let href = link.value().attr("href")?;
                let title: String = link.text().collect();
                let title = title.trim().to_string();

                if !href.starts_with("a/") || title.is_empty() {
                    return None;
                }

                let id = href.trim_start_matches("a/");
                let stats = article.select(&stats_selector).next()?;
                let stats_text = stats.text().collect::<String>();
                let (views, replies) = parse_stats(&stats_text);

                let time_el = article.select(&time_selector).next();
                let time = time_el
                    .map(|el| el.text().collect::<String>())
                    .unwrap_or_default();
                let time = time.trim().to_string();

                Some(TgbPost {
                    id: id.to_string(),
                    title: title.to_string(),
                    content: String::new(),
                    create_time: time,
                    view_count: views,
                    reply_count: replies,
                    like_count: 0,
                    link: format!("{}/a/{}", BASE_URL, id),
                })
            })
            .collect();

        Ok((posts, username))
    }

    pub async fn fetch_post_content(&self, post_id: &str) -> Result<(String, String)> {
        let url = format!("{}/a/{}", BASE_URL, post_id);

        let mut request = self.client.get(&url);
        if let Some(ref cookie) = self.cookie {
            request = request.header("Cookie", cookie);
        }

        let html = request.send().await?.text().await?;

        // 检测登录墙
        let has_login_wall = Html::parse_document(&html)
            .select(&Selector::parse(".login-view-button").unwrap())
            .next()
            .is_some();

        if has_login_wall {
            // PC 端有登录墙时，回退到移动端获取完整内容
            self.fetch_mobile_post_content(post_id).await
        } else {
            Self::extract_post_content(&html)
        }
    }

    async fn fetch_mobile_post_content(&self, post_id: &str) -> Result<(String, String)> {
        let url = format!("{}/a/{}", MOBILE_URL, post_id);

        let mut request = self.client.get(&url);
        if let Some(ref cookie) = self.cookie {
            request = request.header("Cookie", cookie);
        }

        let html = request.send().await?.text().await?;
        Self::extract_mobile_post_content(&html)
    }

    fn extract_mobile_post_content(html: &str) -> Result<(String, String)> {
        let document = Html::parse_document(html);

        // 移动端内容在 .tzitem_text 中，无登录墙
        let content_el = document
            .select(&Selector::parse(".tzitem_text").unwrap())
            .next()
            .or_else(|| {
                document
                    .select(&Selector::parse(".p_coten").unwrap())
                    .next()
            })
            .or_else(|| document.select(&Selector::parse("#first").unwrap()).next())
            .or_else(|| {
                document
                    .select(&Selector::parse(".article-text").unwrap())
                    .next()
            });

        let content = if let Some(el) = content_el {
            let mut inner = el.inner_html();
            // 处理懒加载图片
            let src2_pattern = regex::Regex::new(r#"src2="([^"]*)""#).unwrap();
            inner = src2_pattern.replace_all(&inner, r#"src="$1""#).to_string();
            // 移除 data-original 不需要，html_to_markdown 会优先使用它
            let video_img_pattern =
                regex::Regex::new(r#"<[^>]*id="videoImg"[^>]*>.*?</[^>]+>"#).unwrap();
            inner = video_img_pattern.replace_all(&inner, "").to_string();
            crate::utils::html_to_markdown(&inner)
        } else {
            String::new()
        };

        // 提取发布时间：移动端时间在 JS 注释中，如 'postDate':'2025-08-24 21:26:39.0'
        let time_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}").unwrap();
        let pub_time = time_re
            .find(html)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        Ok((content.trim().to_string(), pub_time))
    }

    fn extract_post_content(html: &str) -> Result<(String, String)> {
        let document = Html::parse_document(html);

        // 提取正文元素
        let content_el = document
            .select(&Selector::parse("#first").unwrap())
            .next()
            .or_else(|| {
                document
                    .select(&Selector::parse(".article-text").unwrap())
                    .next()
            })
            .or_else(|| {
                document
                    .select(&Selector::parse("div.p_coten").unwrap())
                    .next()
            });

        let content = if let Some(el) = content_el {
            // 先处理懒加载图片：将 src2 属性值赋给 src（参考 RSSHub）
            let mut inner = el.inner_html();
            let src2_pattern = regex::Regex::new(r#"src2="([^"]*)""#).unwrap();
            inner = src2_pattern.replace_all(&inner, r#"src="$1""#).to_string();
            let data_original_pattern = regex::Regex::new(r#"\s*data-original="[^"]*""#).unwrap();
            inner = data_original_pattern.replace_all(&inner, "").to_string();
            let video_img_pattern =
                regex::Regex::new(r#"<[^>]*id="videoImg"[^>]*>.*?</[^>]+>"#).unwrap();
            inner = video_img_pattern.replace_all(&inner, "").to_string();
            // 将 HTML 转为 Markdown
            crate::utils::html_to_markdown(&inner)
        } else {
            String::new()
        };

        // 无登录墙（有登录墙时已回退到移动端）

        // 提取发布时间：使用更精确的选择器（参考 RSSHub）
        let time_selector = Selector::parse(".article-data > span:nth-child(2)").unwrap();
        let time = document
            .select(&time_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        let time_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}").unwrap();
        let pub_time = if let Some(captures) = time_re.find(&time) {
            captures.as_str().to_string()
        } else {
            let fallback_selector = Selector::parse(".article-data").unwrap();
            let fallback_time = document
                .select(&fallback_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default();
            if let Some(captures) = time_re.find(&fallback_time) {
                captures.as_str().to_string()
            } else {
                fallback_time
                    .replace("发布于", "")
                    .replace(" ", "")
                    .replace("\n", "")
                    .trim()
                    .to_string()
            }
        };

        Ok((content.trim().to_string(), pub_time))
    }

    pub async fn fetch_recommend_articles(&self) -> Result<Vec<TaogubaRecommend>> {
        // 并行请求 3 页推荐数据
        async fn fetch_page(
            client: &reqwest::Client,
            cookie: &Option<String>,
            page: i32,
        ) -> Result<Vec<TgbRecommendItem>> {
            let url = format!(
                "https://www.tgb.cn/newIndex/getNowRecommend?pageNo={}",
                page
            );
            let mut request = client.get(&url);
            if let Some(ref c) = cookie {
                request = request.header("Cookie", c.clone());
            }
            let resp: TgbRecommendResponse = request.send().await?.json().await?;
            Ok(resp.dto.list)
        }

        let (p1, p2, p3) = tokio::join!(
            fetch_page(&self.client, &self.cookie, 1),
            fetch_page(&self.client, &self.cookie, 2),
            fetch_page(&self.client, &self.cookie, 3),
        );

        let mut items = Vec::new();
        for result in [p1, p2, p3] {
            match result {
                Ok(list) => items.extend(list),
                Err(e) => eprintln!("获取淘股吧推荐页失败: {}", e),
            }
        }

        let articles: Vec<TaogubaRecommend> = items
            .into_iter()
            .map(|item| {
                let date_time = item
                    .dateTime
                    .and_then(|ms| {
                        chrono::DateTime::from_timestamp(
                            ms / 1000,
                            ((ms % 1000) * 1_000_000) as u32,
                        )
                    })
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default();

                TaogubaRecommend {
                    topic_id: item.topicID,
                    new_topic_id: item.newTopicID.unwrap_or_default(),
                    subject: item.subject.unwrap_or_default(),
                    subinfo: item.subinfo.clone().unwrap_or_default(),
                    user_name: item.userName.unwrap_or_default(),
                    view_count: item.totalViewNum.unwrap_or(0),
                    reply_count: item.totalReplyNum.unwrap_or(0),
                    date_time,
                    content: item.subinfo.unwrap_or_default(),
                }
            })
            .collect();

        Ok(articles)
    }
}

fn parse_stats(text: &str) -> (i32, i32) {
    let parts: Vec<&str> = text.split('/').collect();
    let views = parts
        .first()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    let replies = parts
        .get(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    (views, replies)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_user_posts(client: &TgbClient, user_id: &str, output_dir: &str) -> Result<()> {
    println!("📥 正在获取用户 {} 的帖子...", user_id);

    let (posts, username) = client.fetch_user_posts(user_id).await?;
    println!("✅ 获取到 {} 篇帖子", posts.len());

    if posts.is_empty() {
        println!("⚠️ 未获取到帖子");
        return Ok(());
    }

    let user_dir = Path::new(output_dir).join(user_id);
    fs::create_dir_all(&user_dir).await?;

    let mut index = format!(
        "# 用户 {} ({}) 的帖子\n\n共 {} 篇\n\n## 列表\n\n",
        user_id,
        username,
        posts.len()
    );

    for (i, post) in posts.iter().enumerate() {
        let safe_title: String = post
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .take(50)
            .collect();

        if safe_title.is_empty() {
            continue;
        }

        println!("📝 获取第 {}/{} 篇: {}", i + 1, posts.len(), post.title);

        let (content, create_time) = match client.fetch_post_content(&post.id).await {
            Ok((c, t)) => (c, t),
            Err(e) => {
                eprintln!("获取帖子内容失败: {}", e);
                (String::new(), String::new())
            }
        };

        let date = if create_time.is_empty() {
            post.create_time.clone()
        } else {
            create_time
        };

        let filename = format!("{}_{:03}.md", safe_title, i + 1);
        let md = format!(
            "# {}\n\n**时间:** {} | **浏览:** {} | **回复:** {}\n\n{}\n\n---\n\n",
            post.title, date, post.view_count, post.reply_count, content
        );
        fs::write(user_dir.join(filename), md).await?;

        index.push_str(&format!("{}. [{}]({})\n", i + 1, post.title, post.link));
    }

    fs::write(user_dir.join("index.md"), &index).await?;

    let user_md = format!(
        r#"# 用户信息

- UserID: {}
- 用户名: {}
- 帖子数: {}

>"#,
        user_id,
        username,
        posts.len(),
    );
    fs::write(user_dir.join("user.md"), user_md).await?;

    println!(
        "✅ 用户 {} ({}) 的 {} 篇帖子已保存到: {}",
        user_id,
        username,
        posts.len(),
        user_dir.display()
    );
    Ok(())
}
