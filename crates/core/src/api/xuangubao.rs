use anyhow::{Context, Result};
use chrono::{Datelike, FixedOffset};
use serde::{Deserialize, Serialize};

/// UTC+8 Beijing time — all A-share market clock calculations should use this.
fn beijing_tz() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).unwrap()
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
        let mut current = chrono::Utc::now().with_timezone(&beijing_tz()).date_naive();
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
            bomb_rate: last["limit_up_broken_ratio"].as_f64().unwrap_or(0.0),
            market_temperature: last["market_temperature"].as_f64().unwrap_or(50.0),
        }))
    }
    pub async fn fetch_market_trend(&self, symbol: &str) -> Result<MarketTrend> {
        let url = format!(
            "{}/market/trend?fields=tick_at,close_px&prod_code={symbol}",
            "https://api-ddc-wscn.xuangubao.com.cn"
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
    pub async fn fetch_market_real(&self, symbols: &[&str]) -> Result<Vec<IndexQuote>> {
        let joined = symbols.join(",");
        let fields = "prod_name,last_px,px_change,px_change_rate,symbol,trade_status";
        let url = format!(
            "https://api-ddc-wscn.xuangubao.com.cn/market/real?fields={fields}&prod_code={joined}"
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
                    .and_then(|v| v.as_f64())
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
    /// Cursor-paginate to fetch ALL events for the latest trading day (09:30–15:00).
    /// Returns events sorted by event_timestamp ascending.
    pub async fn fetch_events_page(
        &self,
        timestamp: i64,
        count: u32,
    ) -> Result<Vec<PlateAbnormalEvent>> {
        let url = format!(
            "{}/event/history?timestamp={timestamp}&count={count}&types=11000,11001",
            self.base_url
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let code = resp["code"].as_i64().unwrap_or(0);
        anyhow::ensure!(code == 20000, "API 返回错误码: {code}");
        let data = resp["data"].as_array().cloned().unwrap_or_default();
        let events: Vec<PlateAbnormalEvent> = data
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
        Ok(events)
    }

    pub async fn fetch_all_day_events(&self) -> Result<Vec<PlateAbnormalEvent>> {
        let day_close_ts = Self::latest_trading_day_close_ts();
        let day_open_ts = day_close_ts - 5 * 3600 - 30 * 60;

        let url = format!(
            "{}/event/history?timestamp={day_close_ts}&count=200&types=11000,11001",
            self.base_url
        );
        let resp: ApiResponse<Vec<PlateAbnormalEvent>> =
            self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(resp.code == 20000, "API 返回错误码: {}", resp.code);

        let mut all_events = resp.data;
        all_events.retain(|e| e.event_timestamp >= day_open_ts);
        all_events.sort_by_key(|e| e.event_timestamp);

        let plate_events = all_events.iter().filter(|e| e.event_type == 11001).count();
        let stock_events = all_events.iter().filter(|e| e.event_type == 11000).count();
        eprintln!(
            "[replay] fetch_all_day_events done: {} events (plate={plate_events}, stock={stock_events})",
            all_events.len()
        );

        Ok(all_events)
    }

    /// Unix timestamp for 15:00 on the latest completed trading day (Beijing time).
    /// If today's market hasn't closed yet, returns the previous trading day's close.
    pub fn latest_trading_day_close_ts() -> i64 {
        let days = Self::last_n_trading_days(2);
        let now_beijing = chrono::Utc::now().with_timezone(&beijing_tz());
        let today_str = now_beijing.format("%Y-%m-%d").to_string();

        // If today is a trading day and it's before 15:00, use yesterday's close
        let date_str = if days.first().map(|s| s.as_str()) == Some(&today_str)
            && now_beijing.time() < chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap()
        {
            days.get(1).map(|s| s.as_str()).unwrap_or("2026-01-01")
        } else {
            days.first().map(|s| s.as_str()).unwrap_or("2026-01-01")
        };

        let naive = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(15, 0, 0)
            .unwrap();
        let dt = naive.and_local_timezone(beijing_tz()).unwrap();
        dt.timestamp()
    }

    // ---大盘异动---------------------------------------
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

        let fields: Vec<String> = resp["data"]["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let snap = &resp["data"]["snapshot"][symbol];
        let arr = snap.as_array().context("missing snapshot array")?;

        let get_f64 = |name: &str| -> f64 {
            fields
                .iter()
                .position(|f| f == name)
                .and_then(|i| arr.get(i))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        };
        let get_str = |name: &str| -> String {
            fields
                .iter()
                .position(|f| f == name)
                .and_then(|i| arr.get(i))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_i64 = |name: &str| -> i64 {
            fields
                .iter()
                .position(|f| f == name)
                .and_then(|i| arr.get(i))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        };

        Ok(RealtimeQuote {
            symbol: symbol.to_string(),
            name: get_str("prod_name"),
            trade_status: get_str("trade_status"),
            update_time: get_i64("update_time"),
            last_px: get_f64("last_px"),
            px_change: get_f64("px_change"),
            px_change_rate: get_f64("px_change_rate"),
            preclose_px: get_f64("preclose_px"),
            open_px: get_f64("open_px"),
            high_px: get_f64("high_px"),
            low_px: get_f64("low_px"),
            amplitude: get_f64("amplitude"),
            turnover_ratio: get_f64("turnover_ratio"),
            pe_rate: get_f64("pe_rate"),
            dyn_pe: get_f64("dyn_pe"),
            dyn_pb_rate: get_f64("dyn_pb_rate"),
            market_value: get_f64("market_value"),
            circulation_value: get_f64("circulation_value"),
            turnover_volume: get_f64("turnover_volume"),
            turnover_value: get_f64("turnover_value"),
            volume_ratio: get_f64("volume_ratio"),
            circulation_shares: get_f64("circulation_shares"),
            total_shares: get_f64("total_shares"),
        })
    }
    pub async fn plates_by_any_stock(&self, symbol: &str) -> Result<Vec<PlateAndReason>> {
        let url = format!(
            "{}/stage2/plates_by_any_stock?symbol={symbol}&fields=core_avg_pcp,plate_name",
            self.base_url
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
                    plate_id: obj.get("plate_id")?.as_i64()? as i32,
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

// fields=prod_name(0),trade_status(1),update_time(2),last_px(3),px_change(4),
//   px_change_rate(5),preclose_px(6),open_px(7),high_px(8),low_px(9),
//   amplitude(10),turnover_ratio(11),pe_rate(12),dyn_pe(13),dyn_pb_rate(14),
//   market_value(15),circulation_value(16),turnover_volume(17),turnover_value(18),
//   volume_ratio(19),circulation_shares(20),total_shares(21)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    /// 0: prod_name
    pub name: String,
    /// 3: last_px
    pub last_px: f64,
    /// 4: px_change
    pub px_change: f64,
    /// 5: px_change_rate
    pub px_change_rate: f64,
    /// 7: open_px
    pub open_px: f64,
    /// 8: high_px
    pub high_px: f64,
    /// 9: low_px
    pub low_px: f64,
    /// 6: preclose_px
    pub preclose_px: f64,
    /// 10: amplitude
    pub amplitude: f64,
    /// 11: turnover_ratio
    pub turnover_ratio: f64,
    /// 17: turnover_volume
    pub turnover_volume: f64,
    /// 18: turnover_value
    pub turnover_value: f64,
    /// 15: market_value
    pub market_value: f64,
    /// 16: circulation_value
    pub circulation_value: f64,
    /// 12: pe_rate
    pub pe_rate: f64,
    /// 13: dyn_pe
    pub dyn_pe: f64,
    /// 14: dyn_pb_rate
    pub dyn_pb_rate: f64,
    /// 19: volume_ratio
    pub volume_ratio: f64,
    /// 20: circulation_shares
    pub circulation_shares: f64,
    /// 21: total_shares
    pub total_shares: f64,
    /// 1: trade_status
    pub trade_status: String,
    /// 2: update_time
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

/// Lightweight event for the replay frontend — strips unused PlateAbnormalEvent fields.
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

impl From<&PlateAbnormalEvent> for ReplayEvent {
    fn from(ev: &PlateAbnormalEvent) -> Self {
        let (plate_name, pcp, related_stocks) = ev
            .plate_abnormal_event_data
            .as_ref()
            .map(|p| (p.plate_name.clone(), p.pcp, p.related_stocks.clone()))
            .unwrap_or_default();
        ReplayEvent {
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
pub fn build_replay_snapshots(events: &[PlateAbnormalEvent]) -> Vec<TimeSnapshot> {
    if events.is_empty() {
        return vec![];
    }

    let start_ts = events.first().unwrap().event_timestamp;
    let end_ts = events.last().unwrap().event_timestamp;
    let bucket_secs: i64 = 5 * 60;

    let first_bucket = (start_ts / bucket_secs) * bucket_secs;
    let last_bucket = (end_ts / bucket_secs) * bucket_secs;

    let mut snapshots: Vec<TimeSnapshot> = Vec::new();
    let mut bull_acc = 0u32;
    let mut bear_acc = 0u32;
    let mut plate_map: std::collections::HashMap<String, PlateAccumulator> =
        std::collections::HashMap::new();
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
                            acc.related_stocks = pd.related_stocks.clone();
                        }
                    }
                }
            }
            ev_idx += 1;
        }

        bull_acc += bucket_bull;
        bear_acc += bucket_bear;

        let emotion = if bull_acc + bear_acc > 0 {
            bull_acc as f64 / (bull_acc + bear_acc) as f64 * 100.0
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
        top.sort_by(|a, b| b.pcp.abs().partial_cmp(&a.pcp.abs()).unwrap());
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
pub fn build_analysis_prompt(date: &str, snapshots: &[TimeSnapshot]) -> String {
    let mut table = String::from("时间       情绪温度  多/空  主线板块\n");

    for s in snapshots.iter().step_by(2) {
        let top_names: Vec<String> = s.top_plates.iter().map(|p| p.plate_name.clone()).collect();
        table.push_str(&format!(
            "{}      {:.0}       {}/{:<2}  {}\n",
            s.time_label,
            s.emotion_score,
            s.bull_count,
            s.bear_count,
            top_names.join("、"),
        ));
    }

    format!(
        r#"你是一位经验丰富的A股短线交易复盘专家。请根据以下{date}行情回放数据，对全天行情进行结构化分析。

{content}

请从以下维度输出分析报告（Markdown格式）：
1. **全天情绪节奏**：情绪高点/低点出现时间，整体多空趋势
2. **主线板块**：全天最强板块，持续时长，是否有切换
3. **关键时间节点**：情绪突变点分析，可能的原因推断
4. **板块轮动特征**：资金从哪个板块流向哪个板块
5. **复盘结论**：当日适合什么策略，哪类股票机会更大，有什么风险需要注意

要求：分析具体、有数据支撑，避免泛泛而谈。"#,
        date = date,
        content = table,
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
            "https://api-ddc-wscn.xuangubao.com.cn/market/trend?fields=tick_at,close_px&prod_code={prod_code}"
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
                let tick_at = tick_at_idx.and_then(|i| arr.get(i)?.as_f64())? as i64;
                let close_px = close_idx.and_then(|i| arr.get(i)?.as_f64())?;
                Some(IndexTick { tick_at, close_px })
            })
            .collect();
        Ok(ticks)
    }

    pub async fn fetch_news_stream(
        &self,
        tailmark: Option<i64>,
        msg_id_mark: Option<&str>,
    ) -> Result<(Vec<NewsItem>, Option<i64>, Option<String>)> {
        let mut url = String::from(
            "https://api.xuangubao.com.cn/api/pc/msgs?subjids=9,723,10,35,469&limit=50",
        );
        if let Some(tm) = tailmark {
            url.push_str(&format!("&tailmark={tm}"));
        }
        if let Some(mid) = msg_id_mark {
            url.push_str(&format!("&msgIdMark={mid}"));
        }
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let items = resp["NewMsgs"].as_array().cloned().unwrap_or_default();
        // Debug: log first and last item timestamps
        if let (Some(first), Some(last)) = (items.first(), items.last()) {
            eprintln!(
                "[replay] fetch_news_stream raw: first_ts={:?} last_ts={:?} first_id={:?}",
                first["CreatedAtInSec"].as_i64(),
                last["CreatedAtInSec"].as_i64(),
                first["Id"].as_str(),
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
        let next_msg = resp["TailMsgId"].as_str().map(|s| s.to_string());
        eprintln!(
            "[replay] fetch_news_stream: tailmark={tailmark:?} → {} items, next_tail={next_tail:?}",
            result.len()
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
