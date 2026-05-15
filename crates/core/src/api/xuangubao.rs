use anyhow::{Context, Result};
use chrono::Datelike;
use serde::{Deserialize, Serialize};

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
    pub mtm: f64,
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
pub struct PlateAbnormalEvent {
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
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "BkYiDongType")]
    pub bk_yi_dong_type: i32,
    #[serde(rename = "BkjInfo")]
    pub bkj_info: BkjInfo,
    #[serde(rename = "CreatedAt")]
    pub created_at: i64,
    #[serde(rename = "UpdatedAt")]
    pub updated_at: i64,
}
// 板块信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BkjInfo {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
}
/*
      "BkYiDongType": 1,
      "BkjInfo": {
        "Id": "56044969",
        "Name": "数字人民币"
      },
      "CreatedAt": 1775797961,
      "UpdatedAt": 1775798032


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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct XuanGubaoExtra {
    pub limit_down: Vec<Stock>,
    pub plate_abnormal: Vec<PlateAbnormalEvent>,
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
            .build()
            .unwrap()
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

    pub async fn fetch_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_up").await
    }

    pub async fn fetch_limit_down(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("limit_down").await
    }

    pub async fn fetch_yesterday_limit_up(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("yesterday_limit_up").await
    }
    pub async fn fetch_super_stock(&self) -> Result<Vec<Stock>> {
        self.fetch_pool("super_stock").await
    }

    pub async fn fetch_market_overview(&self) -> Result<MarketOverview> {
        let fields = "rise_count,fall_count,limit_up_count,limit_down_count,limit_up_broken_ratio,market_temperature,yesterday_limit_up_avg_pcp";
        let url = format!("{}/market_indicator/line?fields={fields}", self.base_url);

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;

        let latest = &resp["data"]
            .as_array()
            .and_then(|a| a.last())
            .cloned()
            .unwrap_or_default();

        Ok(MarketOverview {
            rise_count: latest["rise_count"].as_u64().unwrap_or(0) as u32,
            fall_count: latest["fall_count"].as_u64().unwrap_or(0) as u32,
            limit_up_count: latest["limit_up_count"].as_u64().unwrap_or(0) as u32,
            limit_down_count: latest["limit_down_count"].as_u64().unwrap_or(0) as u32,
            bomb_rate: latest["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: latest["market_temperature"].as_f64().unwrap_or(50.0) as f64,
            yesterday_limit_up_avg_pcp: latest["yesterday_limit_up_avg_pcp"].as_f64().unwrap_or(0.0)
                as f64,
        })
    }

    pub async fn fetch_market_overview_history(
        &self,
        days: usize,
    ) -> Result<Vec<MarketOverviewDay>> {
        let fields = "rise_count,fall_count,limit_up_count,limit_down_count,limit_up_broken_ratio,market_temperature";
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
                rise_count: d["rise_count"].as_u64().unwrap_or(0) as u32,
                fall_count: d["fall_count"].as_u64().unwrap_or(0) as u32,
                limit_up_count: d["limit_up_count"].as_u64().unwrap_or(0) as u32,
                limit_down_count: d["limit_down_count"].as_u64().unwrap_or(0) as u32,
                bomb_rate: d["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
                market_temperature: d["market_temperature"].as_f64().unwrap_or(50.0),
            })
            .collect())
    }

    /// Fetch intraday market overview for a specific date, returns the last (most recent) snapshot.
    /// Generate the last `n` trading day dates (YYYY-MM-DD), skipping weekends.
    pub fn last_n_trading_days(n: usize) -> Vec<String> {
        let mut dates = Vec::with_capacity(n);
        let mut current = chrono::Local::now().date_naive();
        // If today is a weekend, start from Friday
        while dates.len() < n {
            match current.weekday() {
                chrono::Weekday::Sat => {
                    current = current.pred_opt().unwrap();
                    continue;
                }
                chrono::Weekday::Sun => {
                    current = current.pred_opt().unwrap();
                    continue;
                }
                _ => {
                    dates.push(current.format("%Y-%m-%d").to_string());
                    current = current.pred_opt().unwrap();
                }
            }
        }
        dates
    }

    pub async fn fetch_market_overview_for_date(
        &self,
        date: &str,
    ) -> Result<Option<MarketOverviewDay>> {
        let fields = "rise_count,fall_count,limit_up_count,limit_down_count,limit_up_broken_ratio,market_temperature";
        let url = format!(
            "{}/market_indicator/line?fields={fields}&date={date}",
            self.base_url
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
            rise_count: last["rise_count"].as_u64().unwrap_or(0) as u32,
            fall_count: last["fall_count"].as_u64().unwrap_or(0) as u32,
            limit_up_count: last["limit_up_count"].as_u64().unwrap_or(0) as u32,
            limit_down_count: last["limit_down_count"].as_u64().unwrap_or(0) as u32,
            bomb_rate: last["limit_up_broken_ratio"].as_f64().unwrap_or(0.0) * 100.0,
            market_temperature: last["market_temperature"].as_f64().unwrap_or(50.0),
        }))
    }
    pub async fn fetch_market_trend(&self, symbol: &str) -> Result<MarketTrend> {
        let url = format!(
            "{}/trend?fields=tick_at,close_px&prod_code={symbol}",
            self.base_url
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
                    time: get("tick_at")? as i64,
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

    pub async fn fetch_plate_abnormal(&self, count: i32) -> Result<Vec<PlateAbnormalEvent>> {
        let types =
            "10001,10005,10003,10007,10002,10006,10004,10008,10012,10014,10009,10010,11000,11001";
        let url = format!(
            "{}/event/history?count={}&types={}",
            self.base_url, count, types
        );

        let resp: ApiResponse<Vec<PlateAbnormalEvent>> =
            self.client.get(&url).send().await?.json().await?;

        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);
        Ok(resp.data)
    }
    // ---大盘异动---------------------------------------
    pub async fn fetch_dapan_abnormal(&self) -> Result<Vec<DapanAbnormal>> {
        let url = format!(
            "{}/api/messages/todayDaPanYiDong?headmark={}",
            self.base_url, 0
        );

        let resp: ApiResponse<Vec<DapanAbnormal>> =
            self.client.get(&url).send().await?.json().await?;

        Ok(resp.data)
    }

    // ─── K线数据 ───────────────────────────────────────

    pub async fn fetch_kline(
        &self,
        symbol: &str,
        period: KlinePeriod,
        count: u32,
    ) -> Result<KlineData> {
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/kline?prod_code={symbol}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward&fields=tick_at,open_px,close_px,high_px,low_px,turnover_volume,px_change_rate",
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Kline API error: {}",
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
                    time: get("tick_at")? as i64,
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
            symbol: symbol.to_string(),
            period: period.as_str().to_string(),
            candles,
        })
    }

    pub async fn fetch_realtime(&self, symbol: &str) -> Result<RealtimeQuote> {
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/real?fields=prod_name,trade_status,update_time,last_px,px_change,px_change_rate,preclose_px,open_px,high_px,low_px,amplitude,turnover_ratio,pe_rate,dyn_pe,dyn_pb_rate,market_value,circulation_value,turnover_volume,turnover_value,volume_ratio,circulation_shares,total_shares&prod_code={symbol}"
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            resp["code"] == 20000,
            "Realtime API error: {}",
            resp["message"]
        );

        let snap = &resp["data"]["snapshot"][symbol];
        let arr = snap.as_array().context("missing snapshot array")?;

        let get = |i: usize, default: f64| -> f64 {
            arr.get(i).and_then(|v| v.as_f64()).unwrap_or(default)
        };
        let get_str = |i: usize, default: &str| -> String {
            arr.get(i)
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string()
        };

        Ok(RealtimeQuote {
            symbol: symbol.to_string(),
            name: get_str(0, ""),
            trade_status: get_str(1, ""),
            update_time: arr.get(2).and_then(|v| v.as_i64()).unwrap_or(0),
            last_px: get(3, 0.0),
            px_change: get(4, 0.0),
            px_change_rate: get(5, 0.0),
            preclose_px: get(6, 0.0),
            open_px: get(7, 0.0),
            high_px: get(8, 0.0),
            low_px: get(9, 0.0),
            amplitude: get(10, 0.0),
            turnover_ratio: get(11, 0.0),
            pe_rate: get(12, 0.0),
            dyn_pe: get(13, 0.0),
            dyn_pb_rate: get(14, 0.0),
            market_value: get(15, 0.0),
            circulation_value: get(16, 0.0),
            turnover_volume: get(17, 0.0),
            turnover_value: get(18, 0.0),
            volume_ratio: get(21, 0.0),
            circulation_shares: get(22, 0.0),
            total_shares: get(23, 0.0),
        })
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
    pub fn as_seconds(&self) -> u32 {
        match self {
            KlinePeriod::Minute => 60,
            KlinePeriod::Day => 86400,
            KlinePeriod::Week => 604800,
            KlinePeriod::Month => 2592000,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            KlinePeriod::Minute => "minute",
            KlinePeriod::Day => "day",
            KlinePeriod::Week => "week",
            KlinePeriod::Month => "month",
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    pub name: String,
    pub last_px: f64,
    pub px_change: f64,
    pub px_change_rate: f64,
    pub open_px: f64,
    pub high_px: f64,
    pub low_px: f64,
    pub preclose_px: f64,
    pub amplitude: f64,
    pub turnover_ratio: f64,
    pub turnover_volume: f64,
    pub turnover_value: f64,
    pub market_value: f64,
    pub circulation_value: f64,
    pub pe_rate: f64,
    pub dyn_pe: f64,
    pub dyn_pb_rate: f64,
    pub volume_ratio: f64,
    pub circulation_shares: f64,
    pub total_shares: f64,
    pub trade_status: String,
    pub update_time: i64,
}
