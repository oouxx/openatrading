//! 雪球（xueqiu.com）数据源客户端。
//!
//! 接口与字段依据 `xueqiu_rea/xueqiu_api.md` 的逆向记录实现：
//! - 响应结构已被观察的接口返回强类型模型；
//! - 结构尚未验证的接口原样返回 [`serde_json::Value`]，避免臆造字段。
//!
//! 雪球对自动化访问有 WAF 与频率限制，且大多数接口需要浏览器会话 Cookie。
//! 使用 [`XueqiuClient::with_cookie`] 或 [`XueqiuClient::with_token`] 注入凭据；
//! 没有凭据时只能访问少数匿名接口。
//!
//! 本客户端只保留无需账号登录的接口。关注时间线、自选股、财务/F10/资金、
//! 组合等需要登录会话的接口未实现。
//!
//! 所有 `.json` 接口优先走 `api.xueqiu.com`；同名路径在 `xueqiu.com` 上可能
//! 触发反爬挑战（实测 2026-11-11）。`/v5/stock/*` 走 `stock.xueqiu.com`。

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::time::Duration;

use crate::utils;

const STOCK_BASE: &str = "https://stock.xueqiu.com";
const API_BASE: &str = "https://api.xueqiu.com";
const WEB_BASE: &str = "https://xueqiu.com";
const FUND_BASE: &str = "https://danjuanapp.com";

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// K 线默认指标组合，来自 pysnowball 实际请求。
const KLINE_INDICATOR: &str = "kline,pe,pb,ps,pcf,market_capital,agt,ggt,balance";
/// 用户动态默认类型：全部动态。
const DEFAULT_TIMELINE_TYPE: i32 = 10;

// ── 数据模型 ──────────────────────────────────────────

/// 雪球用户。
///
/// 仅声明首页 SSR 与接口中实际观察到的字段，其余字段保留在 `extra` 中。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub screen_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub profile_image_url: Option<String>,
    #[serde(default)]
    pub photo_domain: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub blog_description: Option<String>,
    #[serde(default)]
    pub followers_count: Option<i64>,
    #[serde(default)]
    pub friends_count: Option<i64>,
    #[serde(default)]
    pub status_count: Option<i64>,
    #[serde(default)]
    pub stocks_count: Option<i64>,
    #[serde(default)]
    pub stock_status_count: Option<i64>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub verified_type: Option<i64>,
    #[serde(default)]
    pub verified_description: Option<String>,
    #[serde(default)]
    pub verified_infos: Option<Value>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub province: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub follow_me: Option<bool>,
    #[serde(default)]
    pub following: Option<bool>,
    #[serde(default)]
    pub blocking: Option<bool>,
    #[serde(default)]
    pub recommend: Option<bool>,
    #[serde(default)]
    pub recommend_reason: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub live_info: Option<Value>,
    #[serde(default)]
    pub name_pinyin: Option<String>,
    #[serde(default)]
    pub screenname_pinyin: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// 雪球动态 / 帖子。
///
/// 该结构同时覆盖用户动态、关注时间线、个股讨论、资讯、热帖和动态详情；
/// 不同接口只返回其中一部分字段，因此所有字段都是可选的。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "rawTitle")]
    pub raw_title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_link: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub edited_at: Option<i64>,
    #[serde(default)]
    pub reply_count: Option<i64>,
    #[serde(default)]
    pub retweet_count: Option<i64>,
    #[serde(default)]
    pub fav_count: Option<i64>,
    #[serde(default)]
    pub like_count: Option<i64>,
    #[serde(default)]
    pub view_count: Option<i64>,
    /// 1 表示置顶动态。
    #[serde(default)]
    pub mark: Option<i64>,
    #[serde(default)]
    pub legal_user_visible: Option<bool>,
    #[serde(default)]
    pub truncated: Option<bool>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub retweeted_status: Option<Box<Status>>,
    #[serde(default, rename = "stockCorrelation")]
    pub stock_correlation: Option<Vec<String>>,
    #[serde(default)]
    pub symbol_id: Option<i64>,
    #[serde(default)]
    pub image_info_list: Option<Vec<ImageInfo>>,
    #[serde(default)]
    pub quote_cards: Option<Vec<QuoteCard>>,
    #[serde(default)]
    pub card: Option<Value>,
    #[serde(default, rename = "tagsForWeb")]
    pub tags_for_web: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageInfo {
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuoteCard {
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// 带总数的列表响应，如用户专栏时间线。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusPage {
    pub list: Vec<Status>,
    pub total: Option<i64>,
}

/// 蛋卷基金基本信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FundInfo {
    #[serde(default)]
    pub fd_full_name: Option<String>,
    #[serde(default)]
    pub fd_name: Option<String>,
    #[serde(default)]
    pub fd_code: Option<String>,
    #[serde(default)]
    pub fund_derived: Option<FundDerived>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FundDerived {
    #[serde(default)]
    pub unit_nav: Option<String>,
    #[serde(default)]
    pub nav_grtd: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// K 线请求。默认值与文档中验证过的组合一致：日线、向前 284 根。
#[derive(Debug, Clone)]
pub struct KlineRequest {
    pub symbol: String,
    /// 起始时间，毫秒时间戳。
    pub begin_ms: i64,
    pub period: String,
    /// 负数表示向前取数量，如 `-284`。
    pub count: i64,
}

impl KlineRequest {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            begin_ms: utils::now_ts(),
            period: "day".to_string(),
            count: -284,
        }
    }
}

// ── 上游响应包装 ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListResponse {
    #[serde(default)]
    list: Vec<Status>,
}

#[derive(Debug, Deserialize)]
struct UserTimelineResponse {
    #[serde(default)]
    statuses: Vec<Status>,
}

#[derive(Debug, Deserialize)]
struct HotListV2Response {
    #[serde(default)]
    items: Vec<HotListItem>,
}

#[derive(Debug, Deserialize)]
struct HotListItem {
    #[serde(default)]
    original_status: Option<Status>,
}

#[derive(Debug, Deserialize)]
struct OriginalTimelineResponse {
    #[serde(default)]
    list: Vec<Status>,
    #[serde(default)]
    total: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UserShowResponse {
    user: User,
}

#[derive(Debug, Deserialize)]
struct FundResponse {
    data: FundInfo,
}

// ── 客户端 ────────────────────────────────────────────

/// 雪球 HTTP 客户端。
///
/// 默认匿名；通过 [`XueqiuClient::with_cookie`] 或 [`XueqiuClient::with_token`]
/// 注入浏览器会话 Cookie。
#[derive(Clone)]
pub struct XueqiuClient {
    client: reqwest::Client,
    cookie: Option<String>,
}

impl Default for XueqiuClient {
    fn default() -> Self {
        Self::new()
    }
}

impl XueqiuClient {
    /// 创建匿名客户端。雪球多数接口会拒绝没有 Cookie 的请求。
    ///
    /// # Panics
    /// 仅在 reqwest 客户端构建失败时 panic；默认参数下不会发生。
    pub fn new() -> Self {
        Self::build(None)
    }

    /// 使用浏览器会话 Cookie 创建客户端。
    ///
    /// `cookie` 是完整的 `Cookie` 头，例如
    /// `xq_a_token=...; xq_r_token=...; u=...`。
    ///
    /// # Panics
    /// 仅在 reqwest 客户端构建失败时 panic；默认参数下不会发生。
    pub fn with_cookie(cookie: impl Into<String>) -> Self {
        Self::build(Some(cookie.into()))
    }

    /// 使用 `xq_a_token` 创建客户端，并同步写入 `xqat` 副本。
    ///
    /// # Panics
    /// 仅在 reqwest 客户端构建失败时 panic；默认参数下不会发生。
    pub fn with_token(token: impl Into<String>) -> Self {
        let token = token.into();
        Self::with_cookie(format!("xq_a_token={token}; xqat={token}"))
    }

    fn build(cookie: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"));
        Self { client, cookie }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        base: &str,
        path: &str,
        params: &[(&str, String)],
        referer: Option<&str>,
    ) -> Result<T> {
        let url = format!("{base}{path}");
        let mut request = self
            .client
            .get(&url)
            .query(params)
            .header("Accept", "application/json");

        if let Some(cookie) = &self.cookie {
            request = request.header("Cookie", cookie);
        }
        if let Some(referer) = referer {
            request = request.header("Referer", referer);
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("请求雪球接口失败: {url}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("读取雪球响应失败: {url}"))?;

        decode_json(status, path, &body)
    }

    async fn stock(
        &self,
        path: &str,
        params: &[(&str, String)],
        referer: Option<&str>,
    ) -> Result<Value> {
        self.get::<Value>(STOCK_BASE, path, params, referer).await
    }

    async fn api(
        &self,
        path: &str,
        params: &[(&str, String)],
        referer: Option<&str>,
    ) -> Result<Value> {
        self.get::<Value>(API_BASE, path, params, referer).await
    }

    async fn fund(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        self.get::<Value>(FUND_BASE, path, params, None).await
    }
}

/// 校验上游响应，并把 JSON 解析为指定类型。
///
/// 雪球限频时可能返回 HTTP 200 + 空正文；WAF 拦截时返回 HTML 而非 JSON。
/// 这两种情况都必须转成显式错误，不能当成「无数据」。
fn decode_json<T: serde::de::DeserializeOwned>(
    status: reqwest::StatusCode,
    path: &str,
    body: &str,
) -> Result<T> {
    if body.trim().is_empty() {
        if status.is_success() {
            bail!("雪球接口 {path} 返回空响应（可能触发限频或接口已失效）");
        }
        bail!("雪球接口 {path} 返回 HTTP {status}");
    }
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(error) if status.is_success() => {
            return Err(anyhow::Error::new(error).context(format!(
                "雪球接口 {path} 响应不是合法 JSON（可能被 WAF 拦截）"
            )));
        }
        Err(_) => bail!("雪球接口 {path} 返回 HTTP {status}"),
    };

    // 雪球在业务失败时仍可能返回 HTTP 200，用 error_code 或 result_code
    // 表示错误；例如动态已删除返回 20210，未登录返回字符串 "400016"。
    // 这类响应必须转成带上游描述的显式错误，不能解析成空对象。
    if let Some((code, message)) = business_error(&value) {
        bail!("雪球接口 {path} 返回错误码 {code}: {message}");
    }

    if !status.is_success() {
        bail!("雪球接口 {path} 返回 HTTP {status}");
    }

    serde_json::from_value(value).with_context(|| format!("解析雪球接口 {path} 响应失败"))
}

fn business_error(value: &Value) -> Option<(i64, String)> {
    for key in ["error_code", "result_code"] {
        let Some(code) = integer_field(value, key) else {
            continue;
        };
        if code == 0 {
            continue;
        }
        let message = value
            .get("error_description")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Some((code, message.to_string()));
    }
    None
}

fn integer_field(value: &Value, key: &str) -> Option<i64> {
    let field = value.get(key)?;
    field
        .as_i64()
        .or_else(|| field.as_str().and_then(|field| field.parse().ok()))
}

// ── 代码规范化 ────────────────────────────────────────

/// 把常见股票代码规范化为雪球格式（`SH600519` / `SZ000002` / `BJ920001`）。
///
/// 支持 `SH600519`、`sh.600519`、`600519.SS`、`600519` 等写法。
///
/// # Errors
/// 输入为空、位数超过 6 位、无法推断交易所，或不是沪深/北交所 A 股时返回错误。
pub fn to_xueqiu_symbol(input: &str) -> Result<String> {
    let raw = input.trim();
    if raw.is_empty() {
        bail!("股票代码不能为空");
    }
    let upper = raw.to_uppercase();

    // 已带交易所前缀：SH600519 / sh.600519 / BJ920001
    for prefix in ["SH", "SZ", "BJ"] {
        if let Some(rest) = upper.strip_prefix(prefix) {
            return Ok(format!("{prefix}{}", normalize_digits(rest)?));
        }
    }

    // Yahoo 风格后缀：600519.SS / 000001.SZ
    if let Some((code, suffix)) = upper.rsplit_once('.') {
        let prefix = match suffix {
            "SS" => "SH",
            "SZ" => "SZ",
            _ => bail!("雪球客户端仅支持沪深/北交所 A 股代码，收到: {input}"),
        };
        return Ok(format!("{prefix}{}", normalize_digits(code)?));
    }

    // 纯数字：复用项目既有的交易所推断
    let yahoo = utils::to_yahoo_symbol(raw)?;
    let (code, suffix) = yahoo
        .split_once('.')
        .with_context(|| format!("无法识别股票代码: {input}"))?;
    let prefix = match suffix {
        "SS" => "SH",
        "SZ" => "SZ",
        "BJ" => "BJ",
        _ => bail!("雪球客户端仅支持沪深/北交所 A 股代码，收到: {input}"),
    };
    Ok(format!("{prefix}{code}"))
}

fn normalize_digits(raw: &str) -> Result<String> {
    let cleaned = raw.trim_start_matches(['.', '-', '_']);
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_digit()) {
        bail!("股票代码格式不正确: {raw}");
    }
    if cleaned.len() > 6 {
        bail!("股票代码位数超过 6 位: {raw}");
    }
    Ok(format!("{cleaned:0>6}"))
}

// ── 行情与 K 线 ───────────────────────────────────────

impl XueqiuClient {
    /// 获取个股实时报价，返回上游 `data.quote` 所在的完整 JSON。
    ///
    /// 文档只验证了 `data.quote.name`；其余字段结构未确认，因此原样返回。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn quote(&self, symbol: &str) -> Result<Value> {
        self.quote_with_extend(symbol, None).await
    }

    /// 获取详细报价（`extend=detail`）。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn quote_detail(&self, symbol: &str) -> Result<Value> {
        self.quote_with_extend(symbol, Some("detail")).await
    }

    async fn quote_with_extend(&self, symbol: &str, extend: Option<&str>) -> Result<Value> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let mut params = vec![("symbol", symbol.clone())];
        if let Some(extend) = extend {
            params.push(("extend", extend.to_string()));
        }
        let referer = format!("{WEB_BASE}/S/{symbol}");
        self.stock("/v5/stock/quote.json", &params, Some(&referer))
            .await
    }

    /// 获取实时行情（quotec）。响应结构未验证，原样返回。
    ///
    /// 实测 2026-11-11 上游持续返回 HTTP 200 + 空正文，接口当前不可用。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn quotec(&self, symbol: &str) -> Result<Value> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        self.stock(
            "/v5/stock/realtime/quotec.json",
            &[("symbol", symbol)],
            Some(&referer),
        )
        .await
    }

    /// 获取盘口。响应结构未验证，原样返回。
    ///
    /// 实测 2026-11-11 上游持续返回 HTTP 200 + 空正文，接口当前不可用。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn pankou(&self, symbol: &str) -> Result<Value> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        self.stock(
            "/v5/stock/realtime/pankou.json",
            &[("symbol", symbol)],
            Some(&referer),
        )
        .await
    }

    /// 获取 K 线。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn kline(&self, request: &KlineRequest) -> Result<Value> {
        let symbol = to_xueqiu_symbol(&request.symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        self.stock(
            "/v5/stock/chart/kline.json",
            &[
                ("symbol", symbol),
                ("begin", request.begin_ms.to_string()),
                ("period", request.period.clone()),
                ("type", "before".to_string()),
                ("count", request.count.to_string()),
                ("indicator", KLINE_INDICATOR.to_string()),
            ],
            Some(&referer),
        )
        .await
    }

    /// 股票搜索联想。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn suggest_stock(&self, query: &str) -> Result<Value> {
        self.api(
            "/query/v1/suggest_stock.json",
            &[("q", query.to_string())],
            None,
        )
        .await
    }
}

// ── 个股内容与讨论 ────────────────────────────────────

impl XueqiuClient {
    /// 获取个股讨论。`source` 取 `all` / `user`（讨论）或 `trans`（交易）。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn symbol_status(
        &self,
        symbol: &str,
        source: &str,
        count: u32,
        page: u32,
    ) -> Result<Vec<Status>> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        let response: ListResponse = self
            .get(
                API_BASE,
                "/query/v1/symbol/search/status.json",
                &[
                    ("symbol_id", symbol.clone()),
                    ("symbol", symbol),
                    ("source", source.to_string()),
                    ("count", count.to_string()),
                    ("page", page.to_string()),
                    ("sort", "time".to_string()),
                    ("comment", "0".to_string()),
                    ("hl", "0".to_string()),
                ],
                Some(&referer),
            )
            .await?;
        Ok(response.list)
    }

    /// 获取个股资讯或公告。`source` 取 `自选股新闻` 或 `公告`。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn stock_timeline(
        &self,
        symbol: &str,
        source: &str,
        count: u32,
        page: u32,
    ) -> Result<Vec<Status>> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        let response: ListResponse = self
            .get(
                API_BASE,
                "/statuses/stock_timeline.json",
                &[
                    ("symbol_id", symbol.clone()),
                    ("symbol", symbol),
                    ("source", source.to_string()),
                    ("count", count.to_string()),
                    ("page", page.to_string()),
                    ("sort", "alpha".to_string()),
                    ("comment", "0".to_string()),
                    ("hl", "0".to_string()),
                ],
                Some(&referer),
            )
            .await?;
        Ok(response.list)
    }

    /// 获取个股评论。该接口在上游没有 `.json` 后缀。
    ///
    /// # Errors
    /// 股票代码非法、请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn stock_comments(&self, symbol: &str, count: u32) -> Result<Vec<Status>> {
        let symbol = to_xueqiu_symbol(symbol)?;
        let referer = format!("{WEB_BASE}/S/{symbol}");
        let response: ListResponse = self
            .get(
                API_BASE,
                "/query/v1/symbol/search/status",
                &[
                    ("u", "11111".to_string()),
                    ("count", count.to_string()),
                    ("comment", "0".to_string()),
                    ("symbol", symbol),
                    ("source", "all".to_string()),
                    ("sort", "time".to_string()),
                ],
                Some(&referer),
            )
            .await?;
        Ok(response.list)
    }
}

// ── 社区信息流与动态 ──────────────────────────────────

impl XueqiuClient {
    /// 获取热帖。响应顶层直接是数组。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn hots(&self, count: u32, page: u32) -> Result<Vec<Status>> {
        let referer = format!("{WEB_BASE}/");
        self.get(
            API_BASE,
            "/statuses/hots.json",
            &[
                ("a", "1".to_string()),
                ("count", count.to_string()),
                ("page", page.to_string()),
                ("scope", "day".to_string()),
                ("type", "status".to_string()),
                ("meigu", "0".to_string()),
            ],
            Some(&referer),
        )
        .await
    }

    /// 获取今日话题 / 热榜 V2。
    ///
    /// 上游把动态包在 `items[].original_status` 中；没有该字段的条目会被跳过。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn hot_list_v2(&self, since_id: i64, size: u32) -> Result<Vec<Status>> {
        let referer = format!("{WEB_BASE}/");
        let response: HotListV2Response = self
            .get(
                API_BASE,
                "/statuses/hot/listV2.json",
                &[
                    ("since_id", since_id.to_string()),
                    ("size", size.to_string()),
                ],
                Some(&referer),
            )
            .await?;
        Ok(response
            .items
            .into_iter()
            .filter_map(|item| item.original_status)
            .collect())
    }

    /// 获取用户动态（默认全部类型）。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn user_timeline(&self, user_id: &str) -> Result<Vec<Status>> {
        self.user_timeline_by_type(user_id, DEFAULT_TIMELINE_TYPE)
            .await
    }

    /// 按类型获取用户动态。
    ///
    /// `timeline_type` 的已验证取值：`0` 原发布、`2` 长文、`4` 问答、
    /// `9` 热门、`10` 默认、`11` 交易。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn user_timeline_by_type(
        &self,
        user_id: &str,
        timeline_type: i32,
    ) -> Result<Vec<Status>> {
        let mut params = vec![
            ("user_id", user_id.to_string()),
            ("type", timeline_type.to_string()),
        ];
        if timeline_type == 11 {
            params.push(("source", "买卖".to_string()));
        }
        let referer = format!("{WEB_BASE}/u/{user_id}");
        let response: UserTimelineResponse = self
            .get(
                API_BASE,
                "/v4/statuses/user_timeline.json",
                &params,
                Some(&referer),
            )
            .await?;
        Ok(response.statuses)
    }

    /// 获取动态 / 文章详情。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn status_show(&self, id: &str) -> Result<Status> {
        let referer = format!("{WEB_BASE}/");
        self.get(
            API_BASE,
            "/statuses/show.json",
            &[("id", id.to_string())],
            Some(&referer),
        )
        .await
    }

    /// 获取用户专栏时间线。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn original_timeline(&self, user_id: &str, page: u32) -> Result<StatusPage> {
        let referer = format!("{WEB_BASE}/{user_id}/column");
        let response: OriginalTimelineResponse = self
            .get(
                API_BASE,
                "/statuses/original/timeline.json",
                &[("user_id", user_id.to_string()), ("page", page.to_string())],
                Some(&referer),
            )
            .await?;
        Ok(StatusPage {
            list: response.list,
            total: response.total,
        })
    }

    /// 获取用户信息（动态页接口）。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn user_profile(&self, user_id: &str) -> Result<User> {
        let referer = format!("{WEB_BASE}/u/{user_id}");
        let response: UserShowResponse = self
            .get(
                API_BASE,
                "/statuses/original/show.json",
                &[("user_id", user_id.to_string())],
                Some(&referer),
            )
            .await?;
        Ok(response.user)
    }

    /// 获取用户收藏。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn favorites(&self, user_id: &str) -> Result<Vec<Status>> {
        let referer = format!("{WEB_BASE}/u/{user_id}");
        let response: ListResponse = self
            .get(
                API_BASE,
                "/favorites.json",
                &[("userid", user_id.to_string())],
                Some(&referer),
            )
            .await?;
        Ok(response.list)
    }
}

// ── 基金（蛋卷） ──────────────────────────────────────

impl XueqiuClient {
    /// 获取基金基本信息。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_info(&self, code: &str) -> Result<FundInfo> {
        let response: FundResponse = self
            .get(FUND_BASE, &format!("/djapi/fund/{code}"), &[], None)
            .await?;
        Ok(response.data)
    }

    /// 获取基金详情。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_detail(&self, code: &str) -> Result<Value> {
        self.fund(&format!("/djapi/fund/detail/{code}"), &[]).await
    }

    /// 获取阶段涨幅。`day` 如 `ty`。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_growth(&self, code: &str, day: &str) -> Result<Value> {
        self.fund(
            &format!("/djapi/fund/growth/{code}"),
            &[("day", day.to_string())],
        )
        .await
    }

    /// 获取净值历史。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_nav_history(&self, code: &str, page: u32, size: u32) -> Result<Value> {
        self.fund(
            &format!("/djapi/fund/nav/history/{code}"),
            &[("page", page.to_string()), ("size", size.to_string())],
        )
        .await
    }

    /// 获取基金衍生指标。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_derived(&self, code: &str) -> Result<Value> {
        self.fund(&format!("/djapi/fund/derived/{code}"), &[]).await
    }

    /// 获取基金业绩。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_achievement(&self, code: &str) -> Result<Value> {
        self.fund(&format!("/djapi/fundx/base/fund/achievement/{code}"), &[])
            .await
    }

    /// 获取基金资产配置。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_asset_percent(&self, code: &str) -> Result<Value> {
        self.fund(
            "/djapi/fundx/base/fund/record/asset/percent",
            &[("fund_code", code.to_string())],
        )
        .await
    }

    /// 获取基金经理列表。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_managers(&self, code: &str) -> Result<Value> {
        self.fund(
            "/djapi/fundx/base/fund/record/manager/list",
            &[
                ("fund_code", code.to_string()),
                ("post_status", "1".to_string()),
            ],
        )
        .await
    }

    /// 获取基金交易日期。响应结构未验证，原样返回。
    ///
    /// # Errors
    /// 请求失败、空响应或响应不是 JSON 时返回错误。
    pub async fn fund_trade_date(&self, code: &str) -> Result<Value> {
        self.fund(
            "/djapi/fund/order/v2/trade_date",
            &[("fd_code", code.to_string())],
        )
        .await
    }
}

// ── 测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse<T: serde::de::DeserializeOwned>(body: &str) -> T {
        serde_json::from_str(body).unwrap_or_else(|error| panic!("fixture should parse: {error}"))
    }

    #[test]
    fn normalizes_xueqiu_symbols() {
        assert_eq!(
            to_xueqiu_symbol("SH600519").unwrap_or_else(|e| panic!("{e}")),
            "SH600519"
        );
        assert_eq!(
            to_xueqiu_symbol("sh600519").unwrap_or_else(|e| panic!("{e}")),
            "SH600519"
        );
        assert_eq!(
            to_xueqiu_symbol("sh.600519").unwrap_or_else(|e| panic!("{e}")),
            "SH600519"
        );
        assert_eq!(
            to_xueqiu_symbol("600519").unwrap_or_else(|e| panic!("{e}")),
            "SH600519"
        );
        assert_eq!(
            to_xueqiu_symbol("600519.SS").unwrap_or_else(|e| panic!("{e}")),
            "SH600519"
        );
        assert_eq!(
            to_xueqiu_symbol("000001.SZ").unwrap_or_else(|e| panic!("{e}")),
            "SZ000001"
        );
        assert_eq!(
            to_xueqiu_symbol("BJ920001").unwrap_or_else(|e| panic!("{e}")),
            "BJ920001"
        );
        assert_eq!(
            to_xueqiu_symbol("920001").unwrap_or_else(|e| panic!("{e}")),
            "BJ920001"
        );
    }

    #[test]
    fn rejects_unsupported_or_malformed_symbols() {
        assert!(to_xueqiu_symbol("").is_err());
        assert!(to_xueqiu_symbol("00700.HK").is_err());
        assert!(to_xueqiu_symbol("SH600519.HK").is_err());
        assert!(to_xueqiu_symbol("SH1234567").is_err());
    }

    #[test]
    fn token_builds_access_cookie() {
        let client = XueqiuClient::with_token("abc");
        assert_eq!(client.cookie.as_deref(), Some("xq_a_token=abc; xqat=abc"));
    }

    #[test]
    fn kline_defaults_match_documented_request() {
        let request = KlineRequest::new("SH600519");
        assert_eq!(request.symbol, "SH600519");
        assert_eq!(request.period, "day");
        assert_eq!(request.count, -284);
        assert!(request.begin_ms > 0);
    }

    #[test]
    fn parses_user_timeline_statuses() {
        let body = r#"{
            "statuses": [{
                "id": 273201573,
                "target": "/9210717241/273201573",
                "created_at": 1700000000000,
                "title": "标题",
                "text": "正文",
                "description": "摘要",
                "mark": 5,
                "legal_user_visible": false,
                "truncated": false,
                "user": { "screen_name": "张三", "profile_image_url": "//x/avatar.png" },
                "image_info_list": [{ "filename": "a.png" }],
                "retweeted_status": { "text": "转发内容" }
            }]
        }"#;
        let response: UserTimelineResponse = parse(body);
        assert_eq!(response.statuses.len(), 1);
        let status = &response.statuses[0];
        assert_eq!(status.id, Some(273_201_573));
        assert_eq!(status.mark, Some(5));
        assert_eq!(
            status
                .user
                .as_ref()
                .and_then(|user| user.screen_name.as_deref()),
            Some("张三")
        );
        assert_eq!(
            status
                .retweeted_status
                .as_ref()
                .and_then(|status| status.text.as_deref()),
            Some("转发内容")
        );
        assert_eq!(
            status
                .image_info_list
                .as_ref()
                .and_then(|list| list.first())
                .and_then(|info| info.filename.as_deref()),
            Some("a.png")
        );
    }

    #[test]
    fn hot_list_v2_skips_items_without_original_status() {
        let body = json!({
            "items": [
                { "original_status": { "title": "热帖", "user": { "screen_name": "李四" } } },
                {},
                { "original_status": null }
            ]
        })
        .to_string();
        let response: HotListV2Response = parse(&body);
        let statuses: Vec<Status> = response
            .items
            .into_iter()
            .filter_map(|item| item.original_status)
            .collect();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].title.as_deref(), Some("热帖"));
    }

    #[test]
    fn parses_symbol_status_list() {
        let body = json!({
            "list": [{
                "title": "讨论标题",
                "description": "讨论摘要",
                "created_at": 1_700_000_000_000_i64,
                "target": "/1/2",
                "quote_cards": [{ "target_url": "https://xueqiu.com/S/SH600519" }],
                "user": { "id": 123, "screen_name": "王五" },
                "source": "雪球"
            }]
        })
        .to_string();
        let response: ListResponse = parse(&body);
        assert_eq!(response.list.len(), 1);
        assert_eq!(response.list[0].title.as_deref(), Some("讨论标题"));
        assert_eq!(
            response.list[0]
                .quote_cards
                .as_ref()
                .and_then(|cards| cards.first())
                .and_then(|card| card.target_url.as_deref()),
            Some("https://xueqiu.com/S/SH600519")
        );
    }

    #[test]
    fn parses_original_timeline_with_total() {
        let body = json!({
            "list": [{ "title": "专栏", "target": "/1/3" }],
            "total": 100
        })
        .to_string();
        let response: OriginalTimelineResponse = parse(&body);
        assert_eq!(response.list.len(), 1);
        assert_eq!(response.total, Some(100));
    }

    #[test]
    fn parses_status_show_retweeted_status() {
        let body = json!({
            "id": 409443228,
            "text": "原文",
            "user": { "screen_name": "赵六" },
            "retweeted_status": { "user": { "screen_name": "钱七" }, "text": "被转发" }
        })
        .to_string();
        let status: Status = parse(&body);
        assert_eq!(status.id, Some(409_443_228));
        assert_eq!(
            status
                .retweeted_status
                .as_ref()
                .and_then(|retweeted| retweeted.user.as_ref())
                .and_then(|user| user.screen_name.as_deref()),
            Some("钱七")
        );
    }

    #[test]
    fn parses_fund_info() {
        let body = json!({
            "data": {
                "fd_full_name": "工银瑞信稳健成长混合A",
                "fd_name": "工银稳健成长混合A",
                "fd_code": "481004",
                "fund_derived": {
                    "unit_nav": "1.2345",
                    "nav_grtd": "0.12",
                    "end_date": "2026-11-10"
                }
            }
        })
        .to_string();
        let response: FundResponse = parse(&body);
        assert_eq!(response.data.fd_code.as_deref(), Some("481004"));
        let derived = response
            .data
            .fund_derived
            .as_ref()
            .unwrap_or_else(|| panic!("fund_derived should exist"));
        assert_eq!(derived.unit_nav.as_deref(), Some("1.2345"));
        assert_eq!(derived.end_date.as_deref(), Some("2026-11-10"));
    }

    #[test]
    fn empty_body_is_reported_as_rate_limit() {
        let error = decode_json::<Value>(reqwest::StatusCode::OK, "/v5/stock/quote.json", " \n")
            .err()
            .unwrap_or_else(|| panic!("empty body should fail"));
        assert!(error.to_string().contains("限频"));
    }

    #[test]
    fn html_body_is_reported_as_waf_interception() {
        let error = decode_json::<Value>(
            reqwest::StatusCode::OK,
            "/v5/stock/quote.json",
            "<html>请求异常已被安全策略拦截</html>",
        )
        .err()
        .unwrap_or_else(|| panic!("HTML body should fail"));
        assert!(error.to_string().contains("WAF"));
    }

    #[test]
    fn http_error_includes_status_code() {
        let error = decode_json::<Value>(reqwest::StatusCode::FORBIDDEN, "/x", "{}")
            .err()
            .unwrap_or_else(|| panic!("non-2xx should fail"));
        assert!(error.to_string().contains("HTTP 403"));
    }

    #[test]
    fn business_error_code_is_not_treated_as_empty_data() {
        let body = json!({
            "error_code": 20210,
            "error_description": "动态不存在"
        })
        .to_string();
        let error = decode_json::<Status>(reqwest::StatusCode::OK, "/statuses/show.json", &body)
            .err()
            .unwrap_or_else(|| panic!("business error should fail"));
        assert!(error.to_string().contains("20210"));
        assert!(error.to_string().contains("动态不存在"));
    }

    /// 实测上游会返回字符串形式的 `error_code`。
    #[test]
    fn string_error_code_is_reported() {
        let body = json!({
            "error_description": "遇到错误，请刷新页面或者重新登录帐号后再试",
            "error_uri": "/v5/stock/quote.json",
            "error_data": null,
            "error_code": "400016"
        })
        .to_string();
        let error = decode_json::<Value>(
            reqwest::StatusCode::BAD_REQUEST,
            "/v5/stock/quote.json",
            &body,
        )
        .err()
        .unwrap_or_else(|| panic!("business error should fail"));
        assert!(error.to_string().contains("400016"));
        assert!(error.to_string().contains("重新登录"));
    }

    /// 蛋卷接口的失败标记在 `result_code` 中。
    #[test]
    fn fund_result_code_error_is_reported() {
        let body = json!({
            "result_code": 1,
            "message": "基金不存在"
        })
        .to_string();
        let error = decode_json::<FundInfo>(reqwest::StatusCode::OK, "/djapi/fund/000000", &body)
            .err()
            .unwrap_or_else(|| panic!("fund business error should fail"));
        assert!(error.to_string().contains("错误码 1"));
        assert!(error.to_string().contains("基金不存在"));
    }

    #[tokio::test]
    #[ignore = "integration test, requires network and XUEQIU_COOKIE"]
    async fn integration_quote() {
        let Ok(cookie) = std::env::var("XUEQIU_COOKIE") else {
            return;
        };
        let client = XueqiuClient::with_cookie(cookie);
        let quote = client
            .quote("SH600519")
            .await
            .unwrap_or_else(|error| panic!("xueqiu quote should succeed: {error}"));
        assert_eq!(
            quote
                .get("data")
                .and_then(|data| data.get("quote"))
                .and_then(|quote| quote.get("name"))
                .and_then(Value::as_str),
            Some("贵州茅台")
        );
    }

    /// 热帖接口会跨域重定向；确认重定向后仍能拿到数据。
    #[tokio::test]
    #[ignore = "integration test, requires network and XUEQIU_COOKIE"]
    async fn integration_hots_through_redirect() {
        let Ok(cookie) = std::env::var("XUEQIU_COOKIE") else {
            return;
        };
        let client = XueqiuClient::with_cookie(cookie);
        let hots = client
            .hots(10, 1)
            .await
            .unwrap_or_else(|error| panic!("xueqiu hots should succeed: {error}"));
        assert!(!hots.is_empty(), "hots should return items");
    }
}
