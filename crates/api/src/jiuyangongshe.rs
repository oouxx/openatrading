use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::utils::{gen_jiuyan_token, now_ts};

const BASE_URL: &str = "https://app.jiuyangongshe.com/jystock-app/api/v1";

// ─── Token & Cookie 凭证 ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Credentials {
    pub token: String,
    /// 登录成功后，将 sessionToken 写入 Cookie 格式: `SESSION=xxx`
    pub cookie: String,
}

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
    /// Create a new `JiuYanSession`.
    ///
    /// # Panics
    /// Panics if the reqwest client cannot be built or default headers are invalid (should never happen).
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
                    "application/json, text/plain, */*"
                        .parse()
                        .unwrap_or_else(|e| panic!("hardcoded header value is valid: {e}")),
                );
                headers.insert(
                    "Origin",
                    "https://www.jiuyangongshe.com"
                        .parse()
                        .unwrap_or_else(|e| panic!("hardcoded header value is valid: {e}")),
                );
                headers.insert(
                    "Referer",
                    "https://www.jiuyangongshe.com/"
                        .parse()
                        .unwrap_or_else(|e| panic!("hardcoded header value is valid: {e}")),
                );
                headers.insert(
                    "platform",
                    "3".parse()
                        .unwrap_or_else(|e| panic!("hardcoded header value is valid: {e}")),
                );
                headers
            })
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"));

        Self {
            inner: RwLock::new(InnerSession {
                credentials: None,
                refreshing: AtomicBool::new(false),
            }),
            client,
        }
    }

    /// 获取凭证，如果已登录且未过期则直接返回
    ///
    /// # Errors
    /// Returns an error if login fails or environment variables are missing.
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
    ///
    /// # Errors
    /// Returns an error if environment variables are missing or the login request fails.
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
            // 必须等对方登录“结束”后再读 credentials：旧凭证在登录成功前仍然保留，
            // 先读到的是过期的那份，拿去重试只会再吃一次「登录失效」。
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let guard = self.inner.read().await;
                if guard.refreshing.load(Ordering::SeqCst) {
                    continue;
                }
                if let Some(creds) = &guard.credentials {
                    return Ok(creds.clone());
                }
                anyhow::bail!("login failed: another login attempt finished without credentials");
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
            .header("token", gen_jiuyan_token(ts)?)
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
                }
            },
            Err(e) => {
                self.inner
                    .write()
                    .await
                    .refreshing
                    .store(false, Ordering::SeqCst);
                return Err(e.into());
            }
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

        let session_token = if let Some(s) = resp["data"]["sessionToken"].as_str() {
            s.to_string()
        } else {
            self.inner
                .write()
                .await
                .refreshing
                .store(false, Ordering::SeqCst);
            anyhow::bail!("登录成功但未返回 sessionToken");
        };

        // 将 sessionToken 写入 Cookie 格式
        let cookie = format!("SESSION={session_token}");

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

    /// 强制重新登录
    ///
    /// # Errors
    /// Returns an error if login fails.
    ///
    /// 不先清空 `credentials`：`login()` 自己会覆盖它。若两个请求同时
    /// 发现 session 失效，先到的登录进行中，后到的会走 `login()` 的
    /// `refreshing` 等待分支复用同一份新凭证，不会把刚拿到的 token 清掉。
    pub async fn refresh(&self) -> Result<Credentials> {
        self.login().await
    }

    /// 判断响应是否表示 session 失效（需要重新登录）。
    fn is_auth_expired(resp: &serde_json::Value) -> bool {
        let code = resp.get("errCode").and_then(serde_json::Value::as_str);
        if matches!(code, Some("1" | "401")) {
            return true;
        }
        resp.get("msg")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|msg| msg.contains("登录失效"))
    }

    /// 发送一次带签名和 Cookie 的请求，返回原始 JSON。
    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let creds = self.get_credentials().await?;
        let url = format!("{BASE_URL}{path}");
        let ts = now_ts();

        let mut request = self
            .client
            .request(method, &url)
            .header("token", gen_jiuyan_token(ts)?)
            .header("timestamp", ts.to_string())
            .header(reqwest::header::COOKIE, &creds.cookie);
        if let Some(body) = body {
            request = request.json(body);
        }

        Ok(request.send().await?.json::<serde_json::Value>().await?)
    }

    /// 发送请求；若 session 失效则重新登录后重试一次。
    async fn send_with_auth_retry(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let resp = self.send_json(method.clone(), path, body).await?;
        if !Self::is_auth_expired(&resp) {
            return Ok(resp);
        }

        tracing::info!(path, "jiuyangongshe session expired, re-login and retry");
        self.refresh().await?;
        self.send_json(method, path, body).await
    }

    /// 发起 GET 请求，MD5 签名 + Session Cookie 认证
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let resp = self
            .send_with_auth_retry(reqwest::Method::GET, path, None)
            .await?;

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {json_str}"))
    }

    /// 发起 POST 请求，MD5 签名 + Session Cookie 认证
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn post<T: for<'de> Deserialize<'de>, B: Serialize + Send + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_value(body)?;
        let resp = self
            .send_with_auth_retry(reqwest::Method::POST, path, Some(&body))
            .await?;

        let json_str = serde_json::to_string(&resp)?;
        serde_json::from_str(&json_str).with_context(|| format!("解析响应失败: {json_str}"))
    }

    /// 获取时间线数据（timeline/list 接口）
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
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
}
