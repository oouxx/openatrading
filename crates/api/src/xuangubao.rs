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

    /// Fetch market overview history for the given number of days.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_market_overview_history(
        &self,
        days: usize,
    ) -> Result<Vec<MarketOverviewDay>> {
        let fields = ApiField::join(&[
            ApiField::RiseCount,
            ApiField::FallCount,
            ApiField::LimitUpCount,
            ApiField::LimitDownCount,
            ApiField::LimitUpBrokenRatio,
            ApiField::MarketTemperature,
        ]);
        let url = format!("{}/market_indicator/line?fields={fields}", self.base_url);

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let arr = resp["data"].as_array().cloned().unwrap_or_default();
        let take = arr.len().min(days);
        let slice = if arr.len() > take {
            &arr[arr.len() - take..]
        } else {
            &arr
        };

        Ok(slice
            .iter()
            .map(|d| MarketOverviewDay {
                date: d["trade_date"].as_str().unwrap_or("").to_string(),
                rise_count: u32::try_from(d["rise_count"].as_u64().unwrap_or(0)).unwrap_or(0),
                fall_count: u32::try_from(d["fall_count"].as_u64().unwrap_or(0)).unwrap_or(0),
                limit_up_count: u32::try_from(d["limit_up_count"].as_u64().unwrap_or(0))
                    .unwrap_or(0),
                limit_down_count: u32::try_from(d["limit_down_count"].as_u64().unwrap_or(0))
                    .unwrap_or(0),
                bomb_rate: d["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
                market_temperature: d["market_temperature"].as_f64().unwrap_or(50.0),
            })
            .collect())
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
        let url = format!(
            "{}/market/trend?fields={}&prod_code={symbol}",
            "https://api-ddc-wscn.xuangubao.com.cn",
            &ApiField::join(&[ApiField::TickAt, ApiField::ClosePx]),
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Trend API error: {}",
            resp["message"]
        );
        let candle_data = &resp["data"]["candle"][symbol];
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
        Ok(MarketTrend {
            symbol: symbol.to_string(),
            candles,
        })
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

    /// Fetch all abnormal events for the current trading day.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_all_day_events(
        &self,
        min_pcp: f64,
    ) -> Result<Vec<PlateOrStockAbnormalEvent>> {
        let day_close_ts = Self::latest_trading_day_close_ts();
        let day_open_ts = day_close_ts - 5 * 3600 - 30 * 60;

        let mut all_events: Vec<PlateOrStockAbnormalEvent> = Vec::new();
        let page_size: u32 = 1000;
        let mut cursor = day_close_ts;

        // Paginate backwards until we cross day_open_ts
        loop {
            let page = self
                .fetch_events_page(cursor, page_size, "11001,11000")
                .await?;
            if page.is_empty() {
                break;
            }
            // The API returns events before `cursor`, newest first
            // The last event in the page is the earliest
            let earliest_in_page = page.last().map_or(cursor, |e| e.event_timestamp);
            let keep: Vec<_> = page
                .into_iter()
                .filter(|e| e.event_timestamp >= day_open_ts)
                .collect();
            all_events.extend(keep);

            if earliest_in_page <= day_open_ts {
                break;
            }
            cursor = earliest_in_page;
        }

        all_events.sort_by_key(|e| e.event_timestamp);
        all_events.dedup_by_key(|e| e.id);
        all_events.retain(|e| {
            e.plate_abnormal_event_data
                .as_ref()
                .and_then(|d| d.pcp)
                .is_some_and(|pcp| pcp.abs() > min_pcp)
        });

        tracing::debug!(
            count = all_events.len(),
            "xuangubao::fetch_all_day_events done"
        );
        Ok(all_events)
    }

    /// Unix timestamp for 15:00 on the latest completed trading day (Beijing time).
    /// If today's market hasn't closed yet, returns the previous trading day's close.
    /// Returns the Unix timestamp for 15:00 on the latest completed trading day (Beijing time).
    ///
    /// # Panics
    /// Panics if the date string cannot be parsed or the time 15:00:00 is invalid (should never happen).
    pub fn latest_trading_day_close_ts() -> i64 {
        let days = Self::last_n_trading_days(2);
        let now_beijing = chrono::Utc::now().with_timezone(&beijing_tz());
        let today_str = now_beijing.format("%Y-%m-%d").to_string();

        // If today is a trading day and it's before 15:00, use yesterday's close
        let date_str = if days.first().map(std::string::String::as_str) == Some(&today_str)
            && now_beijing.time()
                < chrono::NaiveTime::from_hms_opt(15, 0, 0)
                    .unwrap_or_else(|| panic!("15,0,0 is a valid time"))
        {
            days.get(1)
                .map_or("2026-01-01", std::string::String::as_str)
        } else {
            days.first()
                .map_or("2026-01-01", std::string::String::as_str)
        };

        let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("failed to parse date {date_str}: {e}"))
            .and_hms_opt(15, 0, 0)
            .unwrap_or_else(|| panic!("15:00:00 is a valid time"));
        let dt = naive.and_local_timezone(beijing_tz()).unwrap();
        dt.timestamp()
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
            &ApiField::join(&[
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
            &ApiField::join(&[
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
            &ApiField::join(&[
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
            &symbol
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
            &ApiField::join(&[
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
            &ApiField::join(&[ApiField::CoreAvgPcp, ApiField::PlateName]),
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

/// Lightweight event for the replay frontend — strips unused `PlateAbnormalEvent` fields.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplayEvent {
    pub id: i64,
    pub event_type: i32,
    pub event_timestamp: i64,
    pub good_or_bad: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plate_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcp: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_stocks: Vec<RelatedStock>,
}

impl From<&PlateOrStockAbnormalEvent> for ReplayEvent {
    fn from(ev: &PlateOrStockAbnormalEvent) -> Self {
        let (plate_name, pcp, related_stocks) = ev
            .plate_abnormal_event_data
            .as_ref()
            .map(|p| (p.plate_name.clone(), p.pcp, p.related_stocks.clone()))
            .unwrap_or_default();
        Self {
            id: ev.id,
            event_type: ev.event_type,
            event_timestamp: ev.event_timestamp,
            good_or_bad: ev.good_or_bad,
            plate_name,
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
            if ev.event_type == 11001 {
                if let Some(ref pd) = ev.plate_abnormal_event_data {
                    if let (Some(ref name), Some(pcp)) = (&pd.plate_name, pd.pcp) {
                        let acc =
                            plate_map
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
    let secs_since_midnight = ts.rem_euclid(86400);
    let h = secs_since_midnight / 3600;
    let m = (secs_since_midnight % 3600) / 60;
    format!("{h:02}:{m:02}")
}

/// Assemble the text prompt for AI analysis from snapshot data.
/// Build an analysis prompt from snapshots.
///
/// # Panics
/// Panics if writing to the string buffer fails (should never happen).
pub fn build_analysis_prompt(date: &str, snapshots: &[TimeSnapshot]) -> String {
    use std::fmt::Write;
    let mut table = String::from("时间       情绪温度  多/空  主线板块\n");

    for s in snapshots.iter().step_by(2) {
        let top_names: Vec<String> = s.top_plates.iter().map(|p| p.plate_name.clone()).collect();
        writeln!(
            table,
            "{}      {:.0}       {}/{:<2}  {}",
            s.time_label,
            s.emotion_score,
            s.bull_count,
            s.bear_count,
            top_names.join("、"),
        )
        .unwrap_or_else(|e| panic!("write to string should not fail: {e}"));
    }

    format!(
        r"你是一位经验丰富的A股短线交易复盘专家。请根据以下{date}行情回放数据，对全天行情进行结构化分析。

{table}

请从以下维度输出分析报告（Markdown格式）：
1. **全天情绪节奏**：情绪高点/低点出现时间，整体多空趋势
2. **主线板块**：全天最强板块，持续时长，是否有切换
3. **关键时间节点**：情绪突变点分析，可能的原因推断
4. **板块轮动特征**：资金从哪个板块流向哪个板块
5. **复盘结论**：当日适合什么策略，哪类股票机会更大，有什么风险需要注意

要求：分析具体、有数据支撑，避免泛泛而谈。",
    )
}

// ─── 指数走势（三指数并发） ──────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct IndexTrendData {
    pub sh: Vec<IndexTick>,
    pub sz: Vec<IndexTick>,
    pub cyb: Vec<IndexTick>,
}

#[derive(Debug, Serialize, Clone)]
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
            &ApiField::join(&[ApiField::TickAt, ApiField::ClosePx])
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
