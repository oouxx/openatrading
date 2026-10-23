use anyhow::{Context, Result};
use chrono::{Datelike, FixedOffset};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::utils::to_yahoo_symbol;

/// UTC+8 Beijing time — all A-share market clock calculations should use this.
fn beijing_tz() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).unwrap_or_else(|| panic!("8*3600 is a valid offset"))
}

/// Convert f64 to i64, rounding to nearest integer.
/// Timestamps from the API are always whole numbers.
fn f64_to_i64(v: f64) -> i64 {
    v as i64
}

/// 板块事件取板块涨幅，个股事件取个股涨幅。
fn event_pcp(event: &PlateOrStockAbnormalEvent) -> Option<f64> {
    event
        .plate_abnormal_event_data
        .as_ref()
        .and_then(|data| data.pcp)
        .or_else(|| {
            event
                .stock_abnormal_event_data
                .as_ref()
                .and_then(|data| data.pcp)
        })
}

/// 按事件 id 去重，保留第一次出现的事件。
///
/// 上游在同一秒可能重复返回同一个事件，且两条记录之间夹着别的事件；
/// 这类重复不是相邻的，不能用 `Vec::dedup_by_key`。
fn dedup_events_by_id(events: &mut Vec<PlateOrStockAbnormalEvent>) {
    let mut seen = std::collections::HashSet::with_capacity(events.len());
    events.retain(|event| seen.insert(event.id));
}

const BASE_URL: &str = "https://flash-api.xuangubao.cn/api";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlateReason {
    #[serde(default)]
    pub plate_id: i64,
    pub plate_name: String,
    #[serde(default)]
    pub plate_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SurgeReason {
    pub symbol: String,
    pub stock_reason: String,
    pub related_plates: Vec<PlateReason>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimitTimelineItem {
    pub timestamp: i64,
    pub status: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimitTimeline {
    pub items: Vec<LimitTimelineItem>,
}

// ─── 板块异动数据 ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RelatedStock {
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub mtm: f64,
    #[serde(default)]
    pub pcp: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PlateAbnormalEventData {
    #[serde(default)]
    pub plate_id: Option<i64>,
    #[serde(default)]
    pub plate_name: Option<String>,
    #[serde(default)]
    pub pcp: Option<f64>,
    #[serde(default)]
    pub related_stocks: Vec<RelatedStock>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StockAbnormalEventData {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub pcp: Option<f64>,
    pub mtm: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateOrStockAbnormalEvent {
    pub id: i64,
    pub target: String,
    pub event_type: i32,
    pub event_timestamp: i64,
    #[serde(default)]
    pub stock_abnormal_event_data: Option<StockAbnormalEventData>,
    #[serde(default)]
    pub plate_abnormal_event_data: Option<PlateAbnormalEventData>,
    pub good_or_bad: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapanAbnormal {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Title")]
    pub title: String,
    #[serde(alias = "BkYiDongType")]
    pub bk_yi_dong_type: i32,
    #[serde(alias = "BkjInfo")]
    pub bkj_info: BkjInfo,
    #[serde(alias = "CreatedAt")]
    pub created_at: i64,
    #[serde(alias = "UpdatedAt")]
    pub updated_at: i64,
}
// 板块信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BkjInfo {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
}

/*
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "56_044_969",
        "Name": "数字人民币"
      },
      "CreatedAt": 1_775_797_961,
      "UpdatedAt": 1_775_798_032


*/
#[derive(Debug, Deserialize, Clone)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub symbol: String,
    pub stock_chi_name: String,
    pub price: f64,
    pub change_percent: f64,
    pub limit_up_days: u32,
    pub first_limit_up: i64,
    pub last_limit_up: i64,
    pub break_limit_up_times: u32,
    pub first_break_limit_up: i64,
    pub last_break_limit_up: i64,
    pub limit_down_days: u32,
    pub first_limit_down: i64,
    pub last_limit_down: i64,
    pub break_limit_down_times: u32,
    pub first_break_limit_down: i64,
    pub last_break_limit_down: i64,
    pub yesterday_first_limit_up: i64,
    pub yesterday_last_limit_up: i64,
    pub yesterday_break_limit_up_times: u32,
    pub yesterday_limit_up_days: u32,
    pub yesterday_limit_down_days: u32,
    pub non_restricted_capital: f64,
    pub total_capital: f64,
    pub turnover_ratio: f64,
    pub volume_bias_ratio: f64,
    pub buy_lock_volume_ratio: f64,
    pub sell_lock_volume_ratio: f64,
    pub stock_type: i32,
    pub is_new_stock: bool,
    pub issue_price: f64,
    pub listed_date: i64,
    pub surge_reason: Option<SurgeReason>,
    #[serde(default)]
    pub limit_timeline: Option<LimitTimeline>,
    pub mtm: f64,
    pub m_days_n_boards_boards: u32,
    pub m_days_n_boards_days: u32,
    pub nearly_new_acc_pcp: f64,
    pub nearly_new_break_days: u32,
    pub new_stock_acc_pcp: f64,
    pub new_stock_break_limit_up: u32,
    pub new_stock_limit_up_days: u32,
    pub new_stock_limit_up_price_before_broken: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub rise_count: u32,
    pub fall_count: u32,
    pub bomb_rate: f64,
    pub market_temperature: f64,
    pub yesterday_limit_up_avg_pcp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverviewDay {
    pub date: String,
    pub rise_count: u32,
    pub fall_count: u32,
    pub limit_up_count: u32,
    pub limit_down_count: u32,
    pub bomb_rate: f64,
    pub market_temperature: f64,
}
/// 选股宝 API `fields` 请求参数中的单个字段。
///
/// 通过 `ApiField::join(&[variant, ...])` 生成逗号分隔字符串，
/// 每个 endpoint 在其方法中显式声明所需的字段列表。
#[derive(Debug, Clone, Copy)]
pub enum ApiField {
    /// 上涨家数
    RiseCount,
    /// 下跌家数
    FallCount,
    /// 涨停家数
    LimitUpCount,
    /// 跌停家数
    LimitDownCount,
    /// 炸板率
    LimitUpBrokenRatio,
    /// 市场情绪温度
    MarketTemperature,
    /// 昨日涨停股今日平均涨幅
    YesterdayLimitUpAvgPcp,
    /// 时间戳（分时/K线）
    TickAt,
    /// 最新价（分时/K线）
    ClosePx,
    /// 开盘价
    OpenPx,
    /// 最高价
    HighPx,
    /// 最低价
    LowPx,
    /// 成交量
    TurnoverVolume,
    /// 涨跌幅
    PxChangeRate,
    /// 产品名称
    ProdName,
    /// 最新价（实时）
    LastPx,
    /// 涨跌额
    PxChange,
    /// 股票代码
    Symbol,
    /// 交易状态
    TradeStatus,
    /// 更新时间
    UpdateTime,
    /// 昨收价
    PreclosePx,
    /// 振幅
    Amplitude,
    /// 换手率
    TurnoverRatio,
    /// 市盈率
    PeRate,
    /// 动态市盈率
    DynPe,
    /// 市净率
    DynPbRate,
    /// 总市值
    MarketValue,
    /// 流通市值
    CirculationValue,
    /// 成交额
    TurnoverValue,
    /// 量比
    VolumeRatio,
    /// 流通股本
    CirculationShares,
    /// 总股本
    TotalShares,
    /// 板块核心涨幅
    CoreAvgPcp,
    /// 板块名称
    PlateName,
}

impl ApiField {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RiseCount => "rise_count",
            Self::FallCount => "fall_count",
            Self::LimitUpCount => "limit_up_count",
            Self::LimitDownCount => "limit_down_count",
            Self::LimitUpBrokenRatio => "limit_up_broken_ratio",
            Self::MarketTemperature => "market_temperature",
            Self::YesterdayLimitUpAvgPcp => "yesterday_limit_up_avg_pcp",
            Self::TickAt => "tick_at",
            Self::ClosePx => "close_px",
            Self::OpenPx => "open_px",
            Self::HighPx => "high_px",
            Self::LowPx => "low_px",
            Self::TurnoverVolume => "turnover_volume",
            Self::PxChangeRate => "px_change_rate",
            Self::ProdName => "prod_name",
            Self::LastPx => "last_px",
            Self::PxChange => "px_change",
            Self::Symbol => "symbol",
            Self::TradeStatus => "trade_status",
            Self::UpdateTime => "update_time",
            Self::PreclosePx => "preclose_px",
            Self::Amplitude => "amplitude",
            Self::TurnoverRatio => "turnover_ratio",
            Self::PeRate => "pe_rate",
            Self::DynPe => "dyn_pe",
            Self::DynPbRate => "dyn_pb_rate",
            Self::MarketValue => "market_value",
            Self::CirculationValue => "circulation_value",
            Self::TurnoverValue => "turnover_value",
            Self::VolumeRatio => "volume_ratio",
            Self::CirculationShares => "circulation_shares",
            Self::TotalShares => "total_shares",
            Self::CoreAvgPcp => "core_avg_pcp",
            Self::PlateName => "plate_name",
        }
    }

    /// 将多个字段拼接为逗号分隔的字符串（即 `fields=...` 参数值）。
    pub fn join(fields: &[Self]) -> String {
        fields
            .iter()
            .map(Self::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTrend {
    pub symbol: String,
    pub candles: Vec<KlineCandle>,
}

/// Parse the trend endpoint response using the field order declared by the API.
///
/// The trend endpoint returns `data.fields` plus parallel `data.candle.<symbol>.lines`
/// arrays. Parsing by field name avoids depending on a fixed column order.
fn parse_market_trend(symbol: &str, resp: &serde_json::Value) -> Result<MarketTrend> {
    let candle_data = resp["data"]["candle"]
        .get(symbol)
        .with_context(|| format!("missing trend data for {symbol}"))?;
    let lines = candle_data["lines"]
        .as_array()
        .with_context(|| format!("missing trend lines for {symbol}"))?;
    let fields: Vec<String> = resp["data"]["fields"]
        .as_array()
        .context("missing trend fields array")?
        .iter()
        .filter_map(|value| value.as_str().map(String::from))
        .collect();

    let candles: Vec<KlineCandle> = lines
        .iter()
        .filter_map(|line| {
            let arr = line.as_array()?;
            let get = |name: &str| -> Option<f64> {
                let idx = fields.iter().position(|field| field == name)?;
                arr.get(idx)?.as_f64()
            };
            Some(KlineCandle {
                time: get("tick_at").map(f64_to_i64)?,
                open: get("open_px"),
                close: get("close_px")?,
                high: get("high_px"),
                low: get("low_px"),
                volume: get("turnover_volume"),
                change_rate: get("px_change_rate"),
            })
        })
        .collect();

    Ok(MarketTrend {
        symbol: symbol.to_string(),
        candles,
    })
}

#[derive(Clone)]
pub struct XuanguBaoClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for XuanguBaoClient {
    fn default() -> Self {
        Self::new()
    }
}

impl XuanguBaoClient {
    pub fn new() -> Self {
        Self::with_base_url(BASE_URL.to_string())
    }

    pub fn with_base_url(base_url: String) -> Self {
        let client = Self::build_client();
        Self { client, base_url }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build_client() -> reqwest::Client {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
    }

    #[cfg(target_arch = "wasm32")]
    fn build_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    async fn fetch_pool(&self, pool_name: &str) -> Result<Vec<Stock>> {
        let url = format!("{}/pool/detail?pool_name={pool_name}", self.base_url);

        let resp: ApiResponse<Vec<Stock>> = self.client.get(&url).send().await?.json().await?;

        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);
        Ok(resp.data)
    }

    /// Fetch limit-up stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_up").await
    }

    /// Fetch limit-down stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_limit_down(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_down").await
    }

    /// Fetch yesterday's limit-up stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_yesterday_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("yesterday_limit_up").await
    }
    /// Fetch super stocks.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_super_stock(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("super_stock").await
    }

    /// Fetch market overview data.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_overview(&self) -> Result<MarketOverview> {
        let fields = ApiField::join(&[
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
            ApiField::YesterdayLimitUpAvgPcp,
        ]);
        let url = format!("{}/market_indicator/line?fields={fields}", self.base_url);

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let latest = &resp["data"]
            .as_array()
            .and_then(|a| a.last())
            .cloned()
            .unwrap_or_default();

        Ok(MarketOverview {
            rise_count: u32::try_from(latest["rise_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            fall_count: u32::try_from(latest["fall_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            limit_up_count: u32::try_from(latest["limit_up_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            limit_down_count: u32::try_from(latest["limit_down_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            bomb_rate: latest["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: latest["market_temperature"].as_f64().unwrap_or(50.0) as f64,
            yesterday_limit_up_avg_pcp: latest["yesterday_limit_up_avg_pcp"].as_f64().unwrap_or(0.0)
                as f64,
        })
    }

    /// Fetch intraday market overview for a specific date, returns the last (most recent) snapshot.
    /// Generate the last `n` trading day dates (YYYY-MM-DD), skipping weekends.
    /// Generate the last `n` trading day dates (YYYY-MM-DD), skipping weekends.
    ///
    /// # Panics
    /// Panics if date underflow occurs (should never happen with current dates).
    pub fn last_n_trading_days(n: usize) -> Vec<String> {
        let mut dates = Vec::with_capacity(n);
        let mut current = chrono::Utc::now().with_timezone(&beijing_tz()).date_naive();
        // If today is a weekend, start from Friday
        while dates.len() < n {
            match current.weekday() {
                chrono::Weekday::Sat | chrono::Weekday::Sun => {
                    current = current.pred_opt().unwrap_or_else(|| {
                        panic!("date underflow - should not happen with current dates")
                    });
                }
                _ => {
                    dates.push(current.format("%Y-%m-%d").to_string());
                    current = current.pred_opt().unwrap_or_else(|| {
                        panic!("date underflow - should not happen with current dates")
                    });
                }
            }
        }
        dates
    }

    /// Fetch market overview for a specific date.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_overview_for_date(
        &self,
        date: &str,
    ) -> Result<Option<MarketOverviewDay>> {
        let fields = ApiField::join(&[
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
        ]);
        let url = format!(
            "{}/market_indicator/line?fields={fields}&date={date}",
            self.base_url,
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let arr = resp["data"].as_array().cloned().unwrap_or_default();

        // Take the last data point (most recent timestamp) from the intraday array
        let last = match arr.last() {
            Some(v) => v.clone(),
            None => return Ok(None),
        };

        Ok(Some(MarketOverviewDay {
            date: date.to_string(),
            rise_count: u32::try_from(last["rise_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            fall_count: u32::try_from(last["fall_count"].as_u64().unwrap_or(0)).unwrap_or(0),
            limit_up_count: u32::try_from(last["limit_up_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            limit_down_count: u32::try_from(last["limit_down_count"].as_u64().unwrap_or(0))
                .unwrap_or(0),
            bomb_rate: last["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: last["market_temperature"].as_f64().unwrap_or(50.0),
        }))
    }
    /// Fetch market trend data for a symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_trend(&self, symbol: &str) -> Result<MarketTrend> {
        // 与实时行情/K 线保持一致：允许 600519、sh600519、600519.SH 等输入，
        // 统一成接口实际使用的 600519.SS 形式。
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "{}/market/trend?fields={}&prod_code={symbol}",
            "https://api-ddc-wscn.xuangubao.com.cn",
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::ClosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Trend API error: {}",
            resp["message"]
        );
        parse_market_trend(&symbol, &resp)
    }
    /// Fetch real-time market data for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_real(&self, symbols: &[&str]) -> Result<Vec<IndexQuote>> {
        let joined = symbols.join(",");
        let fields = ApiField::join(&[
            ApiField::ProdName,
            ApiField::LastPx,
            ApiField::PxChange,
            ApiField::PxChangeRate,
            ApiField::Symbol,
            ApiField::TradeStatus,
        ]);
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/real?fields={fields}&prod_code={joined}",
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Market Real API error: {}",
            resp["message"]
        );

        let field_names: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let snapshot = resp["data"]["snapshot"]
            .as_object()
            .context("missing snapshot")?;

        let mut results = Vec::new();
        for (symbol, values) in snapshot {
            let arr = values.as_array().context("snapshot value not array")?;
            let get_f64 = |name: &str| -> f64 {
                field_names
                    .iter()
                    .position(|f| f == name)
                    .and_then(|i| arr.get(i))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0)
            };
            let get_str = |name: &str| -> String {
                field_names
                    .iter()
                    .position(|f| f == name)
                    .and_then(|i| arr.get(i))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            results.push(IndexQuote {
                symbol: symbol.clone(),
                name: get_str("prod_name"),
                last_px: get_f64("last_px"),
                px_change: get_f64("px_change"),
                px_change_rate: get_f64("px_change_rate"),
                trade_status: get_str("trade_status"),
            });
        }

        Ok(results)
    }
    /// Fetch plate abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_plate_abnormal(&self, count: i32) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let types =
            "10001,10005,10003,10007,10002,10006,10004,10008,10012,10014,10009,10010,11000,11001";
        let url = format!(
            "{}/event/history?count={}&types={}",
            self.base_url, count, types
        );

        let resp: ApiResponse<Vec<PlateOrStockAbnormalEvent>> =
            self.client.get(&url).send().await?.json().await?;

        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);
        Ok(resp.data)
    }

    /// `fetch_plate_abnormal` with String count — for LLM tool use
    /// Fetch plate abnormal events as raw JSON.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_plate_abnormal_str(&self, count: &str) -> Result<serde_json::Value> {
        let n: i32 = count.parse().unwrap_or(30);
        let data = self.fetch_plate_abnormal(n).await?;
        Ok(serde_json::to_value(&data).unwrap_or_default())
    }

    /// Cursor-paginate to fetch ALL events for the latest trading day (09:30–15:00).
    /// Returns events sorted by `event_timestamp` ascending.
    /// Fetch a page of abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_events_page(
        &self,
        timestamp: i64,
        count: u32,
        types: &str,
    ) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let url = format!(
            "{}/event/history?timestamp={timestamp}&count={count}&types={types}",
            self.base_url
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let code = resp["code"].as_i64().unwrap_or(0);
        anyhow::ensure!(code == 20000, "API 返回错误码: {code}");
        let data = resp["data"].as_array().cloned().unwrap_or_default();
        let events: Vec<PlateOrStockAbnormalEvent> = data
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        Ok(events)
    }

    /// 拉取指定交易日 09:30–15:00 的异动事件。
    ///
    /// 盘中调用时结束时间取当前时间，收盘后取 15:00。`min_pcp` 只做粗过滤，
    /// 建议传一个较低阈值（例如 0.005），把细粒度数据留给回放页面做二次筛选。
    ///
    /// # Errors
    /// 日期非法、HTTP 请求失败或响应无法解析时返回错误。
    pub async fn fetch_events_for_date(&self, date: &str, min_pcp: f64) -> Result<DayEventBatch> {
        let close_ts = Self::trade_date_close_ts(date)?;
        let open_ts = close_ts - 5 * 3600 - 30 * 60;
        let end_ts = Self::effective_event_end_ts(date, close_ts);
        if end_ts <= open_ts {
            return Ok(DayEventBatch {
                events: Vec::new(),
                window_start: open_ts,
                window_end: end_ts,
                reached_open: true,
            });
        }

        let mut all_events: Vec<PlateOrStockAbnormalEvent> = Vec::new();
        let page_size: u32 = 1000;
        let mut cursor = end_ts + 1;
        let mut reached_open = false;

        loop {
            let page = self
                .fetch_events_page(cursor, page_size, "11001,11000")
                .await?;
            if page.is_empty() {
                // 没有更早的事件 —— 分页已经走到当天起点。
                reached_open = true;
                break;
            }
            // API 返回 cursor 之前的事件（新→旧）；用最小时间戳作为下一页游标。
            let earliest = page
                .iter()
                .map(|e| e.event_timestamp)
                .min()
                .unwrap_or(cursor);
            all_events.extend(
                page.into_iter()
                    .filter(|e| e.event_timestamp >= open_ts && e.event_timestamp <= end_ts),
            );
            if earliest <= open_ts {
                reached_open = true;
                break;
            }
            if earliest >= cursor {
                // 上游返回了不在游标之前的数据，避免死循环。
                break;
            }
            cursor = earliest;
        }

        all_events.sort_by_key(|e| e.event_timestamp);
        dedup_events_by_id(&mut all_events);
        all_events.retain(|e| event_pcp(e).is_some_and(|pcp| pcp.abs() > min_pcp));

        tracing::debug!(
            date,
            count = all_events.len(),
            reached_open,
            "xuangubao::fetch_events_for_date done"
        );
        Ok(DayEventBatch {
            events: all_events,
            window_start: open_ts,
            window_end: end_ts,
            reached_open,
        })
    }

    /// Fetch all abnormal events for the latest completed trading day.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_all_day_events(
        &self,
        min_pcp: f64,
    ) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let date = Self::latest_completed_trade_date();
        Ok(self.fetch_events_for_date(&date, min_pcp).await?.events)
    }

    /// 最近一个已收盘的交易日（YYYY-MM-DD）。
    ///
    /// 只跳过周末，不含节假日日历；节假日会取到空数据，调用方需自行回退。
    pub fn latest_completed_trade_date() -> String {
        let now = chrono::Utc::now().with_timezone(&beijing_tz());
        let close = chrono::NaiveTime::from_hms_opt(15, 0, 0)
            .unwrap_or_else(|| panic!("15:00:00 is a valid time"));
        let mut date = now.date_naive();
        if now.time() < close {
            date = date.pred_opt().unwrap_or(date);
        }
        while matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
            date = date.pred_opt().unwrap_or(date);
        }
        date.format("%Y-%m-%d").to_string()
    }

    /// 指定交易日 15:00（北京时间）的 Unix 时间戳。
    ///
    /// # Errors
    /// 日期不是 `YYYY-MM-DD` 时返回错误。
    pub fn trade_date_close_ts(date: &str) -> Result<i64> {
        let naive = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .with_context(|| format!("非法交易日: {date}"))?
            .and_hms_opt(15, 0, 0)
            .with_context(|| format!("非法收盘时间: {date}"))?;
        Ok(naive.and_local_timezone(beijing_tz()).single().map_or_else(
            || panic!("fixed offset always resolves to a single local time"),
            |dt| dt.timestamp(),
        ))
    }

    /// 当天盘中只回放到当前时刻，收盘后回放到 15:00。
    fn effective_event_end_ts(date: &str, close_ts: i64) -> i64 {
        let now = chrono::Utc::now().with_timezone(&beijing_tz());
        if date == now.format("%Y-%m-%d").to_string() {
            now.timestamp().min(close_ts)
        } else {
            close_ts
        }
    }

    /// Unix timestamp for 15:00 on the latest completed trading day (Beijing time).
    /// If today's market hasn't closed yet, returns the previous trading day's close.
    ///
    /// # Panics
    /// Panics if the computed date is invalid (should never happen).
    pub fn latest_trading_day_close_ts() -> i64 {
        let date = Self::latest_completed_trade_date();
        Self::trade_date_close_ts(&date)
            .unwrap_or_else(|e| panic!("latest trading day close timestamp: {e}"))
    }

    // ---大盘异动---------------------------------------
    /// Fetch dapan (大盘) abnormal events.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_dapan_abnormal(&self) -> Result<Vec<DapanAbnormal>> {
        let url =
            "https://api.xuangubao.com.cn/api/messages/todayDaPanYiDong?headmark=0".to_string();

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        // API returns { Messages: [...], NextHeadMark: "..." } or { messages: [...] }
        let messages = resp["Messages"]
            .as_array()
            .or_else(|| resp["messages"].as_array())
            .cloned()
            .unwrap_or_default();

        let result: Vec<DapanAbnormal> = messages
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        Ok(result)
    }

    // ─── K线数据 ───────────────────────────────────────

    /// Fetch K-line data for a symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_kline(
        &self,
        symbol: &str,
        period: KlinePeriod,
        count: u32,
    ) -> Result<KlineData> {
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/kline?fields={}&prod_code={symbol}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward",
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::OpenPx,
                ApiField::ClosePx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Kline API error: {}",
            resp["message"]
        );

        let candle_data = &resp["data"]["candle"][&symbol];
        let lines = candle_data["lines"]
            .as_array()
            .context("missing kline lines array")?;

        // API 返回的字段顺序以 data.fields 为准，需要动态映射
        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let candles: Vec<KlineCandle> = lines
            .iter()
            .filter_map(|line| {
                let arr = line.as_array()?;
                let get = |name: &str| -> Option<f64> {
                    let idx = fields.iter().position(|f| f == name)?;
                    arr.get(idx)?.as_f64()
                };
                Some(KlineCandle {
                    time: get("tick_at").map(f64_to_i64)?,
                    open: get("open_px"),
                    close: get("close_px")?,
                    high: get("high_px"),
                    low: get("low_px"),
                    volume: get("turnover_volume"),
                    change_rate: get("px_change_rate"),
                })
            })
            .collect();

        Ok(KlineData {
            symbol: symbol.clone(),
            period: period.as_str().to_string(),
            candles,
        })
    }

    /// Fetch kline for multiple symbols in a single API call.
    /// Returns a map keyed by normalized (yahoo) symbol.
    /// Fetch K-line data for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, any symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_kline_batch(
        &self,
        symbols: &[String],
        period: KlinePeriod,
        count: u32,
    ) -> Result<ahash::AHashMap<String, KlineData>> {
        let yahoo_symbols: Vec<String> = symbols
            .iter()
            .map(|s| to_yahoo_symbol(s))
            .collect::<Result<Vec<_>>>()?;

        let prod_codes: Vec<&str> = yahoo_symbols
            .iter()
            .map(std::string::String::as_str)
            .collect();
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/kline?fields={}&prod_code={}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward",
            ApiField::join(&[
                ApiField::TickAt,
                ApiField::OpenPx,
                ApiField::ClosePx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::TurnoverVolume,
                ApiField::PxChangeRate,
            ]),
            prod_codes.join(","),
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Kline API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let candle_data = &resp["data"]["candle"];
        let mut result = ahash::AHashMap::new();

        for yahoo_sym in &yahoo_symbols {
            let lines = candle_data[yahoo_sym]["lines"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            let candles: Vec<KlineCandle> = lines
                .iter()
                .filter_map(|line| {
                    let arr = line.as_array()?;
                    let get = |name: &str| -> Option<f64> {
                        let idx = fields.iter().position(|f| f == name)?;
                        arr.get(idx)?.as_f64()
                    };
                    Some(KlineCandle {
                        time: get("tick_at").map(f64_to_i64)?,
                        open: get("open_px"),
                        close: get("close_px")?,
                        high: get("high_px"),
                        low: get("low_px"),
                        volume: get("turnover_volume"),
                        change_rate: get("px_change_rate"),
                    })
                })
                .collect();

            result.insert(
                yahoo_sym.clone(),
                KlineData {
                    symbol: yahoo_sym.clone(),
                    period: period.as_str().to_string(),
                    candles,
                },
            );
        }

        Ok(result)
    }

    /// Fetch real-time quote for a single symbol.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, the symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_realtime(&self, symbol: &str) -> Result<RealtimeQuote> {
        let symbol = to_yahoo_symbol(symbol)?;
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/real?fields={}&prod_code={}",
            ApiField::join(&[
                ApiField::ProdName,
                ApiField::TradeStatus,
                ApiField::UpdateTime,
                ApiField::LastPx,
                ApiField::PxChange,
                ApiField::PxChangeRate,
                ApiField::PreclosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::Amplitude,
                ApiField::TurnoverRatio,
                ApiField::PeRate,
                ApiField::DynPe,
                ApiField::DynPbRate,
                ApiField::MarketValue,
                ApiField::CirculationValue,
                ApiField::TurnoverVolume,
                ApiField::TurnoverValue,
                ApiField::VolumeRatio,
                ApiField::CirculationShares,
                ApiField::TotalShares,
            ]),
            symbol
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Realtime API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let snap = &resp["data"]["snapshot"][&symbol];
        if snap.is_null() {
            anyhow::bail!("symbol '{symbol}' not found in realtime snapshot");
        }
        let arr = snap.as_array().context("missing snapshot array")?;

        let field_pos = |name: &str| -> Option<usize> { fields.iter().position(|f| f == name) };

        let get_f64 = |name: &str| -> Option<f64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_f64)
        };
        let get_str = |name: &str| -> Option<&str> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(|v| v.as_str())
        };
        let get_i64 = |name: &str| -> Option<i64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_i64)
        };

        Ok(RealtimeQuote {
            symbol: symbol.clone(),
            name: get_str("prod_name").unwrap_or("").to_string(),
            trade_status: get_str("trade_status")
                .context("missing trade_status in realtime response")?
                .to_string(),
            update_time: get_i64("update_time")
                .context("missing update_time in realtime response")?,
            last_px: get_f64("last_px").context("missing last_px in realtime response")?,
            px_change: get_f64("px_change").unwrap_or(0.0),
            px_change_rate: get_f64("px_change_rate").unwrap_or(0.0),
            preclose_px: get_f64("preclose_px")
                .context("missing preclose_px in realtime response")?,
            open_px: get_f64("open_px").unwrap_or(0.0),
            high_px: get_f64("high_px").unwrap_or(0.0),
            low_px: get_f64("low_px").unwrap_or(0.0),
            amplitude: get_f64("amplitude").unwrap_or(0.0),
            turnover_ratio: get_f64("turnover_ratio").unwrap_or(0.0),
            pe_rate: get_f64("pe_rate").unwrap_or(0.0),
            dyn_pe: get_f64("dyn_pe").unwrap_or(0.0),
            dyn_pb_rate: get_f64("dyn_pb_rate").unwrap_or(0.0),
            market_value: get_f64("market_value").unwrap_or(0.0),
            circulation_value: get_f64("circulation_value").unwrap_or(0.0),
            turnover_volume: get_f64("turnover_volume").unwrap_or(0.0),
            turnover_value: get_f64("turnover_value").unwrap_or(0.0),
            volume_ratio: get_f64("volume_ratio").unwrap_or(0.0),
            circulation_shares: get_f64("circulation_shares").unwrap_or(0.0),
            total_shares: get_f64("total_shares").unwrap_or(0.0),
        })
    }

    /// Fetch realtime quotes for multiple symbols in a single API call.
    /// Returns a map keyed by normalized (yahoo) symbol.
    /// Fetch real-time quotes for multiple symbols.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, any symbol is invalid, or the response cannot be parsed.
    pub async fn fetch_realtime_batch(
        &self,
        symbols: &[String],
    ) -> Result<ahash::AHashMap<String, RealtimeQuote>> {
        let yahoo_symbols: Vec<String> = symbols
            .iter()
            .map(|s| to_yahoo_symbol(s))
            .collect::<Result<Vec<_>>>()?;

        let prod_codes: Vec<&str> = yahoo_symbols
            .iter()
            .map(std::string::String::as_str)
            .collect();
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/real?fields={}&prod_code={}",
            ApiField::join(&[
                ApiField::ProdName,
                ApiField::TradeStatus,
                ApiField::UpdateTime,
                ApiField::LastPx,
                ApiField::PxChange,
                ApiField::PxChangeRate,
                ApiField::PreclosePx,
                ApiField::OpenPx,
                ApiField::HighPx,
                ApiField::LowPx,
                ApiField::Amplitude,
                ApiField::TurnoverRatio,
                ApiField::PeRate,
                ApiField::DynPe,
                ApiField::DynPbRate,
                ApiField::MarketValue,
                ApiField::CirculationValue,
                ApiField::TurnoverVolume,
                ApiField::TurnoverValue,
                ApiField::VolumeRatio,
                ApiField::CirculationShares,
                ApiField::TotalShares,
            ]),
            prod_codes.join(","),
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Realtime API error: {}",
            resp["message"]
        );

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let field_pos = |name: &str| -> Option<usize> { fields.iter().position(|f| f == name) };
        let get_f64 = |arr: &[serde_json::Value], name: &str| -> Option<f64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_f64)
        };
        let get_str_for_arr = |arr: &[serde_json::Value], name: &str| -> Option<String> {
            let idx = fields.iter().position(|f| f == name)?;
            arr.get(idx).and_then(|v| v.as_str()).map(String::from)
        };
        let get_i64 = |arr: &[serde_json::Value], name: &str| -> Option<i64> {
            field_pos(name)
                .and_then(|i| arr.get(i))
                .and_then(serde_json::Value::as_i64)
        };

        let snapshots = &resp["data"]["snapshot"];
        let mut result = ahash::AHashMap::new();
        for yahoo_sym in &yahoo_symbols {
            let snap = &snapshots[yahoo_sym];
            if snap.is_null() {
                continue;
            }
            if let Some(arr) = snap.as_array() {
                result.insert(
                    yahoo_sym.clone(),
                    RealtimeQuote {
                        symbol: yahoo_sym.clone(),
                        name: get_str_for_arr(arr, "prod_name").unwrap_or_default(),
                        trade_status: get_str_for_arr(arr, "trade_status").unwrap_or_default(),
                        update_time: get_i64(arr, "update_time").unwrap_or(0),
                        last_px: get_f64(arr, "last_px").unwrap_or(0.0),
                        px_change: get_f64(arr, "px_change").unwrap_or(0.0),
                        px_change_rate: get_f64(arr, "px_change_rate").unwrap_or(0.0),
                        preclose_px: get_f64(arr, "preclose_px").unwrap_or(0.0),
                        open_px: get_f64(arr, "open_px").unwrap_or(0.0),
                        high_px: get_f64(arr, "high_px").unwrap_or(0.0),
                        low_px: get_f64(arr, "low_px").unwrap_or(0.0),
                        amplitude: get_f64(arr, "amplitude").unwrap_or(0.0),
                        turnover_ratio: get_f64(arr, "turnover_ratio").unwrap_or(0.0),
                        pe_rate: get_f64(arr, "pe_rate").unwrap_or(0.0),
                        dyn_pe: get_f64(arr, "dyn_pe").unwrap_or(0.0),
                        dyn_pb_rate: get_f64(arr, "dyn_pb_rate").unwrap_or(0.0),
                        market_value: get_f64(arr, "market_value").unwrap_or(0.0),
                        circulation_value: get_f64(arr, "circulation_value").unwrap_or(0.0),
                        turnover_volume: get_f64(arr, "turnover_volume").unwrap_or(0.0),
                        turnover_value: get_f64(arr, "turnover_value").unwrap_or(0.0),
                        volume_ratio: get_f64(arr, "volume_ratio").unwrap_or(0.0),
                        circulation_shares: get_f64(arr, "circulation_shares").unwrap_or(0.0),
                        total_shares: get_f64(arr, "total_shares").unwrap_or(0.0),
                    },
                );
            }
        }

        Ok(result)
    }
    /// Fetch plates that contain the given stock.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    /// Fetch plates that contain the given stock.
    ///
    /// # Panics
    /// Panics if the `plate_id` value does not fit in `i32`.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn plates_by_any_stock(&self, symbol: &str) -> Result<Vec<PlateAndReason>> {
        let url = format!(
            "{}/stage2/plates_by_any_stock?symbol={symbol}&fields={}",
            self.base_url,
            ApiField::join(&[ApiField::CoreAvgPcp, ApiField::PlateName]),
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Plates API error: {}",
            resp["message"]
        );
        let plates = resp["data"]
            .as_object()
            .context("missing plates data")?
            .values()
            .filter_map(|v| {
                let obj = v.as_object()?;
                Some(PlateAndReason {
                    core_avg_pcp: obj.get("core_avg_pcp")?.as_f64()?,
                    plate_id: i32::try_from(obj.get("plate_id")?.as_i64()?)
                        .unwrap_or_else(|e| panic!("plate_id should fit in i32: {e}")),
                    plate_name: obj.get("plate_name")?.as_str()?.to_string(),
                    reason: obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                })
            })
            .collect();

        Ok(plates)
    }
}

// ─── K线数据结构 ──────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum KlinePeriod {
    Minute,
    Day,
    Week,
    Month,
}

impl KlinePeriod {
    pub const fn as_seconds(&self) -> u32 {
        match self {
            Self::Minute => 60,
            Self::Day => 86400,
            Self::Week => 604_800,
            Self::Month => 2_592_000,
        }
    }

    pub const fn as_str(&self) -> &str {
        match self {
            Self::Minute => "minute",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineCandle {
    pub time: i64,
    #[serde(default)]
    pub open: Option<f64>,
    pub close: f64,
    #[serde(default)]
    pub high: Option<f64>,
    #[serde(default)]
    pub low: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub change_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub symbol: String,
    pub period: String,
    pub candles: Vec<KlineCandle>,
}

// fields=prod_name(0),trade_status(1),update_time(2),last_px(3),px_change(4),
//   px_change_rate(5),preclose_px(6),open_px(7),high_px(8),low_px(9),
//   amplitude(10),turnover_ratio(11),pe_rate(12),dyn_pe(13),dyn_pb_rate(14),
//   market_value(15),circulation_value(16),turnover_volume(17),turnover_value(18),
//   volume_ratio(19),circulation_shares(20),total_shares(21)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    /// 0: `prod_name`
    pub name: String,
    /// 3: `last_px`
    pub last_px: f64,
    /// 4: `px_change`
    pub px_change: f64,
    /// 5: `px_change_rate`
    pub px_change_rate: f64,
    /// 7: `open_px`
    pub open_px: f64,
    /// 8: `high_px`
    pub high_px: f64,
    /// 9: `low_px`
    pub low_px: f64,
    /// 6: `preclose_px`
    pub preclose_px: f64,
    /// 10: amplitude
    pub amplitude: f64,
    /// 11: `turnover_ratio`
    pub turnover_ratio: f64,
    /// 17: `turnover_volume`
    pub turnover_volume: f64,
    /// 18: `turnover_value`
    pub turnover_value: f64,
    /// 15: `market_value`
    pub market_value: f64,
    /// 16: `circulation_value`
    pub circulation_value: f64,
    /// 12: `pe_rate`
    pub pe_rate: f64,
    /// 13: `dyn_pe`
    pub dyn_pe: f64,
    /// 14: `dyn_pb_rate`
    pub dyn_pb_rate: f64,
    /// 19: `volume_ratio`
    pub volume_ratio: f64,
    /// 20: `circulation_shares`
    pub circulation_shares: f64,
    /// 21: `total_shares`
    pub total_shares: f64,
    /// 1: `trade_status`
    pub trade_status: String,
    /// 2: `update_time`
    pub update_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexQuote {
    pub symbol: String,
    pub name: String,
    pub last_px: f64,
    pub px_change: f64,
    pub px_change_rate: f64,
    pub trade_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateAndReason {
    pub core_avg_pcp: f64,
    pub plate_id: i32,
    pub plate_name: String,
    #[serde(default)]
    pub reason: String,
}

// ─── 行情回放 ────────────────────────────────────────────

/// 某一天的完整事件批次。`reached_open` 表示分页已经覆盖到 09:30 或
/// 确认没有更早的事件，收盘对账时用它判断数据是否完整。
#[derive(Debug, Clone)]
pub struct DayEventBatch {
    pub events: Vec<PlateOrStockAbnormalEvent>,
    pub window_start: i64,
    pub window_end: i64,
    pub reached_open: bool,
}

/// Lightweight event for the replay frontend — strips unused `PlateAbnormalEvent` fields.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplayEvent {
    pub id: i64,
    pub event_type: i32,
    pub event_timestamp: i64,
    pub good_or_bad: i32,
    /// 事件原始目标（板块名或股票代码），作为展示名的兜底。
    #[serde(default)]
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcp: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_stocks: Vec<RelatedStock>,
}

impl From<&PlateOrStockAbnormalEvent> for ReplayEvent {
    fn from(ev: &PlateOrStockAbnormalEvent) -> Self {
        // 上游对不适用的字段会返回 `{}`，反序列化后是 `Some(默认值)`。
        // 不能只看 `is_some()`，否则空的板块载荷会吞掉真正的个股载荷。
        let plate = ev.plate_abnormal_event_data.as_ref().filter(|plate| {
            plate.plate_name.is_some() || plate.pcp.is_some() || !plate.related_stocks.is_empty()
        });
        let stock = ev
            .stock_abnormal_event_data
            .as_ref()
            .filter(|stock| stock.name.is_some() || stock.symbol.is_some() || stock.pcp.is_some());
        let plate_name = plate.and_then(|plate| plate.plate_name.clone());
        let stock_name = stock.and_then(|stock| stock.name.clone());
        let symbol = stock.and_then(|stock| stock.symbol.clone());
        // 少数板块事件只带关联股票、不带自身 pcp，这时用个股 pcp 兜底。
        let pcp = plate
            .and_then(|plate| plate.pcp)
            .or_else(|| stock.and_then(|stock| stock.pcp));
        let related_stocks = plate
            .map(|plate| plate.related_stocks.clone())
            .unwrap_or_default();
        Self {
            id: ev.id,
            event_type: ev.event_type,
            event_timestamp: ev.event_timestamp,
            good_or_bad: ev.good_or_bad,
            target: ev.target.clone(),
            plate_name,
            stock_name,
            symbol,
            pcp,
            related_stocks,
        }
    }
}

/// Paginated replay response.
#[derive(Debug, Serialize)]
pub struct ReplayPage {
    pub date: String,
    pub market_phase: String,
    pub events: Vec<ReplayEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<i64>,
}

/// Aggregated view of a 5‑minute window.
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSnapshot {
    pub timestamp: i64,
    pub time_label: String,
    pub emotion_score: f64,
    pub bull_count: u32,
    pub bear_count: u32,
    pub top_plates: Vec<TopPlate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopPlate {
    pub plate_name: String,
    pub pcp: f64,
    pub good_or_bad: i32,
    pub related_stocks: Vec<RelatedStock>,
}

struct PlateAccumulator {
    pcp: f64,
    good_or_bad: i32,
    related_stocks: Vec<RelatedStock>,
}

/// Slice events into 5‑minute buckets across 09:30–15:00, computing
/// cumulative emotion scores and top plates per bucket.
/// Build replay snapshots from abnormal events, grouped into 5-minute buckets.
///
/// # Panics
/// Panics if `events` is empty (caller should check before calling).
pub fn build_replay_snapshots(events: &[PlateOrStockAbnormalEvent]) -> Vec<TimeSnapshot> {
    if events.is_empty() {
        return vec![];
    }

    let start_ts = events
        .first()
        .unwrap_or_else(|| panic!("events should not be empty"))
        .event_timestamp;
    let end_ts = events
        .last()
        .unwrap_or_else(|| panic!("events should not be empty"))
        .event_timestamp;
    let bucket_secs: i64 = 5 * 60;

    let first_bucket = (start_ts / bucket_secs) * bucket_secs;
    let last_bucket = (end_ts / bucket_secs) * bucket_secs;

    let mut snapshots: Vec<TimeSnapshot> = Vec::new();
    let mut bull_acc = 0u32;
    let mut bear_acc = 0u32;
    let mut plate_map: ahash::AHashMap<String, PlateAccumulator> = ahash::AHashMap::new();
    let mut ev_idx = 0usize;

    let mut bucket_start = first_bucket;
    while bucket_start <= last_bucket {
        let bucket_end = bucket_start + bucket_secs;

        let mut bucket_bull = 0u32;
        let mut bucket_bear = 0u32;
        while ev_idx < events.len() && events[ev_idx].event_timestamp < bucket_end {
            let ev = &events[ev_idx];
            match ev.good_or_bad {
                1 => bucket_bull += 1,
                2 => bucket_bear += 1,
                _ => {}
            }
            if let Some(ref pd) = ev.plate_abnormal_event_data {
                if let (Some(ref name), Some(pcp)) = (&pd.plate_name, pd.pcp) {
                    let acc = plate_map
                        .entry(name.clone())
                        .or_insert_with(|| PlateAccumulator {
                            pcp,
                            good_or_bad: ev.good_or_bad,
                            related_stocks: pd.related_stocks.clone(),
                        });
                    if pcp.abs() > acc.pcp.abs() {
                        acc.pcp = pcp;
                        acc.good_or_bad = ev.good_or_bad;
                    }
                    if acc.related_stocks.len() < pd.related_stocks.len() {
                        acc.related_stocks.clone_from(&pd.related_stocks);
                    }
                }
            }
            ev_idx += 1;
        }

        bull_acc += bucket_bull;
        bear_acc += bucket_bear;

        let emotion = if bull_acc + bear_acc > 0 {
            f64::from(bull_acc) / f64::from(bull_acc + bear_acc) * 100.0
        } else {
            50.0
        };

        let mut top: Vec<TopPlate> = plate_map
            .drain()
            .map(|(plate_name, acc)| TopPlate {
                plate_name,
                pcp: acc.pcp,
                good_or_bad: acc.good_or_bad,
                related_stocks: acc.related_stocks,
            })
            .collect();
        top.sort_by(|a, b| {
            b.pcp
                .abs()
                .partial_cmp(&a.pcp.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top.truncate(5);

        let time_label = unix_to_hhmm(bucket_start);

        snapshots.push(TimeSnapshot {
            timestamp: bucket_start,
            time_label,
            emotion_score: (emotion * 10.0).round() / 10.0,
            bull_count: bull_acc,
            bear_count: bear_acc,
            top_plates: top,
        });

        bucket_start = bucket_end;
    }

    snapshots
}

fn unix_to_hhmm(ts: i64) -> String {
    // 时间戳是 UTC，展示统一用北京时间。
    let secs_since_midnight = (ts + 8 * 3600).rem_euclid(86400);
    let h = secs_since_midnight / 3600;
    let m = (secs_since_midnight % 3600) / 60;
    format!("{h:02}:{m:02}")
}

// ─── 指数走势（三指数并发） ──────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexTrendData {
    pub sh: Vec<IndexTick>,
    pub sz: Vec<IndexTick>,
    pub cyb: Vec<IndexTick>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexTick {
    pub tick_at: i64,
    pub close_px: f64,
}

/// Fetch index trends for major indices (SH, SZ, CYB).
///
/// # Errors
/// Returns an error if any of the HTTP requests fail.
pub async fn fetch_index_trends(client: &XuanguBaoClient) -> Result<IndexTrendData> {
    let (sh, sz, cyb) = tokio::try_join!(
        client.fetch_index_trend("000001.SS"),
        client.fetch_index_trend("399001.SZ"),
        client.fetch_index_trend("399006.SZ"),
    )?;
    Ok(IndexTrendData { sh, sz, cyb })
}

impl XuanguBaoClient {
    async fn fetch_index_trend(&self, prod_code: &str) -> Result<Vec<IndexTick>> {
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/trend?fields={}&prod_code={prod_code}",
            ApiField::join(&[ApiField::TickAt, ApiField::ClosePx])
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Trend API error: {}",
            resp["message"]
        );
        let candle = &resp["data"]["candle"][prod_code];
        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let tick_at_idx = fields.iter().position(|f| f == "tick_at");
        let close_idx = fields.iter().position(|f| f == "close_px");
        let lines = candle["lines"].as_array().cloned().unwrap_or_default();
        let ticks: Vec<IndexTick> = lines
            .iter()
            .filter_map(|line| {
                let arr = line.as_array()?;
                let tick_at = tick_at_idx.and_then(|i| arr.get(i)?.as_f64().map(f64_to_i64))?;
                let close_px = close_idx.and_then(|i| arr.get(i)?.as_f64())?;
                Some(IndexTick { tick_at, close_px })
            })
            .collect();
        Ok(ticks)
    }

    /// Fetch news stream from xuangubao.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_news_stream(
        &self,
        tailmark: Option<i64>,
        msg_id_mark: Option<&str>,
    ) -> Result<(Vec<NewsItem>, Option<i64>, Option<String>)> {
        let mut url = String::from(
            "https://api.xuangubao.com.cn/api/pc/msgs?subjids=9,723,10,35,469&limit=50",
        );
        if let Some(tm) = tailmark {
            use std::fmt::Write;
            writeln!(url, "&tailmark={tm}").ok();
        }
        if let Some(mid) = msg_id_mark {
            use std::fmt::Write;
            writeln!(url, "&msgIdMark={mid}").ok();
        }
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let items = resp["NewMsgs"].as_array().cloned().unwrap_or_default();
        // Debug: log first and last item timestamps
        if let (Some(first), Some(last)) = (items.first(), items.last()) {
            tracing::debug!(
                first_ts = ?first["CreatedAtInSec"].as_i64(),
                last_ts = ?last["CreatedAtInSec"].as_i64(),
                first_id = ?first["Id"].as_str(),
                "xuangubao::fetch_news_stream raw"
            );
        }
        let result: Vec<NewsItem> = items
            .iter()
            .filter_map(|v| {
                let id = v["Id"].as_str()?.parse::<i64>().ok()?;
                let title = v["Title"].as_str()?.to_string();
                let summary = v["Summary"].as_str().unwrap_or("").to_string();
                let created_at_in_sec = v["CreatedAtInSec"].as_i64()?;
                let updated_at_in_sec = v["UpdatedAtInSec"].as_i64().unwrap_or(created_at_in_sec);
                Some(NewsItem {
                    id,
                    created_at_in_sec,
                    updated_at_in_sec,
                    title,
                    content: String::new(),
                    summary,
                })
            })
            .collect();
        let next_tail = resp["TailMark"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok());
        let next_msg = resp["TailMsgId"]
            .as_str()
            .map(std::string::ToString::to_string);
        tracing::debug!(
            ?tailmark,
            count = result.len(),
            ?next_tail,
            "xuangubao::fetch_news_stream"
        );
        Ok((result, next_tail, next_msg))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct NewsItem {
    pub id: i64,
    pub created_at_in_sec: i64,
    pub updated_at_in_sec: i64,
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_market_trend_ohlcv_fields() {
        let response = serde_json::json!({
            "code": 20000,
            "data": {
                "fields": [
                    "tick_at",
                    "close_px",
                    "open_px",
                    "high_px",
                    "low_px",
                    "turnover_volume",
                    "px_change_rate"
                ],
                "candle": {
                    "600519.SS": {
                        "lines": [[
                            1792546200.0,
                            1331.88,
                            1341.0,
                            1342.0,
                            1327.88,
                            28170.0,
                            -1.3439
                        ]]
                    }
                }
            }
        });

        let trend = parse_market_trend("600519.SS", &response)
            .unwrap_or_else(|error| panic!("trend should parse: {error}"));
        assert_eq!(trend.symbol, "600519.SS");
        assert_eq!(trend.candles.len(), 1);
        let candle = &trend.candles[0];
        assert_eq!(candle.time, 1_792_546_200);
        assert_eq!(candle.close, 1331.88);
        assert_eq!(candle.open, Some(1341.0));
        assert_eq!(candle.high, Some(1342.0));
        assert_eq!(candle.low, Some(1327.88));
        assert_eq!(candle.volume, Some(28170.0));
        assert_eq!(candle.change_rate, Some(-1.3439));
    }

    #[test]
    fn replay_event_keeps_stock_fields() {
        let event = PlateOrStockAbnormalEvent {
            id: 7,
            target: "300750.SZ".to_string(),
            event_type: 11_001,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: Some(StockAbnormalEventData {
                symbol: Some("300750.SZ".to_string()),
                name: Some("宁德时代".to_string()),
                pcp: Some(-0.018),
                mtm: Some(1.0),
            }),
            plate_abnormal_event_data: Some(PlateAbnormalEventData::default()),
            good_or_bad: 2,
        };
        let replay = ReplayEvent::from(&event);
        assert_eq!(replay.stock_name.as_deref(), Some("宁德时代"));
        assert_eq!(replay.symbol.as_deref(), Some("300750.SZ"));
        assert_eq!(replay.pcp, Some(-0.018));
        assert!(replay.plate_name.is_none());
    }

    #[test]
    fn replay_event_uses_stock_pcp_when_plate_pcp_missing() {
        let event = PlateOrStockAbnormalEvent {
            id: 8,
            target: "64".to_string(),
            event_type: 11_000,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: Some(StockAbnormalEventData {
                symbol: Some("300750.SZ".to_string()),
                name: Some("宁德时代".to_string()),
                pcp: Some(0.021),
                mtm: Some(1.0),
            }),
            plate_abnormal_event_data: Some(PlateAbnormalEventData {
                plate_name: Some("机器人".to_string()),
                related_stocks: vec![RelatedStock {
                    symbol: "300750.SZ".to_string(),
                    name: "宁德时代".to_string(),
                    mtm: 1.0,
                    pcp: 0.021,
                }],
                ..PlateAbnormalEventData::default()
            }),
            good_or_bad: 1,
        };
        let replay = ReplayEvent::from(&event);
        assert_eq!(replay.plate_name.as_deref(), Some("机器人"));
        assert_eq!(replay.pcp, Some(0.021));
        assert_eq!(replay.related_stocks.len(), 1);
    }

    #[test]
    fn replay_duplicate_ids_are_removed_even_when_not_adjacent() {
        let make = |id| PlateOrStockAbnormalEvent {
            id,
            target: id.to_string(),
            event_type: 11_000,
            event_timestamp: 1_792_546_200,
            stock_abnormal_event_data: None,
            plate_abnormal_event_data: None,
            good_or_bad: 1,
        };
        let mut events = vec![make(1), make(2), make(1)];
        dedup_events_by_id(&mut events);
        assert_eq!(
            events.iter().map(|event| event.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn market_trend_missing_symbol_is_an_error() {
        let response = serde_json::json!({
            "code": 20000,
            "data": { "fields": [], "candle": {} }
        });
        assert!(parse_market_trend("600519.SS", &response).is_err());
    }
}
