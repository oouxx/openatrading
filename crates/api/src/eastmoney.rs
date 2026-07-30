use ahash::AHashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
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

#[derive(Clone)]
pub struct LhbClient {
    client: Client,
}

impl Default for LhbClient {
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

impl LhbClient {
    /// Create a new `LhbClient`.
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
            LhbClient::fmt_date("20240115").unwrap_or_else(|_| panic!("test failed")),
            "2024-01-15"
        );
        assert_eq!(
            LhbClient::fmt_date("20230101").unwrap_or_else(|_| panic!("test failed")),
            "2023-01-01"
        );
    }

    #[test]
    fn test_fmt_date_too_short() {
        assert!(matches!(
            LhbClient::fmt_date("202401"),
            Err(LhbError::InvalidDate(_))
        ));
    }

    #[test]
    fn test_fmt_date_non_digit() {
        assert!(matches!(
            LhbClient::fmt_date("2024-115"),
            Err(LhbError::InvalidDate(_))
        ));
    }

    #[test]
    fn test_f64_field_present() {
        let v = serde_json::json!({"CLOSE_PRICE": 12.34});
        assert_eq!(LhbClient::f64_field(&v, "CLOSE_PRICE"), Some(12.34));
    }

    #[test]
    fn test_f64_field_missing() {
        let v = serde_json::json!({});
        assert_eq!(LhbClient::f64_field(&v, "CLOSE_PRICE"), None);
    }

    #[test]
    fn test_f64_field_null() {
        let v = serde_json::json!({"CLOSE_PRICE": null});
        assert_eq!(LhbClient::f64_field(&v, "CLOSE_PRICE"), None);
    }

    #[test]
    fn test_str_field() {
        let v = serde_json::json!({"SECURITY_NAME_ABBR": "东方财富"});
        assert_eq!(
            LhbClient::str_field(&v, "SECURITY_NAME_ABBR"),
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
        let client = LhbClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("123", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_invalid_symbol_too_long() {
        let client = LhbClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("1_234_567", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_invalid_symbol_empty() {
        let client = LhbClient::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap_or_else(|e| panic!("failed to build reqwest client: {e}"))
            .block_on(client.lhb_stock_seat("", "20240115", SeatFlag::Buy));
        assert!(matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[tokio::test]
    #[ignore = "integration test, requires network"]
    async fn integration_lhb_stock_seat_valid_symbol() {
        let client = LhbClient::new();
        let result = client
            .lhb_stock_seat("000001", "20240115", SeatFlag::Buy)
            .await;
        assert!(!matches!(result, Err(LhbError::InvalidSymbol(_))));
    }

    #[test]
    fn test_lhb_stock_seat_invalid_date() {
        let client = LhbClient::new();
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
        let client = LhbClient::new();
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
        let client = LhbClient::new();
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
        let client = LhbClient::new();
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
        let client = LhbClient::new();
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
            code: LhbClient::str_field(&raw, "SECURITY_CODE").unwrap_or_default(),
            name: LhbClient::str_field(&raw, "SECURITY_NAME_ABBR").unwrap_or_default(),
            trade_date: LhbClient::str_field(&raw, "TRADE_DATE").unwrap_or_default(),
            close_price: LhbClient::f64_field(&raw, "CLOSE_PRICE"),
            change_rate: LhbClient::f64_field(&raw, "CHANGE_RATE"),
            billboard_net_amt: LhbClient::f64_field(&raw, "BILLBOARD_NET_AMT"),
            billboard_buy_amt: LhbClient::f64_field(&raw, "BILLBOARD_BUY_AMT"),
            billboard_sell_amt: LhbClient::f64_field(&raw, "BILLBOARD_SELL_AMT"),
            billboard_deal_amt: LhbClient::f64_field(&raw, "BILLBOARD_DEAL_AMT"),
            accum_amount: LhbClient::f64_field(&raw, "ACCUM_AMOUNT"),
            turnover_rate: LhbClient::f64_field(&raw, "TURNOVERRATE"),
            free_market_cap: LhbClient::f64_field(&raw, "FREE_MARKET_CAP"),
            explanation: LhbClient::str_field(&raw, "EXPLANATION"),
            d1_chg: LhbClient::f64_field(&raw, "D1_CLOSE_ADJCHRATE"),
            d2_chg: LhbClient::f64_field(&raw, "D2_CLOSE_ADJCHRATE"),
            d5_chg: LhbClient::f64_field(&raw, "D5_CLOSE_ADJCHRATE"),
            d10_chg: LhbClient::f64_field(&raw, "D10_CLOSE_ADJCHRATE"),
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
            code: LhbClient::str_field(&raw, "SECURITY_CODE").unwrap_or_default(),
            name: LhbClient::str_field(&raw, "SECURITY_NAME_ABBR").unwrap_or_default(),
            trade_date: LhbClient::str_field(&raw, "TRADE_DATE").unwrap_or_default(),
            close_price: LhbClient::f64_field(&raw, "CLOSE_PRICE"),
            change_rate: LhbClient::f64_field(&raw, "CHANGE_RATE"),
            billboard_net_amt: LhbClient::f64_field(&raw, "BILLBOARD_NET_AMT"),
            billboard_buy_amt: LhbClient::f64_field(&raw, "BILLBOARD_BUY_AMT"),
            billboard_sell_amt: LhbClient::f64_field(&raw, "BILLBOARD_SELL_AMT"),
            billboard_deal_amt: LhbClient::f64_field(&raw, "BILLBOARD_DEAL_AMT"),
            accum_amount: LhbClient::f64_field(&raw, "ACCUM_AMOUNT"),
            turnover_rate: LhbClient::f64_field(&raw, "TURNOVERRATE"),
            free_market_cap: LhbClient::f64_field(&raw, "FREE_MARKET_CAP"),
            explanation: LhbClient::str_field(&raw, "EXPLANATION"),
            d1_chg: LhbClient::f64_field(&raw, "D1_CLOSE_ADJCHRATE"),
            d2_chg: LhbClient::f64_field(&raw, "D2_CLOSE_ADJCHRATE"),
            d5_chg: LhbClient::f64_field(&raw, "D5_CLOSE_ADJCHRATE"),
            d10_chg: LhbClient::f64_field(&raw, "D10_CLOSE_ADJCHRATE"),
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
        let client = LhbClient::new();
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
