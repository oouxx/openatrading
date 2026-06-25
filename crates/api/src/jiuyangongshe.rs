use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::utils::{gen_jiuyan_token, html_to_markdown, now_ts};

const BASE_URL: &str = "https://app.jiuyangongshe.com/jystock-app/api/v1";

// ─── Token & Cookie 凭证 ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Credentials {
    pub token: String,
    /// 登录成功后，将 sessionToken 写入 Cookie 格式: `SESSION=xxx`
    pub cookie: String,
}

// ─── action/field 响应结构 ────────────────────────────────────

/// 异动详情（action_info 字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInfo {
    /// 异动时间，如 "09:35:38"，可能为 null
    pub time: Option<String>,
    /// 连板信息，如 "4天4板"
    #[serde(default)]
    pub num: Option<String>,
    /// 异动理由/详细描述
    pub expound: Option<String>,
}

/// 文章信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub action_info: Option<ActionInfo>,
    /// 文章标题
    #[serde(default)]
    pub title: String,
}

/// 单只异动股票
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStock {
    pub code: String,
    pub name: String,
    pub article: Option<Article>,
}

/// 异动板块
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldPlate {
    /// 板块名称，如 "公告"、"固态电池"
    #[serde(default)]
    pub name: String,
    /// 板块异动原因
    #[serde(default)]
    pub reason: String,
    /// 该板块异动股票数量
    #[serde(default)]
    pub count: u32,
    /// 异动股票列表
    #[serde(default)]
    pub list: Vec<FieldStock>,
}

/// action/field 接口返回的数据结构（data 数组里的每个元素）
pub type FieldData = Vec<FieldPlate>;

// ─── Timeline 时间线数据 ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineUser {
    pub user_id: String,
    pub nickname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineTheme {
    #[serde(rename = "timeline_theme_id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineInfo {
    #[serde(rename = "article_id")]
    pub article_id: String,
    #[serde(rename = "timeline_id")]
    pub timeline_id: String,
    pub date: String,
    pub grade: i32,
    pub source: String,
    #[serde(rename = "create_time")]
    pub create_time: String,
    #[serde(default)]
    pub theme_list: Vec<TimelineTheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineItem {
    #[serde(rename = "article_id")]
    pub article_id: String,
    #[serde(rename = "user_id")]
    pub user_id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "like_count")]
    pub like_count: i32,
    #[serde(rename = "comment_count")]
    pub comment_count: i32,
    #[serde(rename = "forward_count")]
    pub forward_count: i32,
    pub user: TimelineUser,
    pub timeline: TimelineInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineDay {
    pub date: String,
    pub list: Vec<TimelineItem>,
}

pub type TimelineData = Vec<TimelineDay>;

// ─── 社群文章数据 ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityUser {
    pub user_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub style_str: Option<String>,
    #[serde(default)]
    pub medal_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityStock {
    pub stock_id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityArticle {
    pub article_id: String,
    pub is_top: i32,
    pub user_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub cover: Option<String>,
    #[serde(rename = "type")]
    pub article_type: i32,
    pub comment_count: i32,
    pub collect_count: i32,
    pub like_count: i32,
    pub forward_count: i32,
    pub create_time: String,
    pub content: Option<String>,
    pub user: CommunityUser,
    #[serde(default)]
    pub stock_list: Vec<CommunityStock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityData {
    pub result: Vec<CommunityArticle>,
    #[serde(rename = "pageNo")]
    pub page_no: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
}

/// Query parameters for the community article API.
pub struct CommunityQuery {
    pub category_id: String,
    pub article_type: i32,
    /// 1-based page number (passed directly as `start` to the API).
    pub page: i32,
    pub page_size: i32,
}

// ─── 会话管理器 ───────────────────────────────────────────────

struct InnerSession {
    credentials: Option<Credentials>,
    refreshing: AtomicBool,
}

pub struct JiuYanSession {
    inner: RwLock<InnerSession>,
    client: reqwest::Client,
}

impl Clone for JiuYanSession {
    fn clone(&self) -> Self {
        let creds = self
            .inner
            .try_read()
            .ok()
            .and_then(|g| g.credentials.clone());
        Self {
            inner: RwLock::new(InnerSession {
                credentials: creds,
                refreshing: AtomicBool::new(false),
            }),
            client: self.client.clone(),
        }
    }
}

impl Default for JiuYanSession {
    fn default() -> Self {
        Self::new()
    }
}

impl JiuYanSession {
    pub async fn fetch_article_detail(&self, article_id: &str) -> Result<Option<String>> {
        let url = format!("https://www.jiuyangongshe.com/a/{}", article_id);

        let html = self
            .client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .text()
            .await?;

        // Extract content field from window.__NUXT__
        // Look for "content:" followed by a quote, then extract the JSON string
        if let Some(start) = html.find("content:") {
            let start_idx = start + "content:".len();
            let bytes = html.as_bytes();
            // Skip whitespace and the opening quote
            let mut i = start_idx;
            while i < bytes.len()
                && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
            {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'"' {
                i += 1;
                let mut content_bytes = Vec::new();
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' if i + 1 < bytes.len() => match bytes[i + 1] {
                            b'"' => {
                                content_bytes.push(b'"');
                                i += 2;
                            },
                            b'\\' => {
                                content_bytes.push(b'\\');
                                i += 2;
                            },
                            b'n' => {
                                content_bytes.push(b'\n');
                                i += 2;
                            },
                            b'r' => {
                                content_bytes.push(b'\r');
                                i += 2;
                            },
                            b't' => {
                                content_bytes.push(b'\t');
                                i += 2;
                            },
                            b'u' if i + 5 < bytes.len() => {
                                let hex = &html[i + 2..i + 6];
                                if let Ok(code) = u32::from_str_radix(hex, 16) {
                                    if let Some(c) = char::from_u32(code) {
                                        let mut buf = [0u8; 4];
                                        let len = c.encode_utf8(&mut buf).len();
                                        content_bytes.extend_from_slice(&buf[..len]);
                                    }
                                    i += 6;
                                } else {
                                    content_bytes.push(bytes[i]);
                                    i += 1;
                                }
                            },
                            _ => {
                                content_bytes.push(bytes[i]);
                                i += 1;
                            },
                        },
                        b'"' => break,
                        _ => {
                            content_bytes.push(bytes[i]);
                            i += 1;
                        },
                    }
                }
                if let Ok(content_str) = String::from_utf8(content_bytes) {
                    let markdown = html_to_markdown(&content_str);
                    return Ok(Some(markdown));
                }
            }
        }

        Ok(None)
    }
    pub fn new() -> Self {
        let builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36");
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.cookie_store(true);
        let client = builder
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    "Accept",
                    "application/json, text/plain, */*".parse().unwrap(),
                );
                headers.insert("Origin", "https://www.jiuyangongshe.com".parse().unwrap());
                headers.insert("Referer", "https://www.jiuyangongshe.com/".parse().unwrap());
                headers.insert("platform", "3".parse().unwrap());
                headers
            })
            .build()
            .unwrap();

        Self {
            inner: RwLock::new(InnerSession {
                credentials: None,
                refreshing: AtomicBool::new(false),
            }),
            client,
        }
    }

    /// 获取凭证，如果已登录且未过期则直接返回
    pub async fn get_credentials(&self) -> Result<Credentials> {
        {
            let guard = self.inner.read().await;
            if let Some(creds) = &guard.credentials {
                return Ok(creds.clone());
            }
        }
        self.login().await
    }

    /// 执行登录
    pub async fn login(&self) -> Result<Credentials> {
        let phone = std::env::var("JIUYAN_PHONE").context("JIUYAN_PHONE not set in .env")?;
        let password =
            std::env::var("JIUYAN_PASSWORD").context("JIUYAN_PASSWORD not set in .env")?;

        // 防止并发重复登录
        let already_refreshing = self
            .inner
            .read()
            .await
            .refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err();

        if already_refreshing {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let guard = self.inner.read().await;
                if let Some(creds) = &guard.credentials {
                    return Ok(creds.clone());
                }
                if !guard.refreshing.load(Ordering::SeqCst) {
                    anyhow::bail!(
                        "login failed: another login attempt finished without credentials"
                    );
                }
            }
        }

        let url = format!("{BASE_URL}/user/login");
        let body = serde_json::json!({
            "phone": phone,
            "password": password
        });

        let ts = now_ts();

        let resp_result = self
            .client
            .post(&url)
            .header("timestamp", &ts.to_string())
            .header("token", gen_jiuyan_token(ts))
            .json(&body)
            .send()
            .await;

        let resp: serde_json::Value = match resp_result {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    self.inner
                        .write()
                        .await
                        .refreshing
                        .store(false, Ordering::SeqCst);
                    return Err(e.into());
                },
            },
            Err(e) => {
                self.inner
                    .write()
                    .await
                    .refreshing
                    .store(false, Ordering::SeqCst);
                return Err(e.into());
            },
        };

        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            if err_code != "0" {
                self.inner
                    .write()
                    .await
                    .refreshing
                    .store(false, Ordering::SeqCst);
                anyhow::bail!("登录失败: errCode={}, msg={}", err_code, resp["msg"]);
            }
        }

        let session_token = match resp["data"]["sessionToken"].as_str() {
            Some(s) => s.to_string(),
            None => {
                self.inner
                    .write()
                    .await
                    .refreshing
                    .store(false, Ordering::SeqCst);
                anyhow::bail!("登录成功但未返回 sessionToken");
            },
        };

        // 将 sessionToken 写入 Cookie 格式
        let cookie = format!("SESSION={}", session_token);

        let creds = Credentials {
            token: session_token.clone(),
            cookie: cookie.clone(),
        };

        // 写入缓存
        {
            let mut guard = self.inner.write().await;
            guard.credentials = Some(creds.clone());
            guard.refreshing.store(false, Ordering::SeqCst);
        }

        tracing::info!(
            token_prefix = %&session_token[..16.min(session_token.len())],
            "jiuyangongshe login succeeded"
        );
        Ok(creds)
    }

    /// 强制刷新 token
    pub async fn refresh(&self) -> Result<Credentials> {
        {
            let mut guard = self.inner.write().await;
            guard.credentials = None;
        }
        self.login().await
    }

    /// 检查响应是否有业务错误，返回 true 表示需要刷新
    fn is_auth_error(resp: &serde_json::Value) -> bool {
        resp.get("errCode")
            .and_then(|v| v.as_str())
            .map(|c| c == "1" || c == "9")
            .unwrap_or(false)
    }

    /// 发起 GET 请求，自动处理认证失败重试
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{BASE_URL}{path}");
        let ts = now_ts();

        let resp = self
            .client
            .get(&url)
            .header("token", gen_jiuyan_token(ts))
            .header("timestamp", &ts.to_string())
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {}", json_str))
    }

    /// 发起 POST 请求，自动处理认证失败重试
    pub async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        let url = format!("{BASE_URL}{path}");
        let ts = now_ts();

        let resp = self
            .client
            .post(&url)
            .header("token", gen_jiuyan_token(ts))
            .header("timestamp", &ts.to_string())
            .json(body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {}", json_str))
    }

    /// 获取韭研公社异动数据（action/field 接口）
    pub async fn fetch_field(&self, date: &str, pc: u32) -> Result<FieldData> {
        #[derive(Serialize)]
        struct FieldReq {
            date: String,
            pc: u32,
        }

        let body = FieldReq {
            date: date.to_string(),
            pc,
        };

        let resp: serde_json::Value = self.post("/action/field", &body).await?;

        // 检查业务错误
        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            if err_code != "0" {
                anyhow::bail!(
                    "action/field 请求失败: errCode={}, msg={}",
                    err_code,
                    resp["msg"]
                );
            }
        }
        let data: FieldData = serde_json::from_value(resp.get("data").cloned().unwrap_or_default())
            .context("解析 action/field 数据失败")?;

        Ok(data)
    }

    /// fetch_field with String pc — for LLM tool use
    pub async fn fetch_field_str(&self, date: &str, pc: &str) -> Result<FieldData> {
        let n: u32 = pc.parse().unwrap_or(1);
        self.fetch_field(date, n).await
    }

    /// 获取时间线数据（timeline/list 接口）
    pub async fn fetch_timeline(&self, year_month: &str) -> Result<TimelineData> {
        #[derive(Serialize)]
        struct TimelineReq {
            date: String,
        }

        let body = TimelineReq {
            date: year_month.to_string(),
        };

        let resp: serde_json::Value = self.post("/timeline/list", &body).await?;

        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            if err_code != "0" {
                anyhow::bail!(
                    "timeline/list 请求失败: errCode={}, msg={}",
                    err_code,
                    resp["msg"]
                );
            }
        }

        let data: TimelineData =
            serde_json::from_value(resp.get("data").cloned().unwrap_or_default())
                .context("解析 timeline 数据失败")?;

        Ok(data)
    }

    /// 获取社群文章数据（article/community 接口）
    pub async fn fetch_community(&self, query: CommunityQuery) -> Result<CommunityData> {
        #[derive(Serialize)]
        struct CommunityReq {
            category_id: String,
            #[serde(rename = "type")]
            r#type: i32,
            limit: i32,
            order: i32,
            start: i32,
            back_garden: i32,
        }

        let body = CommunityReq {
            category_id: query.category_id,
            r#type: query.article_type,
            limit: query.page_size,
            order: 1,
            start: query.page,
            back_garden: 0,
        };

        let resp: serde_json::Value = self.post("/article/community", &body).await?;

        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            if err_code != "0" {
                anyhow::bail!(
                    "article/community 请求失败: errCode={}, msg={}",
                    err_code,
                    resp["msg"]
                );
            }
        }

        let data: CommunityData =
            serde_json::from_value(resp.get("data").cloned().unwrap_or_default())
                .context("解析社群文章数据失败")?;

        Ok(data)
    }
}
