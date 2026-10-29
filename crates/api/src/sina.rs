use anyhow::{bail, Context, Result};
use encoding_rs::GBK;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

use crate::eastmoney::{
    MarketStock, StockMarketFilter, StockRankingOrder, StockRankingPage, StockRankingRequest,
};

const MARKET_CENTER_URL: &str =
    "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
const MARKET_PAGE_SIZE: usize = 100;
const MARKET_MAX_PAGES: usize = 100;
const MARKET_CAP_UNIT: f64 = 10_000.0;

// ── 竞价行情数据结构 ────────────────────────────────

/// 盘口一档。新浪这一列给的是**股**；对外盘口模型统一换算成手。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SinaLevel {
    pub price: f64,
    /// 挂单量（股）
    pub volume: f64,
}

/// 个股集合竞价实时快照（9:15-9:25 持续刷新）。
///
/// 新浪一次就回完整五档，`bids[0]`/`asks[0]` 是买一/卖一，按「离现价由近到远」
/// 排列；`bid1_*`/`ask1_*` 是老字段（等于 `bids[0]`/`asks[0]`），保留是为了
/// 不破坏已有调用方。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuctionQuote {
    pub code: String,
    pub name: String,
    pub prev_close: f64,
    pub current_price: f64, // 竞价撮合价，9:15-9:25 持续刷新
    pub change_pct: f64,    // 涨跌幅%
    pub bid1_price: f64,    // 买一价
    pub bid1_volume: u64,   // 买一量(股)
    pub ask1_price: f64,    // 卖一价
    pub ask1_volume: u64,   // 卖一量(股)
    /// 买一 … 买五（量单位：股）
    pub bids: Vec<SinaLevel>,
    /// 卖一 … 卖五（量单位：股）
    pub asks: Vec<SinaLevel>,
    pub volume: u64,  // 累计撮合量(股)
    pub amount: f64,  // 累计成交额
    pub time: String, // 数据时间
}

// ── 新浪行情客户端 ────────────────────────────────────

#[derive(Clone)]
pub struct SinaQuoteClient {
    client: Result<Client, String>,
}

impl SinaQuoteClient {
    /// Create a new `SinaQuoteClient`.
    ///
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| error.to_string());
        Self { client }
    }

    fn http(&self) -> Result<&Client> {
        self.client
            .as_ref()
            .map_err(|error| anyhow::anyhow!("新浪 HTTP 客户端初始化失败: {error}"))
    }

    /// 批量获取竞价行情。
    ///
    /// `codes`: 不带前缀的纯代码，如 `["000001", "600519"]`。
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails or the response cannot be parsed.
    pub async fn fetch_quotes(&self, codes: &[&str]) -> Result<Vec<AuctionQuote>> {
        if codes.is_empty() {
            return Ok(vec![]);
        }
        let list = codes
            .iter()
            .map(|c| format!("{}{}", prefix_for(c), c))
            .collect::<Vec<_>>()
            .join(",");

        let url = format!("https://hq.sinajs.cn/list={list}");

        let bytes = self
            .http()?
            .get(&url)
            .header("Referer", "https://finance.sina.com.cn")
            .send()
            .await
            .context("请求新浪行情接口失败")?
            .bytes()
            .await?;

        let (text, _, had_errors) = GBK.decode(&bytes);
        if had_errors {
            tracing::warn!("[sina] GBK 解码存在异常字符");
        }

        Ok(parse_sina_response(&text, codes))
    }

    /// 获取全市场个股涨幅/跌幅榜。
    ///
    /// 新浪是主数据源；返回值复用个股排行 API 的统一模型。`hs_a` 包含
    /// 北交所，这里按现有接口契约过滤 `.BJ`。
    ///
    /// # Errors
    /// Returns an error if pagination parameters are invalid, the HTTP request
    /// fails, or the response cannot be parsed.
    pub async fn stock_rankings(&self, request: StockRankingRequest) -> Result<StockRankingPage> {
        if request.limit == 0 {
            bail!("个股排行数量必须大于 0");
        }

        let node = sina_market_node(request.market);
        collect_sina_ranking(request, |page| {
            self.fetch_market_page(page, request.order, node)
        })
        .await
    }

    async fn fetch_market_page(
        &self,
        page: usize,
        order: StockRankingOrder,
        node: &str,
    ) -> Result<Vec<Value>> {
        let response = self
            .http()?
            .get(MARKET_CENTER_URL)
            .query(&[
                ("page", page.to_string()),
                ("num", MARKET_PAGE_SIZE.to_string()),
                ("sort", "changepercent".to_string()),
                ("asc", sina_sort_order(order).to_string()),
                ("node", node.to_string()),
                ("symbol", String::new()),
                ("_s_r_a", "page".to_string()),
            ])
            .header("Referer", "https://finance.sina.com.cn/")
            .header("User-Agent", "Mozilla/5.0")
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("新浪个股排行请求失败")?;

        let status = response.status();
        if !status.is_success() {
            bail!("新浪个股排行请求失败: HTTP {status}");
        }
        let text = response.text().await.context("新浪个股排行响应读取失败")?;
        let rows: Vec<Value> =
            serde_json::from_str(&text).context("新浪个股排行响应不是合法 JSON")?;
        Ok(rows)
    }
}

async fn collect_sina_ranking<F, Fut>(
    request: StockRankingRequest,
    mut fetch_page: F,
) -> Result<StockRankingPage>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Value>>>,
{
    let mut items = Vec::with_capacity(request.limit);
    let mut matched = 0usize;
    let mut page = 1usize;

    while items.len() < request.limit {
        if page > MARKET_MAX_PAGES {
            bail!("新浪个股排行分页超过上限");
        }

        let rows = fetch_page(page).await?;
        if rows.is_empty() {
            break;
        }

        let row_count = rows.len();
        for row in rows {
            let Some(stock) = parse_sina_market_stock_row(&row) else {
                continue;
            };
            if !sina_includes_stock(&stock, request.market, request.exclude_st) {
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

        if row_count < MARKET_PAGE_SIZE {
            break;
        }
        page += 1;
    }

    if items.is_empty() && request.offset == 0 {
        bail!("新浪个股排行返回空数据");
    }
    let next_offset = if items.len() < request.limit {
        None
    } else {
        Some(request.offset.saturating_add(items.len()))
    };
    Ok(StockRankingPage { items, next_offset })
}

impl Default for SinaQuoteClient {
    fn default() -> Self {
        Self::new()
    }
}

fn sina_market_node(market: StockMarketFilter) -> &'static str {
    match market {
        StockMarketFilter::All | StockMarketFilter::MainBoard => "hs_a",
        StockMarketFilter::ChiNext => "cyb",
        StockMarketFilter::StarMarket => "kcb",
    }
}

const fn sina_sort_order(order: StockRankingOrder) -> &'static str {
    match order {
        StockRankingOrder::Gainers => "0",
        StockRankingOrder::Losers => "1",
    }
}

fn sina_matches_market(stock: &MarketStock, market: StockMarketFilter) -> bool {
    match market {
        StockMarketFilter::All => true,
        StockMarketFilter::MainBoard => match stock.symbol.rsplit_once('.') {
            Some((code, "SS")) => !code.starts_with("688") && !code.starts_with("689"),
            Some((code, "SZ")) => !code.starts_with("30"),
            _ => false,
        },
        StockMarketFilter::ChiNext => stock.symbol.ends_with(".SZ") && stock.code.starts_with("30"),
        StockMarketFilter::StarMarket => {
            stock.symbol.ends_with(".SS") && stock.code.starts_with("68")
        }
    }
}

fn sina_includes_stock(stock: &MarketStock, market: StockMarketFilter, exclude_st: bool) -> bool {
    !stock.symbol.ends_with(".BJ")
        && sina_matches_market(stock, market)
        && !(exclude_st && is_st_name(&stock.name))
}

fn parse_sina_market_stock_row(row: &Value) -> Option<MarketStock> {
    let code = row["code"].as_str()?.to_string();
    let raw_symbol = row["symbol"].as_str()?;
    let suffix = match raw_symbol.get(..2)? {
        "sh" => "SS",
        "sz" => "SZ",
        "bj" => "BJ",
        _ => return None,
    };
    Some(MarketStock {
        symbol: format!("{code}.{suffix}"),
        code,
        name: row["name"].as_str()?.to_string(),
        price: json_number(row, "trade"),
        change_percent: json_number(row, "changepercent"),
        turnover_rate: json_number(row, "turnoverratio"),
        total_market_cap: json_number(row, "mktcap").map(|value| value * MARKET_CAP_UNIT),
        float_market_cap: json_number(row, "nmc").map(|value| value * MARKET_CAP_UNIT),
    })
}

fn json_number(row: &Value, key: &str) -> Option<f64> {
    row.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str()?.parse::<f64>().ok())
    })
}

fn is_st_name(name: &str) -> bool {
    name.to_ascii_uppercase().contains("ST")
}

fn prefix_for(code: &str) -> &'static str {
    match code.chars().next() {
        Some('0' | '3') => "sz",
        Some('4' | '8') => "bj",
        _ => "sh",
    }
}

// ── 响应解析 ──────────────────────────────────────────

fn parse_sina_response(text: &str, codes: &[&str]) -> Vec<AuctionQuote> {
    let mut quotes = Vec::with_capacity(codes.len());

    for (line, code) in text.lines().zip(codes.iter()) {
        // var hq_str_sz000001="平安银行,9.300,9.250,9.300,...";
        let (Some(start), Some(end)) = (line.find('"'), line.rfind('"')) else {
            continue;
        };
        if end <= start + 1 {
            continue; // 停牌/无数据
        }
        let fields: Vec<&str> = line[start + 1..end].split(',').collect();
        if fields.len() < 32 {
            continue;
        }

        let f = |i: usize| fields[i].parse::<f64>().unwrap_or(0.0);
        // 买卖五档：10..19 = (买一量,买一价) … (买五量,买五价)，
        // 20..29 = (卖一量,卖一价) … (卖五量,卖五价)。量单位是股。
        let level = |vol: usize, price: usize| SinaLevel {
            price: f(price),
            volume: f(vol),
        };
        let bids: Vec<SinaLevel> = (0..5).map(|i| level(10 + i * 2, 11 + i * 2)).collect();
        let asks: Vec<SinaLevel> = (0..5).map(|i| level(20 + i * 2, 21 + i * 2)).collect();
        let prev_close = f(2);
        let current_price = f(3);
        let change_pct = if prev_close > 0.0 {
            (current_price - prev_close) / prev_close * 100.0
        } else {
            0.0
        };

        quotes.push(AuctionQuote {
            code: (*code).to_string(),
            name: fields[0].to_string(),
            prev_close,
            current_price,
            change_pct,
            bid1_price: f(6),
            bid1_volume: fields[10].parse().unwrap_or(0),
            ask1_price: f(7),
            ask1_volume: fields[20].parse().unwrap_or(0),
            bids,
            asks,
            volume: fields[8].parse().unwrap_or(0),
            amount: f(9),
            time: fields[31].to_string(),
        });
    }

    quotes
}

// ── 测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 五档解析：10..19 = 买一..买五、20..29 = 卖一..卖五，量按股原样保留。
    /// 报文取自真实接口（2026-10-26 收盘后，卖档全 0 是正常形态）。
    #[test]
    fn parses_five_level_depth() {
        let line = r#"var hq_str_sz000001="平安银行,11.150,11.200,11.300,11.340,11.130,11.290,11.300,132311308,1485999504.060,11300,11.300,200,11.280,300,11.270,400,11.260,500,11.250,0,0.000,0,0.000,0,0.000,0,0.000,0,0.000,2026-10-26,15:13:45,00";"#;
        let quotes = parse_sina_response(line, &["000001"]);
        let q = quotes.first().unwrap_or_else(|| panic!("应解析出 1 条"));
        assert_eq!(q.bids.len(), 5);
        assert_eq!(q.asks.len(), 5);
        // 买一 = 11.300 / 11300 股，买五 = 11.250 / 500 股
        assert!((q.bids[0].price - 11.300).abs() < 1e-9);
        assert!((q.bids[0].volume - 11300.0).abs() < 1e-9);
        assert!((q.bids[4].price - 11.250).abs() < 1e-9);
        assert!((q.bids[4].volume - 500.0).abs() < 1e-9);
        // 收盘后卖档全 0：档位存在但价量为 0，由调用方判定「无挂单」
        assert!((q.asks[0].price - 0.0).abs() < 1e-9);
        assert_eq!(q.bid1_volume, 11300);
    }

    #[test]
    fn parses_ranking_rows_and_normalizes_units() {
        let row = serde_json::json!({
            "symbol": "sh600519",
            "code": "600519",
            "name": "贵州茅台",
            "trade": "1500.50",
            "changepercent": "1.23",
            "turnoverratio": "0.50",
            "mktcap": "190000000",
            "nmc": "190000000"
        });

        let stock =
            parse_sina_market_stock_row(&row).unwrap_or_else(|| panic!("ranking row should parse"));
        assert_eq!(stock.symbol, "600519.SS");
        assert_eq!(stock.code, "600519");
        assert_eq!(stock.price, Some(1500.5));
        assert_eq!(stock.change_percent, Some(1.23));
        assert_eq!(stock.turnover_rate, Some(0.5));
        assert_eq!(stock.total_market_cap, Some(1_900_000_000_000.0));
        assert_eq!(stock.float_market_cap, Some(1_900_000_000_000.0));
    }

    #[test]
    fn ranking_market_filters_match_existing_contract() {
        let stock = |code: &str, suffix: &str| MarketStock {
            code: code.to_string(),
            symbol: format!("{code}.{suffix}"),
            name: format!("股票{code}"),
            price: Some(10.0),
            change_percent: Some(1.0),
            turnover_rate: Some(1.0),
            total_market_cap: Some(1.0),
            float_market_cap: Some(1.0),
        };

        assert!(sina_matches_market(
            &stock("600519", "SS"),
            StockMarketFilter::MainBoard
        ));
        assert!(sina_matches_market(
            &stock("000001", "SZ"),
            StockMarketFilter::MainBoard
        ));
        assert!(!sina_matches_market(
            &stock("688001", "SS"),
            StockMarketFilter::MainBoard
        ));
        assert!(!sina_matches_market(
            &stock("300001", "SZ"),
            StockMarketFilter::MainBoard
        ));
        assert!(sina_matches_market(
            &stock("300001", "SZ"),
            StockMarketFilter::ChiNext
        ));
        assert!(sina_matches_market(
            &stock("302132", "SZ"),
            StockMarketFilter::ChiNext
        ));
        assert!(!sina_matches_market(
            &stock("302132", "SZ"),
            StockMarketFilter::MainBoard
        ));
        assert!(sina_matches_market(
            &stock("688001", "SS"),
            StockMarketFilter::StarMarket
        ));
        assert!(!sina_matches_market(
            &stock("920001", "BJ"),
            StockMarketFilter::MainBoard
        ));
        assert!(!sina_includes_stock(
            &stock("920001", "BJ"),
            StockMarketFilter::All,
            false
        ));
        assert!(!sina_includes_stock(
            &MarketStock {
                name: "*ST测试".to_string(),
                ..stock("600001", "SS")
            },
            StockMarketFilter::All,
            true
        ));
    }

    #[test]
    fn ranking_market_nodes_and_sort_orders_match_sina_api() {
        assert_eq!(sina_market_node(StockMarketFilter::All), "hs_a");
        assert_eq!(sina_market_node(StockMarketFilter::MainBoard), "hs_a");
        assert_eq!(sina_market_node(StockMarketFilter::ChiNext), "cyb");
        assert_eq!(sina_market_node(StockMarketFilter::StarMarket), "kcb");
        assert_eq!(sina_sort_order(StockRankingOrder::Gainers), "0");
        assert_eq!(sina_sort_order(StockRankingOrder::Losers), "1");
    }

    fn ranking_row(symbol: &str, code: &str, name: &str) -> Value {
        serde_json::json!({
            "symbol": symbol,
            "code": code,
            "name": name,
            "trade": "10.00",
            "changepercent": "1.00",
            "turnoverratio": "1.00",
            "mktcap": "100000",
            "nmc": "80000"
        })
    }

    #[tokio::test]
    async fn ranking_filtered_offset_continues_across_pages() {
        let mut page_one = vec![
            ranking_row("bj920001", "920001", "北交测试"),
            ranking_row("sh600001", "600001", "*ST测试"),
            ranking_row("sh688001", "688001", "科创测试"),
            ranking_row("sh600100", "600100", "主板一"),
        ];
        while page_one.len() < MARKET_PAGE_SIZE {
            let index = page_one.len();
            let code = format!("30{index:04}");
            page_one.push(ranking_row(&format!("sz{code}"), &code, "创业板测试"));
        }
        let page_two = vec![
            ranking_row("sz000001", "000001", "主板二"),
            ranking_row("sz002001", "002001", "主板三"),
            ranking_row("sh600200", "600200", "主板四"),
        ];
        let pages = std::sync::Mutex::new(std::collections::HashMap::from([
            (1usize, page_one),
            (2usize, page_two),
        ]));

        let result = collect_sina_ranking(
            StockRankingRequest {
                order: StockRankingOrder::Gainers,
                market: StockMarketFilter::MainBoard,
                exclude_st: true,
                offset: 1,
                limit: 2,
            },
            |page| {
                let rows = pages
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&page)
                    .unwrap_or_default();
                async move { Ok(rows) }
            },
        )
        .await
        .unwrap_or_else(|error| panic!("scripted ranking should succeed: {error}"));

        assert_eq!(
            result
                .items
                .iter()
                .map(|stock| stock.code.as_str())
                .collect::<Vec<_>>(),
            vec!["000001", "002001"]
        );
        assert_eq!(result.next_offset, Some(3));
    }

    #[tokio::test]
    async fn ranking_empty_first_page_is_an_error() {
        let result = collect_sina_ranking(StockRankingRequest::default(), |_| async {
            Ok(Vec::<Value>::new())
        })
        .await;

        assert!(result.is_err());
    }

    use std::time::Instant;

    #[tokio::test]
    #[ignore = "integration test, requires network and live auction data"]
    async fn test_fetch_single_stock() {
        let client = SinaQuoteClient::new();
        let start = Instant::now();
        let quotes = client
            .fetch_quotes(&["000001"])
            .await
            .unwrap_or_else(|e| panic!("test failed: {e}"));
        let elapsed = start.elapsed();
        assert!(!quotes.is_empty(), "should return at least 1 quote");
        let q = &quotes[0];
        assert_eq!(q.code, "000001");
        assert!(!q.name.is_empty(), "name should not be empty");
        assert!(q.prev_close > 0.0, "prev_close should be > 0");
        assert!(
            q.current_price > 0.0 || q.volume > 0,
            "price or volume should be meaningful"
        );
        eprintln!(
            "[sina test] {} {} | prev={:.2} price={:.2} chg={:.2}% | bid {:.2}x{} ask {:.2}x{} | vol={} amount={:.0} | {}ms",
            q.code,
            q.name,
            q.prev_close,
            q.current_price,
            q.change_pct,
            q.bid1_price,
            q.bid1_volume,
            q.ask1_price,
            q.ask1_volume,
            q.volume,
            q.amount,
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_fetch_batch() {
        let client = SinaQuoteClient::new();
        let codes = ["000001", "600519", "300750", "000858"];
        let quotes = client
            .fetch_quotes(&codes)
            .await
            .unwrap_or_else(|e| panic!("test failed: {e}"));
        assert_eq!(quotes.len(), codes.len(), "all codes should return data");
        for q in &quotes {
            assert!(!q.name.is_empty(), "{} name empty", q.code);
            eprintln!(
                "{} {} chg={:.2}% vol={}",
                q.code, q.name, q.change_pct, q.volume
            );
        }
    }

    #[tokio::test]
    #[ignore = "integration test, requires network and live market data"]
    async fn integration_stock_rankings() {
        let client = SinaQuoteClient::new();
        let page = client
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Gainers,
                market: StockMarketFilter::All,
                exclude_st: false,
                offset: 0,
                limit: 30,
            })
            .await
            .unwrap_or_else(|error| panic!("sina ranking request should succeed: {error}"));

        assert_eq!(page.items.len(), 30);
        assert_eq!(page.next_offset, Some(30));
        assert!(page
            .items
            .iter()
            .all(|stock| !stock.symbol.ends_with(".BJ")));

        let losers = client
            .stock_rankings(StockRankingRequest {
                order: StockRankingOrder::Losers,
                market: StockMarketFilter::All,
                exclude_st: true,
                offset: 0,
                limit: 30,
            })
            .await
            .unwrap_or_else(|error| panic!("sina losers request should succeed: {error}"));
        let first = losers
            .items
            .first()
            .and_then(|stock| stock.change_percent)
            .unwrap_or(0.0);
        let last = losers
            .items
            .last()
            .and_then(|stock| stock.change_percent)
            .unwrap_or(0.0);
        assert!(
            first <= last,
            "losers should be sorted ascending: {first} > {last}"
        );
    }
}
