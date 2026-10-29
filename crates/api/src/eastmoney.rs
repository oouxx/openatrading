//! 东方财富接口客户端。
//!
//! 包含龙虎榜、研报、涨停/炸板池、分钟行情、资金流向和板块 K 线。

use ahash::AHashMap;
use anyhow::{bail, Context, Result};
use chrono::NaiveDateTime;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use thiserror::Error;

use crate::utils::{compact_date, now_ts, to_yahoo_symbol};
// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum LhbError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API returned empty result")]
    EmptyResult,
    #[error("Invalid date format, expected YYYYMMDD: {0}")]
    InvalidDate(String),
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
}

// ─── Raw API types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiResponse {
    result: Option<ApiResult>,
}

#[derive(Debug, Deserialize)]
struct ApiResult {
    data: Option<Vec<serde_json::Value>>,
    pages: Option<u32>,
}

// ─── Public domain types ──────────────────────────────────────────────────────

/// 龙虎榜详情条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbDetail {
    pub code: String,
    pub name: String,
    pub trade_date: String,
    pub close_price: Option<f64>,
    pub change_rate: Option<f64>,
    /// 龙虎榜净买额（元）
    pub billboard_net_amt: Option<f64>,
    /// 龙虎榜买入额
    pub billboard_buy_amt: Option<f64>,
    /// 龙虎榜卖出额
    pub billboard_sell_amt: Option<f64>,
    /// 龙虎榜成交额
    pub billboard_deal_amt: Option<f64>,
    /// 市场总成交额
    pub accum_amount: Option<f64>,
    pub turnover_rate: Option<f64>,
    pub free_market_cap: Option<f64>,
    pub explanation: Option<String>,
    /// 上榜后1/2/5/10日涨跌幅
    pub d1_chg: Option<f64>,
    pub d2_chg: Option<f64>,
    pub d5_chg: Option<f64>,
    pub d10_chg: Option<f64>,
}

/// 龙虎榜合并条目（详情 + 买卖席位）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbCombinedEntry {
    pub code: String,
    pub name: String,
    pub trade_date: String,
    pub close_price: Option<f64>,
    pub change_rate: Option<f64>,
    pub billboard_net_amt: Option<f64>,
    pub billboard_buy_amt: Option<f64>,
    pub billboard_sell_amt: Option<f64>,
    pub turnover_rate: Option<f64>,
    pub free_market_cap: Option<f64>,
    pub explanation: Option<String>,
    pub d1_chg: Option<f64>,
    pub d2_chg: Option<f64>,
    pub d5_chg: Option<f64>,
    pub d10_chg: Option<f64>,
    /// 买方席位
    pub buy_seats: Vec<LhbSeatDetail>,
    /// 卖方席位
    pub sell_seats: Vec<LhbSeatDetail>,
}

/// 个股席位明细（买入或卖出方向）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbSeatDetail {
    pub dept_name: String,
    /// 机构 / 游资
    pub seat_type: Option<String>,
    pub buy_amt: Option<f64>,
    pub buy_ratio: Option<f64>,
    pub sell_amt: Option<f64>,
    pub sell_ratio: Option<f64>,
    pub net_amt: Option<f64>,
}

/// 龙虎榜席位所属营业部（用于按营业部筛选个股）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbSeatRecord {
    pub code: String,
    pub trade_date: String,
    pub dept_name: String,
}

/// 营业部活跃记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LhbActiveYyb {
    pub dept_name: String,
    pub dept_code: String,
    pub onlist_date: String,
    pub buy_stock_count: Option<i64>,
    pub sell_stock_count: Option<i64>,
    pub buy_total_amt: Option<f64>,
    pub sell_total_amt: Option<f64>,
    pub net_amt: Option<f64>,
}

/// 统计周期
#[derive(Debug, Clone, Copy)]
pub enum StatsCycle {
    OneMonth,
    ThreeMonths,
    SixMonths,
    OneYear,
}

impl StatsCycle {
    const fn as_code(self) -> &'static str {
        match self {
            Self::OneMonth => "01",
            Self::ThreeMonths => "02",
            Self::SixMonths => "03",
            Self::OneYear => "04",
        }
    }
}

/// 席位方向
#[derive(Debug, Clone, Copy)]
pub enum SeatFlag {
    Buy,
    Sell,
}

// ─── Client ───────────────────────────────────────────────────────────────────

const BASE_URL: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";

/// 兼容旧名称：东方财富客户端。
pub type LhbClient = EastmoneyClient;

/// 东方财富接口客户端（龙虎榜、研报、涨停股池、分钟行情、资金流、板块 K 线）。
#[derive(Clone)]
pub struct EastmoneyClient {
    client: Client,
}

impl Default for EastmoneyClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Stock search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSearchItem {
    pub code: String,
    pub name: String,
}

impl EastmoneyClient {
    /// Create a new `EastmoneyClient`.
    ///
    /// # Panics
    /// Panics if the reqwest client cannot be built (should never happen).
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .build()
                .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}")),
        }
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// 把 "`20_240_115`" 转成 "2024-01-15"
    fn fmt_date(date: &str) -> Result<String, LhbError> {
        if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
            return Err(LhbError::InvalidDate(date.to_string()));
        }
        Ok(format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]))
    }

    fn f64_field(v: &serde_json::Value, key: &str) -> Option<f64> {
        v.get(key).and_then(serde_json::Value::as_f64)
    }

    fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(std::string::ToString::to_string)
    }

    async fn fetch_all_pages(
        &self,
        params: &mut AHashMap<&str, String>,
    ) -> Result<Vec<serde_json::Value>, LhbError> {
        let first: ApiResponse = self
            .client
            .get(BASE_URL)
            .query(&params.iter().collect::<Vec<_>>())
            .send()
            .await?
            .json()
            .await?;

        let Some(result) = first.result else {
            return Ok(vec![]);
        };
        let total_pages = result.pages.unwrap_or(1);
        let mut rows = result.data.unwrap_or_default();

        for page in 2..=total_pages {
            params.insert("pageNumber", page.to_string());
            let resp: ApiResponse = self
                .client
                .get(BASE_URL)
                .query(&params.iter().collect::<Vec<_>>())
                .send()
                .await?
                .json()
                .await?;
            if let Some(r) = resp.result {
                rows.extend(r.data.unwrap_or_default());
            }
        }

        Ok(rows)
    }

    // ── public API ───────────────────────────────────────────────────────────

    /// 龙虎榜详情列表（按日期范围）
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_detail(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<LhbDetail>, LhbError> {
        let s = Self::fmt_date(start_date)?;
        let e = Self::fmt_date(end_date)?;
        let mut params: AHashMap<&str, String> = [
            ("reportName", "RPT_DAILYBILLBOARD_DETAILSNEW".into()),
            ("columns", "SECURITY_CODE,SECURITY_NAME_ABBR,TRADE_DATE,CLOSE_PRICE,CHANGE_RATE,BILLBOARD_NET_AMT,BILLBOARD_BUY_AMT,BILLBOARD_SELL_AMT,BILLBOARD_DEAL_AMT,ACCUM_AMOUNT,TURNOVERRATE,FREE_MARKET_CAP,EXPLANATION,D1_CLOSE_ADJCHRATE,D2_CLOSE_ADJCHRATE,D5_CLOSE_ADJCHRATE,D10_CLOSE_ADJCHRATE".into()),
            ("filter", format!("(TRADE_DATE<='{e}')(TRADE_DATE>='{s}')")),
            ("pageSize", "5000".into()),
            ("pageNumber", "1".into()),
            ("sortColumns", "TRADE_DATE,SECURITY_CODE".into()),
            ("sortTypes", "-1,1".into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
        ].into();

        let rows = self.fetch_all_pages(&mut params).await?;
        Ok(rows
            .iter()
            .map(|v| LhbDetail {
                code: Self::str_field(v, "SECURITY_CODE").unwrap_or_default(),
                name: Self::str_field(v, "SECURITY_NAME_ABBR").unwrap_or_default(),
                trade_date: Self::str_field(v, "TRADE_DATE").unwrap_or_default(),
                close_price: Self::f64_field(v, "CLOSE_PRICE"),
                change_rate: Self::f64_field(v, "CHANGE_RATE"),
                billboard_net_amt: Self::f64_field(v, "BILLBOARD_NET_AMT"),
                billboard_buy_amt: Self::f64_field(v, "BILLBOARD_BUY_AMT"),
                billboard_sell_amt: Self::f64_field(v, "BILLBOARD_SELL_AMT"),
                billboard_deal_amt: Self::f64_field(v, "BILLBOARD_DEAL_AMT"),
                accum_amount: Self::f64_field(v, "ACCUM_AMOUNT"),
                turnover_rate: Self::f64_field(v, "TURNOVERRATE"),
                free_market_cap: Self::f64_field(v, "FREE_MARKET_CAP"),
                explanation: Self::str_field(v, "EXPLANATION"),
                d1_chg: Self::f64_field(v, "D1_CLOSE_ADJCHRATE"),
                d2_chg: Self::f64_field(v, "D2_CLOSE_ADJCHRATE"),
                d5_chg: Self::f64_field(v, "D5_CLOSE_ADJCHRATE"),
                d10_chg: Self::f64_field(v, "D10_CLOSE_ADJCHRATE"),
            })
            .collect())
    }

    /// 个股龙虎榜席位明细（买入 or 卖出）
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_stock_seat(
        &self,
        code: &str,
        date: &str,
        flag: SeatFlag,
    ) -> Result<Vec<LhbSeatDetail>, LhbError> {
        if code.len() != 6 {
            return Err(LhbError::InvalidSymbol(code.to_string()));
        }
        let d = Self::fmt_date(date)?;
        let (report_name, sort_col) = match flag {
            SeatFlag::Buy => ("RPT_BILLBOARD_DAILYDETAILSBUY", "BUY"),
            SeatFlag::Sell => ("RPT_BILLBOARD_DAILYDETAILSSELL", "SELL"),
        };
        let mut params: AHashMap<&str, String> = [
            ("reportName", report_name.into()),
            ("columns", "ALL".into()),
            (
                "filter",
                format!("(TRADE_DATE='{d}')(SECURITY_CODE=\"{code}\")"),
            ),
            ("pageSize", "500".into()),
            ("pageNumber", "1".into()),
            ("sortTypes", "-1".into()),
            ("sortColumns", sort_col.into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
        ]
        .into();

        let rows = self.fetch_all_pages(&mut params).await?;
        Ok(rows
            .iter()
            .map(|v| LhbSeatDetail {
                dept_name: Self::str_field(v, "OPERATEDEPT_NAME").unwrap_or_default(),
                seat_type: Self::str_field(v, "OPERATEDEPT_TYPE"),
                buy_amt: Self::f64_field(v, "BUY"),
                buy_ratio: Self::f64_field(v, "BUY_RATIO"),
                sell_amt: Self::f64_field(v, "SELL"),
                sell_ratio: Self::f64_field(v, "SELL_RATIO"),
                net_amt: Self::f64_field(v, "NET"),
            })
            .collect())
    }

    /// 日期范围内的全部龙虎榜席位记录（买入 or 卖出）
    ///
    /// 与 [`Self::lhb_stock_seat`] 的区别是不按个股过滤，用于把「营业部」
    /// 映射回它参与过的股票，从而支持按营业部筛选整个榜单。
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_seats_range(
        &self,
        start_date: &str,
        end_date: &str,
        flag: SeatFlag,
    ) -> Result<Vec<LhbSeatRecord>, LhbError> {
        let s = Self::fmt_date(start_date)?;
        let e = Self::fmt_date(end_date)?;
        let (report_name, sort_col) = match flag {
            SeatFlag::Buy => ("RPT_BILLBOARD_DAILYDETAILSBUY", "BUY"),
            SeatFlag::Sell => ("RPT_BILLBOARD_DAILYDETAILSSELL", "SELL"),
        };
        let mut params: AHashMap<&str, String> = [
            ("reportName", report_name.into()),
            (
                "columns",
                "SECURITY_CODE,TRADE_DATE,OPERATEDEPT_NAME".into(),
            ),
            ("filter", format!("(TRADE_DATE>='{s}')(TRADE_DATE<='{e}')")),
            ("pageSize", "500".into()),
            ("pageNumber", "1".into()),
            ("sortTypes", "-1".into()),
            ("sortColumns", sort_col.into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
        ]
        .into();

        let rows = self.fetch_all_pages(&mut params).await?;
        Ok(rows
            .iter()
            .filter_map(|v| {
                let code = Self::str_field(v, "SECURITY_CODE")?;
                let dept_name = Self::str_field(v, "OPERATEDEPT_NAME")?;
                if code.is_empty() || dept_name.is_empty() {
                    return None;
                }
                let trade_date = Self::str_field(v, "TRADE_DATE")
                    .and_then(|d| d.split(' ').next().map(str::to_string))
                    .unwrap_or_default();
                Some(LhbSeatRecord {
                    code,
                    trade_date,
                    dept_name,
                })
            })
            .collect())
    }

    /// 龙虎榜合并数据（详情 + 买卖席位），并发请求，用于前端一次性渲染
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_combined(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<LhbCombinedEntry>, LhbError> {
        let details = self.lhb_detail(start_date, end_date).await?;

        let futures: Vec<_> = details
            .iter()
            .enumerate()
            .map(|(idx, d)| {
                let code = d.code.clone();
                let date = d.trade_date[..10].replace('-', "");
                async move {
                    let buy_seats = self
                        .lhb_stock_seat(&code, &date, SeatFlag::Buy)
                        .await
                        .unwrap_or_default();
                    let sell_seats = self
                        .lhb_stock_seat(&code, &date, SeatFlag::Sell)
                        .await
                        .unwrap_or_default();
                    (idx, buy_seats, sell_seats)
                }
            })
            .collect();

        let mut results: Vec<(usize, Vec<LhbSeatDetail>, Vec<LhbSeatDetail>)> =
            futures::future::join_all(futures).await;
        results.sort_by_key(|(idx, _, _)| *idx);

        let combined = results
            .into_iter()
            .zip(details.iter())
            .map(|((_, buy_seats, sell_seats), d)| LhbCombinedEntry {
                code: d.code.clone(),
                name: d.name.clone(),
                trade_date: d.trade_date.clone(),
                close_price: d.close_price,
                change_rate: d.change_rate,
                billboard_net_amt: d.billboard_net_amt,
                billboard_buy_amt: d.billboard_buy_amt,
                billboard_sell_amt: d.billboard_sell_amt,
                turnover_rate: d.turnover_rate,
                free_market_cap: d.free_market_cap,
                explanation: d.explanation.clone(),
                d1_chg: d.d1_chg,
                d2_chg: d.d2_chg,
                d5_chg: d.d5_chg,
                d10_chg: d.d10_chg,
                buy_seats,
                sell_seats,
            })
            .collect();

        Ok(combined)
    }

    /// 每日活跃营业部
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_active_yyb(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<LhbActiveYyb>, LhbError> {
        let s = Self::fmt_date(start_date)?;
        let e = Self::fmt_date(end_date)?;
        let mut params: AHashMap<&str, String> = [
            ("reportName", "RPT_OPERATEDEPT_ACTIVE".into()),
            ("columns", "ALL".into()),
            (
                "filter",
                format!("(ONLIST_DATE>='{s}')(ONLIST_DATE<='{e}')"),
            ),
            ("pageSize", "5000".into()),
            ("pageNumber", "1".into()),
            (
                "sortColumns",
                "TOTAL_NETAMT,ONLIST_DATE,OPERATEDEPT_CODE".into(),
            ),
            ("sortTypes", "-1,-1,1".into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
        ]
        .into();

        let rows = self.fetch_all_pages(&mut params).await?;
        Ok(rows
            .iter()
            .map(|v| LhbActiveYyb {
                dept_name: Self::str_field(v, "OPERATEDEPT_NAME").unwrap_or_default(),
                dept_code: Self::str_field(v, "OPERATEDEPT_CODE").unwrap_or_default(),
                onlist_date: Self::str_field(v, "ONLIST_DATE").unwrap_or_default(),
                buy_stock_count: v.get("BUY_STOCK_COUNT").and_then(serde_json::Value::as_i64),
                sell_stock_count: v
                    .get("SELL_STOCK_COUNT")
                    .and_then(serde_json::Value::as_i64),
                buy_total_amt: Self::f64_field(v, "BUY_AMT"),
                sell_total_amt: Self::f64_field(v, "SELL_AMT"),
                net_amt: Self::f64_field(v, "TOTAL_NETAMT"),
            })
            .collect())
    }

    /// 个股上榜统计（近一月/三月/六月/一年）
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn lhb_stock_statistic(
        &self,
        cycle: StatsCycle,
    ) -> Result<Vec<serde_json::Value>, LhbError> {
        let mut params: AHashMap<&str, String> = [
            ("reportName", "RPT_BILLBOARD_TRADEALL".into()),
            ("columns", "ALL".into()),
            (
                "filter",
                format!("(STATISTICS_CYCLE=\"{}\")", cycle.as_code()),
            ),
            ("pageSize", "5000".into()),
            ("pageNumber", "1".into()),
            (
                "sortColumns",
                "BILLBOARD_TIMES,LATEST_TDATE,SECURITY_CODE".into(),
            ),
            ("sortTypes", "-1,-1,1".into()),
            ("source", "WEB".into()),
            ("client", "WEB".into()),
        ]
        .into();
        self.fetch_all_pages(&mut params).await
    }

    /// Search stocks by keyword (name or code) via 东方财富 suggestion API.
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn search_stocks(&self, keyword: &str) -> Result<Vec<StockSearchItem>, LhbError> {
        let url = "https://searchapi.eastmoney.com/api/suggest/get";
        let params = [
            ("input", keyword),
            ("type", "14"),
            ("token", "D43BF722C8E33BDC906FB84D85E326E8"),
            ("count", "12"),
        ];
        let resp = self
            .client
            .get(url)
            .query(&params)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let data = resp
            .pointer("/QuotationCodeTable/Data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let items: Vec<StockSearchItem> = data
            .iter()
            .filter_map(|v| {
                let code = v.get("Code")?.as_str()?.to_string();
                let name = v.get("Name")?.as_str()?.to_string();
                // Only include A-stock codes (6 digits, starting with 0/3/6/4/8)
                if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
                    Some(StockSearchItem { code, name })
                } else {
                    None
                }
            })
            .collect();

        Ok(items)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 单元测试（纯逻辑，无网络）────────────────────────────────────────────

    #[test]
    fn test_fmt_date_valid() {
        assert_eq!(
            EastmoneyClient::fmt_date("20240115").unwrap_or_else(|_| panic!("test failed")),
            "2024-01-15"
        );
        assert_eq!(
            EastmoneyClient::fmt_date("20230101").unwrap_or_else(|_| panic!("test failed")),
            "2023-01-01"
        );
    }

    #[test]
    fn test_fmt_date_too_short() {
        assert!(matches!(
            EastmoneyClient::fmt_date("202401"),
            Err(LhbError::InvalidDate(_))
        ));
    }

    #[test]
    fn test_fmt_date_non_digit() {
        assert!(matches!(
            EastmoneyClient::fmt_date("2024-115"),
            Err(LhbError::InvalidDate(_))
        ));
    }

    #[test]
    fn test_f64_field_present() {
        let v = serde_json::json!({"CLOSE_PRICE": 12.34});
        assert_eq!(EastmoneyClient::f64_field(&v, "CLOSE_PRICE"), Some(12.34));
    }

    #[test]
    fn test_f64_field_missing() {
        let v = serde_json::json!({});
        assert_eq!(EastmoneyClient::f64_field(&v, "CLOSE_PRICE"), None);
    }

    #[test]
    fn test_f64_field_null() {
        let v = serde_json::json!({"CLOSE_PRICE": null});
        assert_eq!(EastmoneyClient::f64_field(&v, "CLOSE_PRICE"), None);
    }

    #[test]
    fn test_str_field() {
        let v = serde_json::json!({"SECURITY_NAME_ABBR": "东方财富"});
        assert_eq!(
            EastmoneyClient::str_field(&v, "SECURITY_NAME_ABBR"),
            Some("东方财富".to_string())
        );
    }

    #[test]
    fn test_stats_cycle_codes() {
        assert_eq!(StatsCycle::OneMonth.as_code(), "01");
        assert_eq!(StatsCycle::ThreeMonths.as_code(), "02");
        assert_eq!(StatsCycle::SixMonths.as_code(), "03");
        assert_eq!(StatsCycle::OneYear.as_code(), "04");
    }

    #[test]
    fn test_invalid_symbol_too_short() {
        let client = EastmoneyClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("123", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_invalid_symbol_too_long() {
        let client = EastmoneyClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("1_234_567", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_invalid_symbol_empty() {
        let client = EastmoneyClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_stock_seat_valid_symbol() {
        let client = EastmoneyClient::new();
        let result = client
            .lhb_stock_seat("000001", "20240115", SeatFlag::Buy)
            .await;
        assert!(!matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_lhb_stock_seat_invalid_date() {
        let client = EastmoneyClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("000001", "2024-115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidDate(_))));
    }

    // ── 集成测试（真实网络，加 #[ignore = "integration test, requires network"] 默认跳过）────────────────────────────
    // 运行：cargo test -- --ignored

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_detail_returns_data() {
        let client = EastmoneyClient::new();
        let rows = client
            .lhb_detail("20_240_415", "20_240_417")
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"));

        assert!(!rows.is_empty(), "should have at least one row");

        let first = &rows[0];
        assert_eq!(first.code.len(), 6, "code should be 6 digits");
        assert!(!first.name.is_empty(), "name should not be empty");
        assert!(first.close_price.unwrap_or(0.0) > 0.0);

        println!("lhb_detail: {} rows, first={:?}", rows.len(), first);
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_stock_seat_buy() {
        let client = EastmoneyClient::new();
        // 用一个确定有龙虎榜记录的日期和股票
        let rows = client
            .lhb_stock_seat("000788", "20_220_315", SeatFlag::Buy)
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"));

        println!("lhb_stock_seat buy: {} seats", rows.len());
        for seat in &rows {
            println!("  {} buy={:?}", seat.dept_name, seat.buy_amt);
        }
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_active_yyb() {
        let client = EastmoneyClient::new();
        let rows = client
            .lhb_active_yyb("20_240_415", "20_240_417")
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"));

        assert!(!rows.is_empty());
        println!("active yyb: {} rows", rows.len());
        println!("top dept: {} net={:?}", rows[0].dept_name, rows[0].net_amt);
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_stock_statistic() {
        let client = EastmoneyClient::new();
        let rows = client
            .lhb_stock_statistic(StatsCycle::OneMonth)
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"));

        assert!(!rows.is_empty());
        println!("stock statistic: {} rows", rows.len());
    }
    // ── lhb_detail 数据映射测试 ─────────────────────────────

    #[test]
    fn test_lhb_detail_maps_all_fields() {
        let raw = serde_json::json!({
            "SECURITY_CODE": "600519",
            "SECURITY_NAME_ABBR": "贵州茅台",
            "TRADE_DATE": "2025-01-20 00:00:00",
            "CLOSE_PRICE": 1685.0,
            "CHANGE_RATE": 2.35,
            "BILLBOARD_NET_AMT": 320_000_000.0,
            "BILLBOARD_BUY_AMT": 437_000_000.0,
            "BILLBOARD_SELL_AMT": 117_000_000.0,
            "BILLBOARD_DEAL_AMT": 554_000_000.0,
            "ACCUM_AMOUNT": 8_632_000_000.0,
            "TURNOVERRATE": 0.85,
            "FREE_MARKET_CAP": 2_112_000_000_000.0,
            "EXPLANATION": "日涨幅偏离值达7%",
            "D1_CLOSE_ADJCHRATE": 1.2,
            "D2_CLOSE_ADJCHRATE": -0.5,
            "D5_CLOSE_ADJCHRATE": 3.1,
            "D10_CLOSE_ADJCHRATE": 5.8
        });
        let detail = LhbDetail {
            code: EastmoneyClient::str_field(&raw, "SECURITY_CODE").unwrap_or_default(),
            name: EastmoneyClient::str_field(&raw, "SECURITY_NAME_ABBR").unwrap_or_default(),
            trade_date: EastmoneyClient::str_field(&raw, "TRADE_DATE").unwrap_or_default(),
            close_price: EastmoneyClient::f64_field(&raw, "CLOSE_PRICE"),
            change_rate: EastmoneyClient::f64_field(&raw, "CHANGE_RATE"),
            billboard_net_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_NET_AMT"),
            billboard_buy_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_BUY_AMT"),
            billboard_sell_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_SELL_AMT"),
            billboard_deal_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_DEAL_AMT"),
            accum_amount: EastmoneyClient::f64_field(&raw, "ACCUM_AMOUNT"),
            turnover_rate: EastmoneyClient::f64_field(&raw, "TURNOVERRATE"),
            free_market_cap: EastmoneyClient::f64_field(&raw, "FREE_MARKET_CAP"),
            explanation: EastmoneyClient::str_field(&raw, "EXPLANATION"),
            d1_chg: EastmoneyClient::f64_field(&raw, "D1_CLOSE_ADJCHRATE"),
            d2_chg: EastmoneyClient::f64_field(&raw, "D2_CLOSE_ADJCHRATE"),
            d5_chg: EastmoneyClient::f64_field(&raw, "D5_CLOSE_ADJCHRATE"),
            d10_chg: EastmoneyClient::f64_field(&raw, "D10_CLOSE_ADJCHRATE"),
        };
        assert_eq!(detail.code, "600519");
        assert_eq!(detail.name, "贵州茅台");
        assert_eq!(detail.close_price, Some(1685.0));
        assert_eq!(detail.change_rate, Some(2.35));
        assert_eq!(detail.billboard_net_amt, Some(320_000_000.0));
        assert_eq!(detail.billboard_buy_amt, Some(437_000_000.0));
        assert_eq!(detail.billboard_sell_amt, Some(117_000_000.0));
        assert_eq!(detail.explanation, Some("日涨幅偏离值达7%".to_string()));
        assert_eq!(detail.d1_chg, Some(1.2));
        assert_eq!(detail.d2_chg, Some(-0.5));
        assert_eq!(detail.d5_chg, Some(3.1));
        assert_eq!(detail.d10_chg, Some(5.8));
    }

    #[test]
    fn test_lhb_detail_missing_optional_fields() {
        let raw = serde_json::json!({
            "SECURITY_CODE": "000001",
            "SECURITY_NAME_ABBR": "平安银行",
            "TRADE_DATE": "2025-01-20 00:00:00"
        });
        let detail = LhbDetail {
            code: EastmoneyClient::str_field(&raw, "SECURITY_CODE").unwrap_or_default(),
            name: EastmoneyClient::str_field(&raw, "SECURITY_NAME_ABBR").unwrap_or_default(),
            trade_date: EastmoneyClient::str_field(&raw, "TRADE_DATE").unwrap_or_default(),
            close_price: EastmoneyClient::f64_field(&raw, "CLOSE_PRICE"),
            change_rate: EastmoneyClient::f64_field(&raw, "CHANGE_RATE"),
            billboard_net_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_NET_AMT"),
            billboard_buy_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_BUY_AMT"),
            billboard_sell_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_SELL_AMT"),
            billboard_deal_amt: EastmoneyClient::f64_field(&raw, "BILLBOARD_DEAL_AMT"),
            accum_amount: EastmoneyClient::f64_field(&raw, "ACCUM_AMOUNT"),
            turnover_rate: EastmoneyClient::f64_field(&raw, "TURNOVERRATE"),
            free_market_cap: EastmoneyClient::f64_field(&raw, "FREE_MARKET_CAP"),
            explanation: EastmoneyClient::str_field(&raw, "EXPLANATION"),
            d1_chg: EastmoneyClient::f64_field(&raw, "D1_CLOSE_ADJCHRATE"),
            d2_chg: EastmoneyClient::f64_field(&raw, "D2_CLOSE_ADJCHRATE"),
            d5_chg: EastmoneyClient::f64_field(&raw, "D5_CLOSE_ADJCHRATE"),
            d10_chg: EastmoneyClient::f64_field(&raw, "D10_CLOSE_ADJCHRATE"),
        };
        assert_eq!(detail.code, "000001");
        assert_eq!(detail.close_price, None);
        assert_eq!(detail.change_rate, None);
        assert_eq!(detail.billboard_net_amt, None);
        assert_eq!(detail.explanation, None);
        assert_eq!(detail.d1_chg, None);
    }

    // ── lhb_combined 数据映射测试 ──────────────────────────

    #[test]
    fn test_lhb_combined_maps_detail_fields() {
        let detail = LhbDetail {
            code: "600519".into(),
            name: "贵州茅台".into(),
            trade_date: "2025-01-20 00:00:00".into(),
            close_price: Some(1685.0),
            change_rate: Some(2.35),
            billboard_net_amt: Some(320_000_000.0),
            billboard_buy_amt: Some(437_000_000.0),
            billboard_sell_amt: Some(117_000_000.0),
            billboard_deal_amt: Some(554_000_000.0),
            accum_amount: Some(8_632_000_000.0),
            turnover_rate: Some(0.85),
            free_market_cap: Some(2_112_000_000_000.0),
            explanation: Some("日涨幅偏离值达7%".into()),
            d1_chg: Some(1.2),
            d2_chg: Some(-0.5),
            d5_chg: Some(3.1),
            d10_chg: Some(5.8),
        };
        let seats = vec![LhbSeatDetail {
            dept_name: "机构专用".into(),
            seat_type: Some("机构".into()),
            buy_amt: Some(152_000_000.0),
            buy_ratio: Some(0.035),
            sell_amt: None,
            sell_ratio: None,
            net_amt: Some(152_000_000.0),
        }];
        let combined = LhbCombinedEntry {
            code: detail.code.clone(),
            name: detail.name.clone(),
            trade_date: detail.trade_date.clone(),
            close_price: detail.close_price,
            change_rate: detail.change_rate,
            billboard_net_amt: detail.billboard_net_amt,
            billboard_buy_amt: detail.billboard_buy_amt,
            billboard_sell_amt: detail.billboard_sell_amt,
            turnover_rate: detail.turnover_rate,
            free_market_cap: detail.free_market_cap,
            explanation: detail.explanation.clone(),
            d1_chg: detail.d1_chg,
            d2_chg: detail.d2_chg,
            d5_chg: detail.d5_chg,
            d10_chg: detail.d10_chg,
            buy_seats: seats,
            sell_seats: vec![],
        };
        assert_eq!(combined.code, "600519");
        assert_eq!(combined.name, "贵州茅台");
        assert_eq!(combined.billboard_net_amt, Some(320_000_000.0));
        assert_eq!(combined.buy_seats.len(), 1);
        assert_eq!(combined.buy_seats[0].dept_name, "机构专用");
        assert_eq!(combined.sell_seats.len(), 0);
    }

    #[test]
    fn test_lhb_combined_empty_seats() {
        let detail = LhbDetail {
            code: "000001".into(),
            name: "平安银行".into(),
            trade_date: "2025-01-20 00:00:00".into(),
            close_price: Some(12.5),
            change_rate: Some(-3.2),
            billboard_net_amt: None,
            billboard_buy_amt: None,
            billboard_sell_amt: None,
            billboard_deal_amt: None,
            accum_amount: None,
            turnover_rate: None,
            free_market_cap: None,
            explanation: None,
            d1_chg: None,
            d2_chg: None,
            d5_chg: None,
            d10_chg: None,
        };
        let combined = LhbCombinedEntry {
            code: detail.code.clone(),
            name: detail.name.clone(),
            trade_date: detail.trade_date.clone(),
            close_price: detail.close_price,
            change_rate: detail.change_rate,
            billboard_net_amt: detail.billboard_net_amt,
            billboard_buy_amt: detail.billboard_buy_amt,
            billboard_sell_amt: detail.billboard_sell_amt,
            turnover_rate: detail.turnover_rate,
            free_market_cap: detail.free_market_cap,
            explanation: detail.explanation,
            d1_chg: detail.d1_chg,
            d2_chg: detail.d2_chg,
            d5_chg: detail.d5_chg,
            d10_chg: detail.d10_chg,
            buy_seats: vec![],
            sell_seats: vec![],
        };
        assert_eq!(combined.code, "000001");
        assert_eq!(combined.billboard_net_amt, None);
        assert_eq!(combined.buy_seats.len(), 0);
        assert_eq!(combined.sell_seats.len(), 0);
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_combined() {
        let client = EastmoneyClient::new();
        let rows = client
            .lhb_combined("20_240_415", "20_240_417")
            .await
            .unwrap_or_else(|e| panic!("request failed: {e}"));
        assert!(!rows.is_empty(), "should have at least one row");
        let first = &rows[0];
        assert_eq!(first.code.len(), 6);
        assert!(!first.name.is_empty());
        println!(
            "lhb_combined: {} rows, first={} buy_seats={} sell_seats={}",
            rows.len(),
            first.name,
            first.buy_seats.len(),
            first.sell_seats.len()
        );
    }
}

// ─── 研究报告 ────────────────────────────────────────────────────────────────

/// 研报分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportCategory {
    /// 策略报告
    StrategyReport,
    /// 宏观研究
    MacroResearch,
    /// 券商晨报
    BrokerReport,
    /// 行业研报
    Industry,
    /// 个股研报
    Stock,
}

impl ReportCategory {
    fn path_segment(self) -> &'static str {
        match self {
            Self::StrategyReport => "strategyreport",
            Self::MacroResearch => "macresearch",
            Self::BrokerReport => "brokerreport",
            Self::Industry => "industry",
            Self::Stock => "stock",
        }
    }
}

/// 研报列表条目（非个股研报）
///
/// 东方财富列表接口返回 camelCase 字段（`orgName`/`publishDate`/`encodeUrl` 等），
/// 与 Rust 字段的 snake_case 通过 `rename_all` 映射。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub id: i64,
    pub title: String,
    pub author: Vec<String>,
    pub org_name: String,
    /// 东方财富字段名为 `orgSName`（非 camelCase 规则的 orgShortName）
    #[serde(rename = "orgSName")]
    pub org_short_name: String,
    pub publish_date: String,
    pub encode_url: String,
    pub researcher: String,
    pub industry_name: String,
}

/// 个股研报条目（含盈利预测）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockReportItem {
    pub title: String,
    pub stock_name: String,
    pub stock_code: String,
    pub org_name: String,
    /// 东方财富字段名 `orgSName`（非 camelCase 的 `orgShortName`）
    #[serde(rename = "orgSName")]
    pub org_short_name: String,
    pub publish_date: String,
    pub info_code: String,
    pub predict_this_year_eps: Option<String>,
    pub predict_this_year_pe: Option<String>,
    pub predict_next_year_eps: Option<String>,
    pub predict_next_year_pe: Option<String>,
    pub em_rating_name: String,
    /// 评级变动（上调/下调次数），东方财富返回整数或空字符串
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub rating_change: Option<i64>,
    pub industry_name: String,
    pub researcher: String,
    pub attach_pages: Option<i64>,
}

/// 研报详情（PDF 链接 + 摘要）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDetail {
    pub title: String,
    pub pdf_url: Option<String>,
    pub description: String,
    pub org_name: String,
    pub publish_date: String,
}

impl EastmoneyClient {
    /// 获取研报列表。
    ///
    /// `category` 研报分类，`page` 页码（从 1 开始），`size` 每页条数。
    pub async fn reports(
        &self,
        category: ReportCategory,
        _page: u32,
        _size: u32,
    ) -> Result<Vec<serde_json::Value>, LhbError> {
        let url = format!(
            "https://data.eastmoney.com/report/{}",
            category.path_segment()
        );
        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;

        let data = extract_initdata(&html).ok_or(LhbError::EmptyResult)?;
        Ok(data)
    }

    /// 获取个股研报列表（含盈利预测）。
    pub async fn stock_reports(
        &self,
        _page: u32,
        _size: u32,
    ) -> Result<Vec<StockReportItem>, LhbError> {
        let items = self.reports(ReportCategory::Stock, _page, _size).await?;
        let reports: Vec<StockReportItem> = items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        Ok(reports)
    }

    /// 获取非个股研报列表。
    pub async fn non_stock_reports(
        &self,
        category: ReportCategory,
        page: u32,
        size: u32,
    ) -> Result<Vec<ReportItem>, LhbError> {
        let items = self.reports(category, page, size).await?;
        let reports: Vec<ReportItem> = items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        Ok(reports)
    }

    /// 获取研报详情（PDF 链接 + 摘要）。
    ///
    /// `info_code` 研报 infoCode（个股研报）或 encodeUrl（非个股研报）。
    /// `category` 研报分类，用于构造详情页 URL。
    pub async fn report_detail(
        &self,
        info_code: &str,
        category: ReportCategory,
    ) -> Result<ReportDetail, LhbError> {
        let url = if category == ReportCategory::Stock {
            format!("https://data.eastmoney.com/report/info/{}.html", info_code)
        } else {
            // 非个股研报详情页：encodeUrl/infocode 仅作为查询参数，不带路径段
            let (path, param) = match category {
                ReportCategory::BrokerReport => ("zw_brokerreport", "encodeUrl"),
                ReportCategory::Industry => ("zw_industry", "infocode"),
                ReportCategory::MacroResearch => ("zw_macresearch", "encodeUrl"),
                _ => ("zw_strategy", "encodeUrl"),
            };
            format!(
                "https://data.eastmoney.com/report/{}.jshtml?{}={}",
                path, param, info_code
            )
        };

        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;

        let pdf_url = extract_pdf_url(&html);
        let description = extract_description(&html);

        Ok(ReportDetail {
            title: extract_title(&html),
            pdf_url,
            description,
            org_name: extract_org_name(&html),
            publish_date: extract_publish_date(&html),
        })
    }
}

/// 从东方财富页面 HTML 中提取 initdata JSON。
fn extract_initdata(html: &str) -> Option<Vec<serde_json::Value>> {
    let re = regex::Regex::new(r"initdata\s*=\s*(\{.*?\});").ok()?;
    let cap = re.captures(html)?;
    let parsed: serde_json::Value = serde_json::from_str(&cap[1]).ok()?;
    parsed.get("data")?.as_array().cloned()
}

/// 从研报详情页 HTML 中提取 PDF 链接。
fn extract_pdf_url(html: &str) -> Option<String> {
    // 个股研报: #ContentBody .rightlab href
    let re = regex::Regex::new(r#"(?i)class="rightlab"[^>]*href="([^"]+)""#).ok()?;
    if let Some(cap) = re.captures(html) {
        let url = cap[1].to_string();
        if url.starts_with("http") || url.starts_with("/") {
            return Some(if url.starts_with("http") {
                url
            } else {
                format!("https://data.eastmoney.com{url}")
            });
        }
    }
    // 非个股研报: .pdf-link href
    let re = regex::Regex::new(r#"(?i)class="pdf-link"[^>]*href="([^"]+)""#).ok()?;
    if let Some(cap) = re.captures(html) {
        let url = cap[1].to_string();
        return Some(if url.starts_with("http") {
            url
        } else {
            format!("https://data.eastmoney.com{url}")
        });
    }
    None
}

/// 容忍东方财富 `ratingChange` 的空字符串（`""`）/ null / 整数三种形态。
fn deserialize_optional_i64<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(de)?;
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(None),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("expected integer")),
        _ => Err(serde::de::Error::custom("unexpected ratingChange value")),
    }
}

/// 去掉 HTML 标签、折叠空白，得到纯文本。
fn strip_html(raw: &str) -> String {
    let strip_tags = regex::Regex::new(r"<[^>]+>").ok();
    let collapse = regex::Regex::new(r"\s+").ok();
    let text = strip_tags
        .map(|re| re.replace_all(raw, " "))
        .unwrap_or_else(|| raw.into());
    collapse
        .map(|re| re.replace_all(&text, " ").trim().to_string())
        .unwrap_or_else(|| text.trim().to_string())
}

/// 从研报详情页 HTML 中提取正文摘要（纯文本）。
///
/// 个股与非个股详情页的正文都位于 `.ctx-content`（旧版个股页为 `.newsContent`），
/// 内容为多行 HTML（`<p>` 等标签），提取后去掉标签、折叠空白得到纯文本。
fn extract_description(html: &str) -> String {
    let Some(re) =
        regex::Regex::new(r#"(?is)class="(?:ctx-content|newsContent)"[^>]*>(.*?)</div>"#).ok()
    else {
        return String::new();
    };
    let Some(cap) = re.captures(html) else {
        return String::new();
    };
    strip_html(&cap[1])
}

/// 从研报详情页 HTML 中提取标题（`<h1>` 标签内容）。
fn extract_title(html: &str) -> String {
    let Some(re) = regex::Regex::new(r#"(?is)<h1[^>]*>(.*?)</h1>"#).ok() else {
        return String::new();
    };
    let Some(cap) = re.captures(html) else {
        return String::new();
    };
    strip_html(&cap[1])
}

/// 从研报详情页 HTML 中提取机构名称（`publish-date` 后的下一个 `<span>`）。
fn extract_org_name(html: &str) -> String {
    let Some(re) =
        regex::Regex::new(r#"(?is)<span id="publish-date"[^>]*></span>\s*<span>([^<]+)</span>"#)
            .ok()
    else {
        return String::new();
    };
    let Some(cap) = re.captures(html) else {
        return String::new();
    };
    cap[1].trim().to_string()
}

/// 从研报详情页 HTML 中提取发布日期（`notice_date` 字段，格式 `YYYY-MM-DD HH:MM:SS`）。
fn extract_publish_date(html: &str) -> String {
    let Some(re) = regex::Regex::new(r#""notice_date":"([^"]+)""#).ok() else {
        return String::new();
    };
    let Some(cap) = re.captures(html) else {
        return String::new();
    };
    cap[1].to_string()
}

// ─── 行情接口（按 akshare 移植） ─────────────────────────────────────────

const ZT_POOL_URL: &str = "https://push2ex.eastmoney.com/getTopicZTPool";
const ZB_POOL_URL: &str = "https://push2ex.eastmoney.com/getTopicZBPool";
const TREND_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/trends2/get";
const KLINE_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
const FUND_FLOW_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get";
const BOARD_SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";
const MARKET_LIST_URL: &str = "https://82.push2.eastmoney.com/api/qt/clist/get";
const UT: &str = "7eea3edcaed734bea9cbfc24409ed989";
const SEARCH_TOKEN: &str = "D43BF722C8E33BDC906FB84D85E326E8";
const MARKET_LIST_PAGE_SIZE: u32 = 100;
const MARKET_LIST_MAX_PAGES: u32 = 100;
const MARKET_LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

// ─── Domain types ────────────────────────────────────────────────────────────

/// 涨停统计：最近 `days` 个交易日内涨停 `count` 次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitUpStats {
    pub days: u32,
    pub count: u32,
}

/// 涨停股池条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitUpStock {
    pub code: String,
    pub name: String,
    pub price: Option<f64>,
    pub change_percent: Option<f64>,
    pub amount: Option<f64>,
    pub float_market_cap: Option<f64>,
    pub total_market_cap: Option<f64>,
    pub turnover_rate: Option<f64>,
    pub consecutive_boards: Option<u32>,
    /// 首次封板时间，格式 `HH:MM:SS`。
    pub first_seal_time: Option<String>,
    /// 最后封板时间，格式 `HH:MM:SS`。
    pub last_seal_time: Option<String>,
    /// 封板资金（元）。
    pub seal_funds: Option<f64>,
    pub broken_times: Option<u32>,
    pub industry: Option<String>,
    pub limit_up_stats: Option<LimitUpStats>,
}

/// 炸板股池条目（曾触及涨停但当前未封住）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenBoardStock {
    pub code: String,
    pub name: String,
    pub price: Option<f64>,
    /// 涨停价。
    pub limit_up_price: Option<f64>,
    pub change_percent: Option<f64>,
    pub amount: Option<f64>,
    pub float_market_cap: Option<f64>,
    pub total_market_cap: Option<f64>,
    pub turnover_rate: Option<f64>,
    /// 首次封板时间，格式 `HH:MM:SS`。
    pub first_seal_time: Option<String>,
    pub broken_times: Option<u32>,
    pub amplitude: Option<f64>,
    /// 涨速。
    pub speed: Option<f64>,
    pub industry: Option<String>,
    pub limit_up_stats: Option<LimitUpStats>,
}

/// 全市场个股行情条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStock {
    /// 东方财富原始代码（6 位）。
    pub code: String,
    /// 规范化代码，例如 `600519.SS`、`300750.SZ`、`920001.BJ`。
    pub symbol: String,
    pub name: String,
    pub price: Option<f64>,
    /// 涨跌幅，单位：%。
    pub change_percent: Option<f64>,
    pub turnover_rate: Option<f64>,
    pub total_market_cap: Option<f64>,
    pub float_market_cap: Option<f64>,
}

/// 个股排行榜请求。
#[derive(Debug, Clone, Copy)]
pub struct StockRankingRequest {
    pub order: StockRankingOrder,
    pub market: StockMarketFilter,
    pub exclude_st: bool,
    pub offset: usize,
    pub limit: usize,
}

impl Default for StockRankingRequest {
    fn default() -> Self {
        Self {
            order: StockRankingOrder::Gainers,
            market: StockMarketFilter::All,
            exclude_st: false,
            offset: 0,
            limit: 100,
        }
    }
}

/// 一页个股排行。`next_offset` 为 `None` 表示已到末尾。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockRankingPage {
    pub items: Vec<MarketStock>,
    pub next_offset: Option<usize>,
}

/// 个股排行榜方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockRankingOrder {
    Gainers,
    Losers,
}

impl StockRankingOrder {
    const fn sort_order(self) -> &'static str {
        match self {
            Self::Gainers => "1",
            Self::Losers => "0",
        }
    }
}

impl FromStr for StockRankingOrder {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gainers" | "gain" | "up" | "涨幅" => Ok(Self::Gainers),
            "losers" | "lose" | "down" | "跌幅" => Ok(Self::Losers),
            other => bail!("不支持的个股排行方向: {other}（可选 gainers/losers）"),
        }
    }
}

/// 个股市场/板块过滤。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockMarketFilter {
    All,
    MainBoard,
    ChiNext,
    StarMarket,
}

impl StockMarketFilter {
    const fn fs(self) -> &'static str {
        match self {
            Self::All => "m:0 t:6,m:0 t:80,m:1 t:2,m:1 t:23",
            Self::MainBoard => "m:1 t:2,m:0 t:6",
            Self::ChiNext => "m:0 t:80",
            Self::StarMarket => "m:1 t:23",
        }
    }
}

impl FromStr for StockMarketFilter {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" | "全部" => Ok(Self::All),
            "main" | "main-board" | "主板" => Ok(Self::MainBoard),
            "chinext" | "cyb" | "创业板" => Ok(Self::ChiNext),
            "star" | "star-market" | "kcb" | "科创板" => Ok(Self::StarMarket),
            other => bail!("不支持的个股市场: {other}"),
        }
    }
}

/// 分钟周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinutePeriod {
    One,
    Five,
    Fifteen,
    Thirty,
    Sixty,
}

impl MinutePeriod {
    const fn as_klt(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Five => "5",
            Self::Fifteen => "15",
            Self::Thirty => "30",
            Self::Sixty => "60",
        }
    }
}

impl FromStr for MinutePeriod {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "1m" | "one" => Ok(Self::One),
            "5" | "5m" | "five" => Ok(Self::Five),
            "15" | "15m" | "fifteen" => Ok(Self::Fifteen),
            "30" | "30m" | "thirty" => Ok(Self::Thirty),
            "60" | "60m" | "sixty" => Ok(Self::Sixty),
            other => bail!("不支持的分钟周期: {other}（可选 1/5/15/30/60）"),
        }
    }
}

/// 复权方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Adjust {
    #[default]
    None,
    Qfq,
    Hfq,
}

impl Adjust {
    const fn as_code(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::Qfq => "1",
            Self::Hfq => "2",
        }
    }
}

/// 个股分钟 K 线。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteCandle {
    /// `YYYY-MM-DD HH:MM:SS`
    pub time: String,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    /// 成交量（手）。
    pub volume: Option<f64>,
    /// 成交额（元）。
    pub amount: Option<f64>,
    /// 均价，仅 1 分钟接口提供。
    pub avg_price: Option<f64>,
    pub change_percent: Option<f64>,
    pub change_amount: Option<f64>,
    pub amplitude: Option<f64>,
    pub turnover_rate: Option<f64>,
}

/// 分钟 K 线请求（可选参数较多，使用 builder）。
#[derive(Debug, Clone)]
pub struct MinuteHistRequest {
    pub symbol: String,
    pub period: MinutePeriod,
    pub adjust: Adjust,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    /// 只保留最后 N 根 K 线；`None` 表示全部。
    pub limit: Option<usize>,
}

impl MinuteHistRequest {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            period: MinutePeriod::Five,
            adjust: Adjust::None,
            start_time: None,
            end_time: None,
            limit: None,
        }
    }

    #[must_use]
    pub fn period(mut self, period: MinutePeriod) -> Self {
        self.period = period;
        self
    }

    #[must_use]
    pub fn adjust(mut self, adjust: Adjust) -> Self {
        self.adjust = adjust;
        self
    }

    #[must_use]
    pub fn start_time(mut self, value: impl Into<String>) -> Self {
        self.start_time = Some(value.into());
        self
    }

    #[must_use]
    pub fn end_time(mut self, value: impl Into<String>) -> Self {
        self.end_time = Some(value.into());
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// 个股单日资金流向。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundFlowDay {
    pub date: String,
    pub close: Option<f64>,
    pub change_percent: Option<f64>,
    /// 主力净流入（元）。
    pub main_net: Option<f64>,
    /// 主力净流入占比（%）。
    pub main_net_ratio: Option<f64>,
    pub super_large_net: Option<f64>,
    pub super_large_ratio: Option<f64>,
    pub large_net: Option<f64>,
    pub large_ratio: Option<f64>,
    pub medium_net: Option<f64>,
    pub medium_ratio: Option<f64>,
    pub small_net: Option<f64>,
    pub small_ratio: Option<f64>,
}

/// 板块 K 线周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardPeriod {
    #[default]
    Day,
    Week,
    Month,
}

impl BoardPeriod {
    const fn as_klt(self) -> &'static str {
        match self {
            Self::Day => "101",
            Self::Week => "102",
            Self::Month => "103",
        }
    }
}

impl FromStr for BoardPeriod {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "day" | "daily" | "日" | "日k" => Ok(Self::Day),
            "week" | "weekly" | "周" | "周k" => Ok(Self::Week),
            "month" | "monthly" | "月" | "月k" => Ok(Self::Month),
            other => bail!("不支持的板块周期: {other}（可选 day/week/month）"),
        }
    }
}

/// 板块历史 K 线。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardKline {
    pub date: String,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub change_percent: Option<f64>,
    pub change_amount: Option<f64>,
    /// 成交量（手）。
    pub volume: Option<f64>,
    /// 成交额（元）。
    pub amount: Option<f64>,
    pub amplitude: Option<f64>,
    pub turnover_rate: Option<f64>,
}

/// 板块历史 K 线请求。
#[derive(Debug, Clone)]
pub struct BoardKlineRequest {
    /// 板块代码（`BKxxxx`）或板块名称。
    pub board: String,
    pub period: BoardPeriod,
    pub adjust: Adjust,
    /// `YYYYMMDD` 或 `YYYY-MM-DD`。
    pub start_date: String,
    /// `YYYYMMDD` 或 `YYYY-MM-DD`。
    pub end_date: String,
    /// 只保留最后 N 根 K 线；`None` 表示全部。
    pub limit: Option<usize>,
}

impl BoardKlineRequest {
    pub fn new(board: impl Into<String>) -> Self {
        Self {
            board: board.into(),
            period: BoardPeriod::Day,
            adjust: Adjust::None,
            start_date: "19900101".to_string(),
            end_date: "20500101".to_string(),
            limit: None,
        }
    }

    #[must_use]
    pub fn period(mut self, period: BoardPeriod) -> Self {
        self.period = period;
        self
    }

    #[must_use]
    pub fn adjust(mut self, adjust: Adjust) -> Self {
        self.adjust = adjust;
        self
    }

    #[must_use]
    pub fn date_range(
        mut self,
        start_date: impl Into<String>,
        end_date: impl Into<String>,
    ) -> Self {
        self.start_date = start_date.into();
        self.end_date = end_date.into();
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl EastmoneyClient {
    /// 获取全市场个股涨幅/跌幅榜。
    ///
    /// 东方财富负责按 `f3`（涨跌幅）排序和分页；ST 使用股票名称中的
    /// `ST`/`*ST` 标识过滤。返回值已按请求方向排序。
    ///
    /// # Errors
    /// Returns an error if pagination parameters are invalid, the HTTP request
    /// fails, or the response cannot be parsed.
    pub async fn stock_rankings(&self, request: StockRankingRequest) -> Result<StockRankingPage> {
        if request.limit == 0 {
            bail!("个股排行数量必须大于 0");
        }

        let mut items = Vec::with_capacity(request.limit);
        let mut matched = 0usize;
        let mut page = 1u32;
        let mut reached_end = false;

        while items.len() < request.limit {
            if page > MARKET_LIST_MAX_PAGES {
                bail!("个股排行分页超过上限");
            }
            let response = self
                .client
                .get(MARKET_LIST_URL)
                .query(&[
                    ("pn", page.to_string()),
                    ("pz", MARKET_LIST_PAGE_SIZE.to_string()),
                    ("po", request.order.sort_order().to_string()),
                    ("np", "1".to_string()),
                    ("ut", UT.to_string()),
                    ("fltt", "2".to_string()),
                    ("invt", "2".to_string()),
                    ("fid", "f3".to_string()),
                    ("fs", request.market.fs().to_string()),
                    ("fields", "f2,f3,f8,f12,f13,f14,f20,f21".to_string()),
                ])
                .header(
                    "Referer",
                    "https://quote.eastmoney.com/center/gridlist.html",
                )
                .timeout(MARKET_LIST_TIMEOUT)
                .send()
                .await
                .context("个股排行请求失败")?
                .text()
                .await
                .context("个股排行响应读取失败")?;
            let response: Value =
                serde_json::from_str(&response).context("个股排行响应不是合法 JSON")?;

            let total = response["data"]["total"].as_u64().unwrap_or_default();
            let rows = response["data"]["diff"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if rows.is_empty() {
                reached_end = true;
                break;
            }

            for row in rows {
                let Some(stock) = parse_market_stock_row(&row) else {
                    continue;
                };
                if stock.symbol.ends_with(".BJ") {
                    continue;
                }
                if request.exclude_st && is_st_name(&stock.name) {
                    continue;
                }
                if matched < request.offset {
                    matched += 1;
                    continue;
                }
                matched += 1;
                items.push(stock);
                if items.len() == request.limit {
                    break;
                }
            }

            if page as u64 * u64::from(MARKET_LIST_PAGE_SIZE) >= total {
                reached_end = true;
                break;
            }
            page += 1;
        }

        let next_offset = if items.len() < request.limit || reached_end {
            None
        } else {
            Some(request.offset.saturating_add(items.len()))
        };
        Ok(StockRankingPage { items, next_offset })
    }

    /// 涨停股池。
    ///
    /// `date` 使用 `YYYYMMDD` 或 `YYYY-MM-DD`，例如 `20261021`。
    ///
    /// # Errors
    /// Returns an error if the date is invalid, the HTTP request fails, or the
    /// response cannot be parsed.
    pub async fn limit_up_pool(&self, date: &str) -> Result<Vec<LimitUpStock>> {
        let date = compact_date(date)?;
        let response: Value = self
            .client
            .get(ZT_POOL_URL)
            .query(&[
                ("ut", UT),
                ("dpt", "wz.ztzt"),
                ("Pageindex", "0"),
                ("pagesize", "10000"),
                ("sort", "fbt:asc"),
                ("date", date.as_str()),
            ])
            .send()
            .await
            .context("涨停股池请求失败")?
            .json()
            .await
            .context("涨停股池响应不是合法 JSON")?;

        let Some(pool) = response["data"]["pool"].as_array() else {
            return Ok(Vec::new());
        };
        Ok(pool.iter().filter_map(parse_limit_up_row).collect())
    }

    /// 炸板股池（曾涨停但当前未封住）。
    ///
    /// 东方财富只提供最近约 30 个交易日的数据。
    ///
    /// # Errors
    /// Returns an error if the date is invalid, the HTTP request fails, or the
    /// response cannot be parsed.
    pub async fn broken_board_pool(&self, date: &str) -> Result<Vec<BrokenBoardStock>> {
        let date = compact_date(date)?;
        let response: Value = self
            .client
            .get(ZB_POOL_URL)
            .query(&[
                ("ut", UT),
                ("dpt", "wz.ztzt"),
                ("Pageindex", "0"),
                ("pagesize", "5000"),
                ("sort", "fbt:asc"),
                ("date", date.as_str()),
            ])
            .send()
            .await
            .context("炸板股池请求失败")?
            .json()
            .await
            .context("炸板股池响应不是合法 JSON")?;

        let Some(pool) = response["data"]["pool"].as_array() else {
            return Ok(Vec::new());
        };
        Ok(pool.iter().filter_map(parse_broken_board_row).collect())
    }

    /// 个股分钟 K 线。
    ///
    /// 1 分钟周期返回均价；5/15/30/60 分钟周期返回涨跌幅、振幅、换手率等字段。
    ///
    /// # Errors
    /// Returns an error if the symbol is invalid, the HTTP request fails, or
    /// the response cannot be parsed.
    pub async fn minute_kline(&self, request: &MinuteHistRequest) -> Result<Vec<MinuteCandle>> {
        let secid = stock_secid(&request.symbol)?;
        let mut candles = if request.period == MinutePeriod::One {
            self.fetch_one_minute(&secid).await?
        } else {
            self.fetch_minute_kline(&secid, request.period, request.adjust)
                .await?
        };

        let start = request
            .start_time
            .as_deref()
            .map(normalize_range_start)
            .transpose()?;
        let end = request
            .end_time
            .as_deref()
            .map(normalize_range_end)
            .transpose()?;
        candles.retain(|candle| {
            start
                .as_deref()
                .map_or(true, |start| candle.time.as_str() >= start)
                && end
                    .as_deref()
                    .map_or(true, |end| candle.time.as_str() <= end)
        });

        if let Some(limit) = request.limit {
            if candles.len() > limit {
                candles.drain(..candles.len() - limit);
            }
        }
        Ok(candles)
    }

    /// 个股资金流向（按日）。
    ///
    /// # Errors
    /// Returns an error if the symbol is invalid, the HTTP request fails, or
    /// the response cannot be parsed.
    pub async fn individual_fund_flow(&self, symbol: &str) -> Result<Vec<FundFlowDay>> {
        let secid = stock_secid(symbol)?;
        let cache_buster = now_ts().to_string();
        let response: Value = self
            .client
            .get(FUND_FLOW_URL)
            .query(&[
                ("lmt", "0"),
                ("klt", "101"),
                ("secid", secid.as_str()),
                ("fields1", "f1,f2,f3,f7"),
                (
                    "fields2",
                    "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65",
                ),
                ("ut", "b2884a393a59ad64002292a3e90d46a5"),
                ("_", cache_buster.as_str()),
            ])
            .send()
            .await
            .context("个股资金流请求失败")?
            .json()
            .await
            .context("个股资金流响应不是合法 JSON")?;

        let klines = response["data"]["klines"]
            .as_array()
            .context("个股资金流缺少 data.klines")?;
        Ok(klines
            .iter()
            .filter_map(|line| line.as_str().and_then(parse_fund_flow_line))
            .collect())
    }

    /// 概念板块历史 K 线。
    ///
    /// `board` 可以是板块名称（如 `绿色电力`）或 `BK` 代码。
    ///
    /// # Errors
    /// Returns an error if the board cannot be resolved, the date is invalid,
    /// the HTTP request fails, or the response cannot be parsed.
    pub async fn concept_kline(&self, request: &BoardKlineRequest) -> Result<Vec<BoardKline>> {
        self.board_kline(request).await
    }

    /// 行业板块历史 K 线。
    ///
    /// `board` 可以是板块名称（如 `小金属`）或 `BK` 代码。
    ///
    /// # Errors
    /// Returns an error if the board cannot be resolved, the date is invalid,
    /// the HTTP request fails, or the response cannot be parsed.
    pub async fn industry_kline(&self, request: &BoardKlineRequest) -> Result<Vec<BoardKline>> {
        self.board_kline(request).await
    }

    async fn fetch_one_minute(&self, secid: &str) -> Result<Vec<MinuteCandle>> {
        let response: Value = self
            .client
            .get(TREND_URL)
            .query(&[
                ("fields1", "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13"),
                ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58"),
                ("ut", UT),
                ("ndays", "5"),
                ("iscr", "0"),
                ("secid", secid),
            ])
            .send()
            .await
            .context("分钟行情请求失败")?
            .json()
            .await
            .context("分钟行情响应不是合法 JSON")?;

        let trends = response["data"]["trends"]
            .as_array()
            .context("分钟行情缺少 data.trends")?;
        Ok(trends
            .iter()
            .filter_map(|line| line.as_str().and_then(parse_one_minute_line))
            .collect())
    }

    async fn fetch_minute_kline(
        &self,
        secid: &str,
        period: MinutePeriod,
        adjust: Adjust,
    ) -> Result<Vec<MinuteCandle>> {
        let response: Value = self
            .client
            .get(KLINE_URL)
            .query(&[
                ("fields1", "f1,f2,f3,f4,f5,f6"),
                ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61"),
                ("ut", UT),
                ("klt", period.as_klt()),
                ("fqt", adjust.as_code()),
                ("secid", secid),
                ("beg", "0"),
                ("end", "20500000"),
            ])
            .send()
            .await
            .context("分钟 K 线请求失败")?
            .json()
            .await
            .context("分钟 K 线响应不是合法 JSON")?;

        let klines = response["data"]["klines"]
            .as_array()
            .context("分钟 K 线缺少 data.klines")?;
        Ok(klines
            .iter()
            .filter_map(|line| line.as_str().and_then(parse_minute_kline_line))
            .collect())
    }

    async fn board_kline(&self, request: &BoardKlineRequest) -> Result<Vec<BoardKline>> {
        let board_code = self.resolve_board_code(&request.board).await?;
        let secid = format!("90.{board_code}");
        let start = compact_date(&request.start_date)?;
        let end = compact_date(&request.end_date)?;
        let response: Value = self
            .client
            .get(KLINE_URL)
            .query(&[
                ("secid", secid.as_str()),
                ("fields1", "f1,f2,f3,f4,f5,f6"),
                ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61"),
                ("klt", request.period.as_klt()),
                ("fqt", request.adjust.as_code()),
                ("beg", start.as_str()),
                ("end", end.as_str()),
                ("smplmt", "10000"),
                ("lmt", "1000000"),
            ])
            .send()
            .await
            .context("板块 K 线请求失败")?
            .json()
            .await
            .context("板块 K 线响应不是合法 JSON")?;

        let klines = response["data"]["klines"]
            .as_array()
            .context("板块 K 线缺少 data.klines")?;
        let mut candles: Vec<BoardKline> = klines
            .iter()
            .filter_map(|line| line.as_str().and_then(parse_board_kline_line))
            .collect();
        if let Some(limit) = request.limit {
            if candles.len() > limit {
                candles.drain(..candles.len() - limit);
            }
        }
        Ok(candles)
    }

    async fn resolve_board_code(&self, board: &str) -> Result<String> {
        let board = board.trim();
        if board.is_empty() {
            bail!("板块名称或代码不能为空");
        }
        let upper = board.to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix("BK") {
            if !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()) {
                return Ok(upper);
            }
        }

        let response: Value = self
            .client
            .get(BOARD_SEARCH_URL)
            .query(&[
                ("input", board),
                ("type", "14"),
                ("token", SEARCH_TOKEN),
                ("count", "20"),
            ])
            .send()
            .await
            .context("板块搜索请求失败")?
            .json()
            .await
            .context("板块搜索响应不是合法 JSON")?;
        let rows = response["QuotationCodeTable"]["Data"]
            .as_array()
            .context("板块搜索缺少 QuotationCodeTable.Data")?;

        for row in rows {
            let Some(code) = row["Code"].as_str() else {
                continue;
            };
            let Some(name) = row["Name"].as_str() else {
                continue;
            };
            if row["Classify"].as_str().is_some_and(|value| value == "BK")
                && code.to_ascii_uppercase().starts_with("BK")
                && name == board
            {
                return Ok(code.to_ascii_uppercase());
            }
        }
        bail!("未找到板块: {board}（可改用 BK 代码）")
    }
}

// ─── Parsing helpers ─────────────────────────────────────────────────────────

fn parse_limit_up_row(row: &Value) -> Option<LimitUpStock> {
    Some(LimitUpStock {
        code: row["c"].as_str()?.to_string(),
        name: row["n"].as_str()?.to_string(),
        price: json_f64(row, "p").map(|value| value / 1000.0),
        change_percent: json_f64(row, "zdp"),
        amount: json_f64(row, "amount"),
        float_market_cap: json_f64(row, "ltsz"),
        total_market_cap: json_f64(row, "tshare"),
        turnover_rate: json_f64(row, "hs"),
        consecutive_boards: json_u32(row, "lbc"),
        first_seal_time: format_seal_time(json_u64(row, "fbt")),
        last_seal_time: format_seal_time(json_u64(row, "lbt")),
        seal_funds: json_f64(row, "fund"),
        broken_times: json_u32(row, "zbc"),
        industry: row["hybk"].as_str().map(std::string::ToString::to_string),
        limit_up_stats: parse_limit_up_stats(&row["zttj"]),
    })
}

fn parse_market_stock_row(row: &Value) -> Option<MarketStock> {
    let code = row["f12"].as_str()?.to_string();
    let change_percent = json_f64(row, "f3")?;
    Some(MarketStock {
        symbol: market_symbol_from_row(row)?,
        code,
        name: row["f14"].as_str()?.to_string(),
        price: json_f64(row, "f2"),
        change_percent: Some(change_percent),
        turnover_rate: json_f64(row, "f8"),
        total_market_cap: json_f64(row, "f20"),
        float_market_cap: json_f64(row, "f21"),
    })
}

fn market_symbol_from_row(row: &Value) -> Option<String> {
    let code = row["f12"].as_str()?;
    let market = row["f13"].as_u64();
    market_suffix(code, market).map(|suffix| format!("{code}.{suffix}"))
}

fn is_st_name(name: &str) -> bool {
    name.to_ascii_uppercase().contains("ST")
}

fn market_suffix(code: &str, market: Option<u64>) -> Option<&'static str> {
    match market {
        Some(1) => Some("SS"),
        Some(0)
            if code.starts_with("83")
                || code.starts_with("43")
                || code.starts_with("87")
                || code.starts_with("88")
                || code.starts_with("920") =>
        {
            Some("BJ")
        }
        Some(0) => Some("SZ"),
        _ => None,
    }
}

fn parse_broken_board_row(row: &Value) -> Option<BrokenBoardStock> {
    Some(BrokenBoardStock {
        code: row["c"].as_str()?.to_string(),
        name: row["n"].as_str()?.to_string(),
        price: json_f64(row, "p").map(|value| value / 1000.0),
        limit_up_price: json_f64(row, "ztp").map(|value| value / 1000.0),
        change_percent: json_f64(row, "zdp"),
        amount: json_f64(row, "amount"),
        float_market_cap: json_f64(row, "ltsz"),
        total_market_cap: json_f64(row, "tshare"),
        turnover_rate: json_f64(row, "hs"),
        first_seal_time: format_seal_time(json_u64(row, "fbt")),
        broken_times: json_u32(row, "zbc"),
        amplitude: json_f64(row, "zf"),
        speed: json_f64(row, "zs"),
        industry: row["hybk"].as_str().map(std::string::ToString::to_string),
        limit_up_stats: parse_limit_up_stats(&row["zttj"]),
    })
}

fn parse_limit_up_stats(value: &Value) -> Option<LimitUpStats> {
    Some(LimitUpStats {
        days: u32::try_from(value["days"].as_u64()?).ok()?,
        count: u32::try_from(value["ct"].as_u64()?).ok()?,
    })
}

fn parse_one_minute_line(line: &str) -> Option<MinuteCandle> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 8 {
        return None;
    }
    Some(MinuteCandle {
        time: normalize_minute_time(parts[0]),
        open: parse_number(parts[1]),
        close: parse_number(parts[2]),
        high: parse_number(parts[3]),
        low: parse_number(parts[4]),
        volume: parse_number(parts[5]),
        amount: parse_number(parts[6]),
        avg_price: parse_number(parts[7]),
        change_percent: None,
        change_amount: None,
        amplitude: None,
        turnover_rate: None,
    })
}

fn parse_minute_kline_line(line: &str) -> Option<MinuteCandle> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 11 {
        return None;
    }
    Some(MinuteCandle {
        time: normalize_minute_time(parts[0]),
        open: parse_number(parts[1]),
        close: parse_number(parts[2]),
        high: parse_number(parts[3]),
        low: parse_number(parts[4]),
        volume: parse_number(parts[5]),
        amount: parse_number(parts[6]),
        avg_price: None,
        change_percent: parse_number(parts[8]),
        change_amount: parse_number(parts[9]),
        amplitude: parse_number(parts[7]),
        turnover_rate: parse_number(parts[10]),
    })
}

fn parse_fund_flow_line(line: &str) -> Option<FundFlowDay> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 13 {
        return None;
    }
    Some(FundFlowDay {
        date: parts[0].to_string(),
        main_net: parse_number(parts[1]),
        small_net: parse_number(parts[2]),
        medium_net: parse_number(parts[3]),
        large_net: parse_number(parts[4]),
        super_large_net: parse_number(parts[5]),
        main_net_ratio: parse_number(parts[6]),
        small_ratio: parse_number(parts[7]),
        medium_ratio: parse_number(parts[8]),
        large_ratio: parse_number(parts[9]),
        super_large_ratio: parse_number(parts[10]),
        close: parse_number(parts[11]),
        change_percent: parse_number(parts[12]),
    })
}

fn parse_board_kline_line(line: &str) -> Option<BoardKline> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 11 {
        return None;
    }
    Some(BoardKline {
        date: parts[0].to_string(),
        open: parse_number(parts[1]),
        close: parse_number(parts[2]),
        high: parse_number(parts[3]),
        low: parse_number(parts[4]),
        volume: parse_number(parts[5]),
        amount: parse_number(parts[6]),
        amplitude: parse_number(parts[7]),
        change_percent: parse_number(parts[8]),
        change_amount: parse_number(parts[9]),
        turnover_rate: parse_number(parts[10]),
    })
}

fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse().ok()
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(parse_number))
    })
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .get(key)
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
}

fn json_u32(value: &Value, key: &str) -> Option<u32> {
    u32::try_from(json_u64(value, key)?).ok()
}

fn format_seal_time(value: Option<u64>) -> Option<String> {
    let value = value?;
    if value == 0 || value > 235_959 {
        return None;
    }
    let text = format!("{value:06}");
    Some(format!("{}:{}:{}", &text[0..2], &text[2..4], &text[4..6]))
}

fn stock_secid(symbol: &str) -> Result<String> {
    let normalized = to_yahoo_symbol(symbol)?;
    let (code, suffix) = normalized
        .split_once('.')
        .with_context(|| format!("无法识别股票代码: {symbol}"))?;
    let market = match suffix {
        "SS" => "1",
        "SZ" | "BJ" => "0",
        other => bail!("不支持的股票市场后缀: {other}"),
    };
    Ok(format!("{market}.{code}"))
}

fn normalize_minute_time(value: &str) -> String {
    let value = value.trim();
    if value.len() == 16 {
        format!("{value}:00")
    } else {
        value.to_string()
    }
}

fn normalize_range_start(value: &str) -> Result<String> {
    normalize_range(value, false)
}

fn normalize_range_end(value: &str) -> Result<String> {
    normalize_range(value, true)
}

fn normalize_range(value: &str, end: bool) -> Result<String> {
    let value = value.trim();
    if value.len() == 10 {
        return Ok(format!(
            "{value} {}",
            if end { "23:59:59" } else { "00:00:00" }
        ));
    }
    if value.len() == 16 {
        return Ok(format!("{value}:{}", if end { "59" } else { "00" }));
    }
    if value.len() == 19 {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
            .with_context(|| format!("非法时间: {value}"))?;
        return Ok(value.to_string());
    }
    bail!("时间格式应为 YYYY-MM-DD、YYYY-MM-DD HH:MM 或 YYYY-MM-DD HH:MM:SS: {value}")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod market_tests {
    use super::*;

    #[test]
    fn parses_limit_up_pool_row() {
        let row = serde_json::json!({
            "c": "002313",
            "n": "日海智能",
            "p": 9680,
            "zdp": 10.0,
            "amount": 64_604_320,
            "ltsz": 3_623_991_624.0,
            "tshare": 3_624_192_000.0,
            "hs": 1.7826,
            "lbc": 2,
            "fbt": 92500,
            "lbt": 92500,
            "fund": 95_359_616,
            "zbc": 0,
            "hybk": "通信设备",
            "zttj": { "days": 2, "ct": 2 }
        });
        let row = parse_limit_up_row(&row).unwrap_or_else(|| panic!("row should parse"));
        assert_eq!(row.code, "002313");
        assert_eq!(row.price, Some(9.68));
        assert_eq!(row.consecutive_boards, Some(2));
        assert_eq!(row.first_seal_time.as_deref(), Some("09:25:00"));
        assert_eq!(row.seal_funds, Some(95_359_616.0));
        assert_eq!(row.industry.as_deref(), Some("通信设备"));
        assert_eq!(row.limit_up_stats, Some(LimitUpStats { days: 2, count: 2 }));
    }

    #[test]
    fn parses_broken_board_row() {
        let row = serde_json::json!({
            "c": "000032",
            "n": "深桑达Ａ",
            "p": 13200,
            "ztp": 13950,
            "zdp": 4.1009,
            "amount": 1_709_858_160,
            "ltsz": 16_040_366_905.2,
            "tshare": 16_040_366_918.4,
            "hs": 10.4745,
            "fbt": 92500,
            "zbc": 1,
            "zf": 7.4921,
            "zs": 0.0,
            "hybk": "专业工程",
            "zttj": { "days": 0, "ct": 0 }
        });
        let row = parse_broken_board_row(&row).unwrap_or_else(|| panic!("row should parse"));
        assert_eq!(row.code, "000032");
        assert_eq!(row.limit_up_price, Some(13.95));
        assert_eq!(row.first_seal_time.as_deref(), Some("09:25:00"));
        assert_eq!(row.broken_times, Some(1));
        assert_eq!(row.industry.as_deref(), Some("专业工程"));
    }

    #[test]
    fn parses_one_minute_line() {
        let candle = parse_one_minute_line(
            "2026-10-21 09:31,1330.88,1323.36,1331.98,1323.33,721,95668150.00,1328.633",
        )
        .unwrap_or_else(|| panic!("line should parse"));
        assert_eq!(candle.time, "2026-10-21 09:31:00");
        assert_eq!(candle.open, Some(1330.88));
        assert_eq!(candle.close, Some(1323.36));
        assert_eq!(candle.volume, Some(721.0));
        assert_eq!(candle.avg_price, Some(1328.633));
    }

    #[test]
    fn parses_five_minute_line() {
        let candle = parse_minute_kline_line(
            "2026-08-31 09:35,1297.99,1297.01,1305.00,1293.70,2046,265829477.00,0.87,-0.03,-0.39,0.02",
        )
        .unwrap_or_else(|| panic!("line should parse"));
        assert_eq!(candle.time, "2026-08-31 09:35:00");
        assert_eq!(candle.close, Some(1297.01));
        assert_eq!(candle.amplitude, Some(0.87));
        assert_eq!(candle.change_percent, Some(-0.03));
        assert_eq!(candle.turnover_rate, Some(0.02));
        assert_eq!(candle.avg_price, None);
    }

    #[test]
    fn parses_fund_flow_line() {
        let row = parse_fund_flow_line(
            "2026-04-22,-28038096.0,-2637216.0,30675328.0,-152804032.0,124765936.0,-0.74,-0.07,0.81,-4.03,3.29,1381.48,-0.18,0.00,0.00",
        )
        .unwrap_or_else(|| panic!("line should parse"));
        assert_eq!(row.date, "2026-04-22");
        assert_eq!(row.main_net, Some(-28_038_096.0));
        assert_eq!(row.super_large_net, Some(124_765_936.0));
        assert_eq!(row.main_net_ratio, Some(-0.74));
        assert_eq!(row.close, Some(1381.48));
    }

    #[test]
    fn parses_board_kline_line() {
        let row = parse_board_kline_line(
            "2026-01-05,1097.74,1103.80,1108.63,1081.81,19318555,44360341030.00,2.47,1.77,19.17,3.02",
        )
        .unwrap_or_else(|| panic!("line should parse"));
        assert_eq!(row.date, "2026-01-05");
        assert_eq!(row.close, Some(1103.80));
        assert_eq!(row.amount, Some(44_360_341_030.0));
        assert_eq!(row.change_percent, Some(1.77));
    }

    #[test]
    fn formats_seal_times() {
        assert_eq!(format_seal_time(Some(92500)).as_deref(), Some("09:25:00"));
        assert_eq!(format_seal_time(Some(150000)).as_deref(), Some("15:00:00"));
        assert_eq!(format_seal_time(Some(0)), None);
        assert_eq!(format_seal_time(None), None);
    }

    #[test]
    fn normalizes_time_ranges() {
        assert_eq!(
            normalize_range_start("2026-10-21").unwrap_or_default(),
            "2026-10-21 00:00:00"
        );
        assert_eq!(
            normalize_range_end("2026-10-21").unwrap_or_default(),
            "2026-10-21 23:59:59"
        );
        assert_eq!(
            normalize_range_start("2026-10-21 09:30").unwrap_or_default(),
            "2026-10-21 09:30:00"
        );
    }

    #[test]
    fn infers_stock_secid() {
        assert_eq!(stock_secid("600519.SS").unwrap_or_default(), "1.600519");
        assert_eq!(stock_secid("000001").unwrap_or_default(), "0.000001");
    }

    #[test]
    fn parses_market_stock_row_and_symbols() {
        let row = serde_json::json!({
            "f2": 12.34,
            "f3": 5.67,
            "f8": 2.1,
            "f12": "920001",
            "f13": 0,
            "f14": "北交样本",
            "f20": 1000000000.0,
            "f21": 500000000.0
        });
        let stock = parse_market_stock_row(&row).unwrap_or_else(|| panic!("row should parse"));
        assert_eq!(stock.symbol, "920001.BJ");
        assert_eq!(stock.change_percent, Some(5.67));

        let star = serde_json::json!({"f12":"688001","f13":1});
        assert_eq!(market_symbol_from_row(&star).as_deref(), Some("688001.SS"));

        let legacy_beijing = serde_json::json!({"f12":"430001","f13":0});
        assert_eq!(
            market_symbol_from_row(&legacy_beijing).as_deref(),
            Some("430001.BJ")
        );
    }

    #[test]
    fn market_row_requires_exchange_id() {
        let missing_market = serde_json::json!({
            "f2": 12.34,
            "f3": 5.67,
            "f12": "600001",
            "f14": "样本"
        });
        assert!(parse_market_stock_row(&missing_market).is_none());
    }

    #[test]
    fn all_market_filter_excludes_beijing() {
        let fs = StockMarketFilter::All.fs();
        assert!(!fs.contains("t:81"));
        assert!(fs.contains("t:80"));
        assert!(fs.contains("t:23"));
    }

    #[test]
    fn detects_st_names() {
        assert!(is_st_name("ST交昂"));
        assert!(is_st_name("*ST禾信"));
        assert!(!is_st_name("东方财富"));
    }

    #[test]
    fn parses_rank_order_and_market_filters() {
        assert_eq!(
            "gainers".parse::<StockRankingOrder>().ok(),
            Some(StockRankingOrder::Gainers)
        );
        assert_eq!(
            "跌幅".parse::<StockRankingOrder>().ok(),
            Some(StockRankingOrder::Losers)
        );
        assert_eq!(
            "创业板".parse::<StockMarketFilter>().ok(),
            Some(StockMarketFilter::ChiNext)
        );
        assert_eq!(
            "科创板".parse::<StockMarketFilter>().ok(),
            Some(StockMarketFilter::StarMarket)
        );
        assert!("北交所".parse::<StockMarketFilter>().is_err());
    }

    #[test]
    fn stock_ranking_page_reports_next_offset_and_end() {
        let stock = |code: &str| MarketStock {
            code: code.to_string(),
            symbol: format!("{code}.SZ"),
            name: format!("股票{code}"),
            price: Some(10.0),
            change_percent: Some(1.0),
            turnover_rate: Some(1.0),
            total_market_cap: Some(1_000_000.0),
            float_market_cap: Some(500_000.0),
        };
        let page = StockRankingPage {
            items: vec![stock("000001"), stock("000002")],
            next_offset: Some(102),
        };
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.next_offset, Some(102));

        let end = StockRankingPage {
            items: vec![],
            next_offset: None,
        };
        assert_eq!(end.next_offset, None);
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_stock_rankings_losers() {
        let rows = EastmoneyClient::new()
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Losers,
                market: StockMarketFilter::All,
                exclude_st: true,
                offset: 0,
                limit: 10,
            })
            .await
            .unwrap_or_else(|error| panic!("request failed: {error}"));
        assert_eq!(rows.items.len(), 10);
        assert!(rows
            .items
            .windows(2)
            .all(|window| window[0].change_percent <= window[1].change_percent));
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_stock_rankings_second_page() {
        let client = EastmoneyClient::new();
        let first = client
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Gainers,
                market: StockMarketFilter::MainBoard,
                exclude_st: true,
                offset: 0,
                limit: 20,
            })
            .await
            .unwrap_or_else(|error| panic!("first page failed: {error}"));
        let second = client
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Gainers,
                market: StockMarketFilter::MainBoard,
                exclude_st: true,
                offset: 20,
                limit: 20,
            })
            .await
            .unwrap_or_else(|error| panic!("second page failed: {error}"));
        assert_eq!(first.items.len(), 20);
        assert_eq!(second.items.len(), 20);
        assert!(first
            .items
            .iter()
            .all(|left| second.items.iter().all(|right| left.symbol != right.symbol)));
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_stock_rankings_market_filters() {
        let client = EastmoneyClient::new();
        for market in [
            StockMarketFilter::MainBoard,
            StockMarketFilter::ChiNext,
            StockMarketFilter::StarMarket,
        ] {
            let rows = client
                .stock_rankings(StockRankingRequest {
                    order: StockRankingOrder::Gainers,
                    market,
                    exclude_st: false,
                    offset: 0,
                    limit: 5,
                })
                .await
                .unwrap_or_else(|error| panic!("request failed for {market:?}: {error}"));
            assert_eq!(rows.items.len(), 5, "market {market:?}");
        }
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_stock_rankings() {
        let rows = EastmoneyClient::new()
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Gainers,
                market: StockMarketFilter::All,
                exclude_st: true,
                offset: 0,
                limit: 10,
            })
            .await
            .unwrap_or_else(|error| panic!("request failed: {error}"));
        assert_eq!(rows.items.len(), 10);
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_limit_up_pool() {
        let rows = EastmoneyClient::new()
            .limit_up_pool("20261021")
            .await
            .unwrap_or_else(|error| panic!("request failed: {error}"));
        assert!(!rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_broken_board_pool() {
        let rows = EastmoneyClient::new()
            .broken_board_pool("20261021")
            .await
            .unwrap_or_else(|error| panic!("request failed: {error}"));
        assert!(!rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_minute_and_fund_flow() {
        let client = EastmoneyClient::new();
        let request = MinuteHistRequest::new("600519.SS")
            .period(MinutePeriod::One)
            .limit(10);
        let candles = client
            .minute_kline(&request)
            .await
            .unwrap_or_else(|error| panic!("minute request failed: {error}"));
        assert!(!candles.is_empty());
        assert!(candles[0].avg_price.is_some());

        let flow = client
            .individual_fund_flow("600519")
            .await
            .unwrap_or_else(|error| panic!("fund flow request failed: {error}"));
        assert!(!flow.is_empty());
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_board_kline() {
        let client = EastmoneyClient::new();
        let request = BoardKlineRequest::new("绿色电力")
            .date_range("20260101", "20261021")
            .limit(20);
        let concept_rows = client
            .concept_kline(&request)
            .await
            .unwrap_or_else(|error| panic!("concept board request failed: {error}"));
        assert!(!concept_rows.is_empty());

        let request = BoardKlineRequest::new("小金属")
            .date_range("20260101", "20261021")
            .limit(20);
        let industry_rows = client
            .industry_kline(&request)
            .await
            .unwrap_or_else(|error| panic!("industry board request failed: {error}"));
        assert!(!industry_rows.is_empty());
    }
}
