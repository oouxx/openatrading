use anyhow::{Ok, Result};
use md5;
use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use std::collections::BTreeMap;
use url::form_urlencoded;
#[derive(Clone)]
pub struct ClsClient {
    client: reqwest::Client,
}
#[derive(Debug, Clone)]
pub struct News {
    pub id: String,
    pub title: String,
    pub content: String,
    pub create_time: String,
    pub is_red: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ClsTelegraphResp {
    #[serde(alias = "errno")]
    pub error: i32,
    pub data: TelegraphData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelegraphData {
    pub roll_data: Vec<TelegramInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelegramInfo {
    pub id: Option<i64>,
    pub title: Option<String>,
    pub brief: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "ctime")]
    pub ctime: Option<i64>,
    #[serde(rename = "modified_time")]
    pub modified_time: Option<i64>,
    pub level: Option<String>,
    pub reading_num: Option<i64>,
    pub deny_comment: Option<i32>,
    #[serde(default)]
    pub author_extends: Option<String>,
    #[serde(default)]
    pub depth_extends: Option<String>,
    #[serde(default)]
    pub assoc_fast_fact: Option<serde_json::Value>,
}

// pub enum Categories {
//     Watch,
//     Announcement ,
//     Explain ,
//     Red,
//     Jpush,
//     Remind,
//     Fund ,
//     Hk,
// }

impl ClsClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap();
        Self { client }
    }
}
impl Default for ClsClient {
    fn default() -> Self {
        Self::new()
    }
}
const ROOT_URL: &str = "https://www.cls.cn";

impl ClsClient {
    /// Fetch telegraph with cursor-based pagination.
    /// Pass `category=""` for all, or e.g. "red", "watch".
    /// Cursor-based: pass `last_time` (oldest item's ctime) to get next page.
    pub async fn fetch_telegraph(
        &self,
        category: &str,
        last_time: Option<i64>,
        count: u32,
    ) -> Result<Vec<TelegramInfo>> {
        let api_url = if !category.is_empty() {
            format!("{}/v1/roll/get_roll_list", ROOT_URL)
        } else {
            format!("{}/api/cache", ROOT_URL)
        };
        let mut params = BTreeMap::new();
        if !category.is_empty() {
            params.insert("category".to_string(), category.to_string());
            params.insert("hasFirstVipArticle".to_string(), "1".to_string());
            params.insert("rn".to_string(), count.to_string());
            params.insert("refresh_type".to_string(), "1".to_string());
            if let Some(lt) = last_time {
                params.insert("last_time".to_string(), lt.to_string());
            }
        } else {
            params.insert("name".to_string(), "telegraph".to_string());
        }
        let querystr = get_search_params(params);
        let url = format!("{}?{}", api_url, querystr);
        eprintln!("[cls::telegraph] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let body_text = resp.text().await?;
        eprintln!(
            "[cls::telegraph] status={}, body={}",
            status,
            &body_text.chars().take(500).collect::<String>()
        );
        let resp_data: ClsTelegraphResp = serde_json::from_str(&body_text)?;
        Ok(resp_data.data.roll_data)
    }

    /// Shortcut for LLM tool: fetch latest telegraph without pagination
    pub async fn fetch_telegraph_latest(&self, category: &str) -> Result<Vec<TelegramInfo>> {
        self.fetch_telegraph(category, None, 50).await
    }

    /// Fetch all telegraph items for the current trading day via cursor pagination.
    /// Stops when reaching items before today 00:00 or empty page.
    pub async fn fetch_telegraph_allday(&self, category: &str) -> Result<Vec<TelegramInfo>> {
        let today_start = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();

        let mut all_items: Vec<TelegramInfo> = Vec::new();
        let mut cursor: Option<i64> = None;
        let page_size: u32 = 50;

        loop {
            let page = self.fetch_telegraph(category, cursor, page_size).await?;
            if page.is_empty() {
                break;
            }
            // Stop if oldest item is before today
            let oldest_ts = page.last().and_then(|i| i.ctime).unwrap_or(0);
            let keep: Vec<_> = page.into_iter().filter(|i| i.ctime.unwrap_or(0) >= today_start).collect();
            all_items.extend(keep);

            if oldest_ts < today_start {
                break;
            }
            cursor = Some(oldest_ts);
        }

        all_items.sort_by_key(|i| i.ctime.unwrap_or(0));
        all_items.dedup_by_key(|i| i.id);
        eprintln!("[cls] fetch_telegraph_allday: {} items", all_items.len());
        Ok(all_items)
    }
}

fn get_search_params(more_params: BTreeMap<String, String>) -> String {
    // 默认参数
    let mut params = BTreeMap::from([
        ("app".to_string(), "CailianpressWeb".to_string()),
        ("os".to_string(), "web".to_string()),
        ("sv".to_string(), "8.7.9".to_string()),
    ]);

    // 合并额外参数
    params.extend(more_params);

    // URL 编码并排序（BTreeMap 默认按 key 排序）
    let mut serializer = form_urlencoded::Serializer::new(String::new());

    for (k, v) in &params {
        serializer.append_pair(k, v);
    }

    let query_string = serializer.finish();

    // SHA1
    let mut sha1 = Sha1::new();
    sha1.update(query_string.as_bytes());
    let sha1_result = sha1.finalize();

    let sha1_hex = format!("{:x}", sha1_result);

    // MD5(SHA1(...))
    let sign = format!("{:x}", md5::compute(sha1_hex));

    // 拼接 sign
    let final_query = format!("{}&sign={}", query_string, sign);

    final_query
}

#[tokio::test(flavor = "current_thread")]
async fn test_fetch_telegraph() {
    let client = ClsClient::new();
    let items = client.fetch_telegraph("", None, 20).await.unwrap();
    println!("fetched {} telegraph items", items.len());
    assert!(
        !items.is_empty(),
        "should return at least one telegraph item"
    );
}
