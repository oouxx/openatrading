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
    pub async fn fetch_telegraph(&self, category: &str) -> Result<Vec<TelegramInfo>> {
        let api_url = if !category.is_empty() {
            format!("{}/v1/roll/get_roll_list", ROOT_URL)
        } else {
            format!("{}/api/cache", ROOT_URL)
        };
        let mut params = BTreeMap::new();
        if !category.is_empty() {
            params.insert("category".to_string(), category.to_string());
            params.insert("hasFirstVipArticle".to_string(), "1".to_string());
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
    let items = client.fetch_telegraph("").await.unwrap();
    println!("fetched {} telegraph items", items.len());
    assert!(
        !items.is_empty(),
        "should return at least one telegraph item"
    );
}
