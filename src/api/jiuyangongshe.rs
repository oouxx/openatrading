use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

const BASE_URL: &str = "https://app.jiuyangongshe.com/jystock-app/api/v1";

fn now_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}

// ─── Token & Cookie 凭证 ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Credentials {
    pub token: String,
    /// 登录成功后，将 sessionToken 写入 Cookie 格式: `SESSION=xxx`
    pub cookie: String,
}

// ─── action/field 响应结构 ────────────────────────────────────

/// 异动详情（action_info 字段）
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct Article {
    pub action_info: Option<ActionInfo>,
    /// 文章标题
    #[serde(default)]
    pub title: String,
}

/// 单只异动股票
#[derive(Debug, Clone, Deserialize)]
pub struct FieldStock {
    pub code: String,
    pub name: String,
    pub article: Option<Article>,
}

/// 异动板块
#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineUser {
    pub user_id: String,
    pub nickname: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineTheme {
    #[serde(rename = "timeline_theme_id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct TimelineDay {
    pub date: String,
    pub list: Vec<TimelineItem>,
}

pub type TimelineData = Vec<TimelineDay>;

// ─── 社群文章数据 ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CommunityUser {
    pub user_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub style_str: Option<String>,
    #[serde(default)]
    pub medal_count: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommunityStock {
    pub stock_id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct CommunityData {
    pub result: Vec<CommunityArticle>,
    #[serde(rename = "pageNo")]
    pub page_no: i32,
    #[serde(rename = "pageSize")]
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

impl JiuYanSession {
    pub fn new() -> Self {
        let init_token = std::env::var("JIUYAN_INIT_TOKEN")
            .unwrap_or_else(|_| "3f79cb21f69cf26271919c901d749274".to_string());

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36")
            .cookie_store(true)
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert("Accept", "application/json, text/plain, */*".parse().unwrap());
                headers.insert("Origin", "https://www.jiuyangongshe.com".parse().unwrap());
                headers.insert("Referer", "https://www.jiuyangongshe.com/".parse().unwrap());
                headers.insert("platform", "3".parse().unwrap());
                headers.insert("token", init_token.parse().unwrap());
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
            }
        }

        let url = format!("{BASE_URL}/user/login");
        let body = serde_json::json!({
            "phone": phone,
            "password": password
        });

        let timestamp = now_ts();

        let resp: serde_json::Value = self
            .client
            .post(&url)
            .header("timestamp", &timestamp)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // 检查业务错误 (errCode=0 表示成功)
        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            if err_code != "0" {
                anyhow::bail!("登录失败: errCode={}, msg={}", err_code, resp["msg"]);
            }
        }

        let session_token = resp["data"]["sessionToken"]
            .as_str()
            .context("登录成功但未返回 sessionToken")?
            .to_string();

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

        println!(
            "✅ 九言公社登录成功 (token: {}...)",
            &session_token[..16.min(session_token.len())]
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
        let creds = self.get_credentials().await?;
        let url = format!("{BASE_URL}{path}");
        let ts = now_ts();

        let resp = self
            .client
            .get(&url)
            .header("token", &creds.token)
            .header("timestamp", &ts)
            .header(reqwest::header::COOKIE, &creds.cookie)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if Self::is_auth_error(&resp) {
            self.refresh().await?;
            let creds = self.get_credentials().await?;
            let ts = now_ts();

            let resp = self
                .client
                .get(&url)
                .header("token", &creds.token)
                .header("timestamp", &ts)
                .header(reqwest::header::COOKIE, &creds.cookie)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;

            let json_str = serde_json::to_string(&resp)?;
            return serde_json::from_str(&json_str)
                .with_context(|| format!("解析响应失败: {}", json_str));
        }

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {}", json_str))
    }

    /// 发起 POST 请求，自动处理认证失败重试
    pub async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        let creds = self.get_credentials().await?;
        let url = format!("{BASE_URL}{path}");
        let ts = now_ts();

        let resp = self
            .client
            .post(&url)
            .header("token", &creds.token)
            .header("timestamp", &ts)
            .header(reqwest::header::COOKIE, &creds.cookie)
            .json(body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if Self::is_auth_error(&resp) {
            self.refresh().await?;
            let creds = self.get_credentials().await?;
            let ts = now_ts();

            let resp = self
                .client
                .post(&url)
                .header("token", &creds.token)
                .header("timestamp", &ts)
                .header(reqwest::header::COOKIE, &creds.cookie)
                .json(body)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;

            let json_str = serde_json::to_string(&resp)?;
            return serde_json::from_str(&json_str)
                .with_context(|| format!("解析响应失败: {}", json_str));
        }

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {}", json_str))
    }

    /// 获取九言公社异动数据（action/field 接口）
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
    pub async fn fetch_community(
        &self,
        category_id: &str,
        article_type: i32,
        page_no: i32,
        page_size: i32,
    ) -> Result<CommunityData> {
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
            category_id: category_id.to_string(),
            r#type: article_type,
            limit: page_size,
            order: 1,
            start: page_no,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn require_env() -> bool {
        std::env::var("JIUYAN_PHONE").is_ok() && std::env::var("JIUYAN_PASSWORD").is_ok()
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_login_returns_token() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = JiuYanSession::new();
        let creds = session.login().await.expect("登录失败");

        assert!(!creds.token.is_empty(), "token 不应为空");
        assert!(
            creds.token.len() > 10,
            "token 长度异常: {}",
            creds.token.len()
        );
        assert!(
            creds.cookie.starts_with("SESSION="),
            "cookie 应以 SESSION= 开头"
        );
        println!("✅ token 获取成功: {}...", &creds.token[..20]);
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_token_valid_via_action_field() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = JiuYanSession::new();
        session.login().await.expect("登录失败");

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let body = serde_json::json!({
            "date": today,
            "pc": 1
        });
        let resp: serde_json::Value = session
            .post("/action/field", &body)
            .await
            .expect("action/field 调用失败");

        println!(
            "action/field 响应: {}",
            serde_json::to_string_pretty(&resp).unwrap()
        );

        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            assert!(
                err_code != "1" && err_code != "9",
                "token 已失效: errCode={}",
                err_code
            );
        }

        println!("✅ token 验证通过");
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_credentials_cached() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = JiuYanSession::new();
        let creds1 = session.get_credentials().await.expect("获取凭证失败");
        let creds2 = session.get_credentials().await.expect("获取凭证失败");

        assert_eq!(creds1.token, creds2.token, "缓存应返回相同 token");
        println!("✅ 凭证缓存生效，两次 token 一致");
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_refresh_token() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = JiuYanSession::new();
        let creds1 = session.login().await.expect("首次登录失败");
        let creds2 = session.refresh().await.expect("刷新失败");

        assert!(!creds2.token.is_empty(), "刷新后 token 不应为空");
        println!(
            "✅ 刷新成功: 旧token={}[..16]... 新token={}[..16]...",
            &creds1.token[..16],
            &creds2.token[..16]
        );
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_api_call_with_token() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = JiuYanSession::new();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let body = serde_json::json!({
            "date": today,
            "pc": 1
        });
        let resp: serde_json::Value = session
            .post("/action/field", &body)
            .await
            .expect("action/field 调用失败");

        if let Some(err_code) = resp.get("errCode").and_then(|v| v.as_str()) {
            panic!(
                "接口返回认证错误: errCode={}, msg={}",
                err_code, resp["msg"]
            );
        }

        println!("✅ 接口调用成功，token 有效");
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_concurrent_login_dedup() {
        dotenvy::dotenv().ok();
        if !require_env() {
            panic!("未配置 JIUYAN_PHONE / JIUYAN_PASSWORD");
        }

        let session = Arc::new(JiuYanSession::new());
        let mut handles: Vec<tokio::task::JoinHandle<Credentials>> = vec![];

        for _i in 0..5 {
            let s = session.clone();
            handles.push(tokio::spawn(
                async move { s.get_credentials().await.unwrap() },
            ));
        }

        let results: Vec<Credentials> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|h| h.unwrap())
            .collect();

        let first_token = &results[0].token;
        for (i, c) in results.iter().enumerate().skip(1) {
            assert_eq!(&c.token, first_token, "并发任务 {} token 不一致", i);
        }

        println!(
            "✅ 并发登录去重成功，{} 个任务获得相同 token",
            results.len()
        );
    }

    #[tokio::test]
    #[ignore = "需要 JIUYAN_PHONE / JIUYAN_PASSWORD"]
    async fn test_login_with_wrong_credentials() {
        dotenvy::dotenv().ok();

        std::env::set_var("JIUYAN_PHONE", "13800000000");
        std::env::set_var("JIUYAN_PASSWORD", "wrong_pass_12345");

        let session = JiuYanSession::new();
        let result = session.login().await;

        assert!(result.is_err(), "使用错误账号登录应失败");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("errCode") || err.contains("登录失败"),
            "错误信息应包含业务错误码: {}",
            err
        );

        println!("✅ 错误账号登录失败校验通过: {}", err);

        std::env::remove_var("JIUYAN_PHONE");
        std::env::remove_var("JIUYAN_PASSWORD");
    }

    #[test]
    fn test_field_data_parsing_from_json() {
        let json_str =
            std::fs::read_to_string("jiuyan_res.json").expect("Failed to read jiuyan_res.json");
        let resp: serde_json::Value =
            serde_json::from_str(&json_str).expect("Failed to parse JSON");

        // 验证顶层结构
        assert_eq!(resp["errCode"].as_str().unwrap(), "0");

        let data: FieldData = serde_json::from_value(resp["data"].clone())
            .expect("Failed to parse data into FieldData");

        assert!(!data.is_empty(), "板块列表不应为空");

        // 验证第一个非空板块（跳过"简图"）
        let first_real_plate = data.iter().find(|p| !p.list.is_empty()).unwrap();

        // 验证板块基本字段
        assert_eq!(first_real_plate.name, "公告");
        assert_eq!(first_real_plate.count, 12);
        assert!(!first_real_plate.list.is_empty());

        // 验证第一只股票
        let first_stock = &first_real_plate.list[0];
        assert_eq!(first_stock.code, "sh600743");
        assert_eq!(first_stock.name, "华远控股");

        // 验证 article 和 action_info
        let article = first_stock.article.as_ref().expect("article 不应为空");
        let action_info = article.action_info.as_ref().expect("action_info 不应为空");
        assert_eq!(action_info.time.as_deref().unwrap(), "09:35:38");
        assert_eq!(action_info.num.as_deref().unwrap(), "4天4板");
        assert!(action_info
            .expound
            .as_deref()
            .unwrap()
            .contains("实控人筹划重组"));

        // 验证带 reason 的板块
        let solid_state_plate = data.iter().find(|p| p.name == "固态电池").unwrap();
        assert!(solid_state_plate.reason.contains("清陶能源"));
        assert_eq!(solid_state_plate.count, 9);

        println!("✅ 数据结构校验通过");
    }
}
